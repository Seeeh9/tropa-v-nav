//! Sound events and layered battle music (design doc, section 11).
//! A generated `name-flow.ogg` always wins over the synthesised `name.wav`.
//! Missing files are simply silent, so the game never waits for audio.

use crate::art::Art;
use crate::session::{Cues, Session};
use crate::AppState;
use bevy::audio::Volume;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use nav_core::{Event, IntentKind, NodeKind};
use std::collections::HashMap;

const SFX: &[&str] = &[
    "card_play", "card_draw", "hit", "hit_heavy", "block", "heal", "status", "death", "downed", "raised", "click", "ready", "round", "rune", "potion", "gold", "victory",
    "defeat",
];

pub fn plugin(app: &mut App) {
    // unattended screenshot runs (dev) stay silent
    let quiet = cfg!(feature = "dev") && std::env::var("TROPA_SHOT").is_ok();
    app.insert_resource(if quiet { Mixer { music: 0.0, sfx: 0.0 } } else { Mixer { music: 0.6, sfx: 0.8 } })
        .add_systems(Startup, load)
        .add_systems(Update, cue_sounds.after(crate::session::playback).run_if(in_state(AppState::Battle)).run_if(resource_exists::<Session>))
        .add_systems(Update, (battle_layers.run_if(in_state(AppState::Battle)).run_if(resource_exists::<Session>), fade_layers))
        .add_systems(OnEnter(AppState::Battle), start_battle_music)
        .add_systems(OnExit(AppState::Battle), stop_music)
        .add_observer(|_: On<Activate>, mut commands: Commands, sfx: Res<Sfx>, mixer: Res<Mixer>| sfx.play(&mut commands, "click", &mixer, 1.0));
}

/// Player volume settings, 0..1.
#[derive(Resource)]
pub struct Mixer {
    pub music: f32,
    pub sfx: f32,
}

#[derive(Resource, Default)]
pub struct Sfx(HashMap<&'static str, Handle<AudioSource>>);

impl Sfx {
    pub fn play(&self, commands: &mut Commands, name: &str, mixer: &Mixer, gain: f32) {
        if let Some(handle) = self.0.get(name) {
            commands.spawn((AudioPlayer::new(handle.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(mixer.sfx * gain))));
        }
    }
}

fn load(mut commands: Commands, art: Res<Art>, assets: Res<AssetServer>) {
    let mut sfx = Sfx::default();
    for name in SFX {
        let stem = format!("sfx/{name}");
        // generated audio first, synthesised stand-in second
        let path = art.find(&stem, "ogg", "flow").filter(|p| p.ends_with("-flow.ogg")).or_else(|| art.find(&stem, "wav", "flow"));
        if let Some(path) = path {
            sfx.0.insert(*name, assets.load(path));
        }
    }
    commands.insert_resource(sfx);
}

fn cue_sounds(mut commands: Commands, cues: Res<Cues>, sfx: Res<Sfx>, mixer: Res<Mixer>) {
    // one sound of a kind per frame: an area attack is one crash, not six
    let mut played: Vec<&str> = Vec::new();
    for e in &cues.0 {
        let (name, gain) = match e {
            Event::CardPlayed { .. } => ("card_play", 1.0),
            Event::Drawn { .. } => ("card_draw", 0.8),
            Event::Damage { lost, .. } if *lost >= 14 => ("hit_heavy", 1.0),
            Event::Damage { lost, .. } if *lost > 0 => ("hit", 0.9),
            Event::Damage { blocked, .. } if *blocked > 0 => ("block", 0.9),
            Event::Block { .. } => ("block", 0.6),
            Event::Heal { .. } => ("heal", 0.9),
            Event::Status { delta, .. } if *delta > 0 => ("status", 0.7),
            Event::EnemyDied { .. } => ("death", 1.0),
            Event::Downed { .. } => ("downed", 1.0),
            Event::Raised { .. } => ("raised", 1.0),
            Event::Ready { ready: true, .. } => ("ready", 0.9),
            Event::RoundStart(_) => ("round", 0.7),
            Event::RuneFired { .. } | Event::RuneCarved { .. } => ("rune", 0.8),
            Event::PotionUsed { .. } | Event::Brewed { .. } => ("potion", 0.9),
            Event::GoldStolen { .. } => ("gold", 0.9),
            Event::Ended { won: true } => ("victory", 1.0),
            Event::Ended { won: false } => ("defeat", 1.0),
            _ => continue,
        };
        if !played.contains(&name) {
            played.push(name);
            sfx.play(&mut commands, name, &mixer, gain);
        }
    }
}

// ---- adaptive battle music ----

/// One of the three stacked layers of a battle track, and how loud it should be now.
#[derive(Component)]
struct MusicLayer {
    index: u8,
    target: f32,
    level: f32,
}

fn start_battle_music(mut commands: Commands, art: Res<Art>, assets: Res<AssetServer>, net: Option<Res<crate::net::Net>>) {
    let run = net.as_ref().and_then(|n| n.node.state.as_ref());
    let act = run.map(|r| r.act).unwrap_or(1);
    let track = match run.and_then(|r| r.fighting) {
        Some(NodeKind::Boss) if act >= 4 => "koschei".to_string(),
        Some(NodeKind::Boss) => format!("boss_act{act}"),
        Some(NodeKind::Elite) => "elite".to_string(),
        _ => format!("battle_act{act}"),
    };
    // all layers start together and stay in step; only their volume moves
    for index in 1..=3u8 {
        let Some(path) = art.find(&format!("music/{track}_l{index}"), "ogg", "flow") else { continue };
        commands.spawn((
            MusicLayer { index, target: if index == 1 { 1.0 } else { 0.0 }, level: 0.0 },
            AudioPlayer::<AudioSource>::new(assets.load(path)),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
        ));
    }
}

fn stop_music(mut commands: Commands, layers: Query<Entity, With<MusicLayer>>) {
    for e in &layers {
        commands.entity(e).despawn();
    }
}

/// Layer 1 always plays; layer 2 joins when enemies mean to attack; layer 3 when things are dire.
fn battle_layers(session: Res<Session>, db: Res<crate::session::Db>, net: Option<Res<crate::net::Net>>, mut layers: Query<&mut MusicLayer>) {
    let s = &session.shown;
    let attacking = s.living_enemies().any(|e| {
        e.intent.as_ref().and_then(|i| db.0.enemies.get(&e.def)?.moves.get(i.mv)).is_some_and(|m| matches!(m.intent, IntentKind::Attack | IntentKind::AttackAll))
    });
    let (hp, max): (i32, i32) = s.heroes.iter().fold((0, 0), |a, h| (a.0 + h.hp, a.1 + h.max_hp));
    let boss = net.as_ref().and_then(|n| n.node.state.as_ref()).and_then(|r| r.fighting) == Some(NodeKind::Boss);
    let dire = boss || s.heroes.iter().any(|h| h.downed) || hp * 10 < max * 3;
    for mut layer in &mut layers {
        layer.target = match layer.index {
            1 => 1.0,
            2 => attacking as u8 as f32,
            _ => dire as u8 as f32,
        };
    }
}

fn fade_layers(time: Res<Time>, mixer: Res<Mixer>, mut layers: Query<(&mut MusicLayer, &mut AudioSink)>) {
    for (mut layer, mut sink) in &mut layers {
        // about a bar to come in or go out, never a hard cut
        let step = time.delta_secs() / 2.0;
        layer.level = if layer.level < layer.target { (layer.level + step).min(layer.target) } else { (layer.level - step).max(layer.target) };
        sink.set_volume(Volume::Linear(layer.level * mixer.music));
    }
}
