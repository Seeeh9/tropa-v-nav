//! Sound events and layered battle music (design doc, section 11).
//! A generated `name-flow.ogg` always wins over the synthesised `name.wav`.
//! Missing files are simply silent, so the game never waits for audio.

use crate::art::Art;
use crate::session::{Cues, Session};
use crate::AppState;
use bevy::audio::Volume;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use nav_core::{Event, IntentKind, NodeKind, UnitRef};
use std::collections::HashMap;

const SFX: &[&str] = &[
    "card_play", "card_draw", "hit", "hit_heavy", "block", "heal", "status", "death", "downed", "raised", "click", "ready", "round", "rune", "potion", "gold", "victory",
    "defeat",
];

pub fn plugin(app: &mut App) {
    // unattended screenshot runs (dev) stay silent
    let quiet = cfg!(feature = "dev") && std::env::var("TROPA_SHOT").is_ok();
    app.insert_resource(if quiet { Mixer { music: 0.0, sfx: 0.0, narrator: 0.0, voices: 0.0, duck: 1.0 } } else { Mixer { music: 0.6, sfx: 0.8, narrator: 1.0, voices: 0.8, duck: 1.0 } })
        .add_systems(Update, duck_music)
        .add_systems(Startup, load)
        .add_systems(Update, cue_sounds.after(crate::session::playback).run_if(in_state(AppState::Battle)).run_if(resource_exists::<Session>))
        .add_systems(Update, speak_lines.after(crate::session::playback).run_if(in_state(AppState::Battle)).run_if(resource_exists::<Session>))
        .add_systems(Update, (battle_layers.run_if(in_state(AppState::Battle)).run_if(resource_exists::<Session>), fade_layers))
        .init_resource::<BattleTrack>()
        .add_systems(Update, (ambient, fade_ambient))
        .add_systems(Update, koschei_movements.run_if(in_state(AppState::Battle)).run_if(resource_exists::<Session>))
        .add_systems(OnEnter(AppState::Battle), start_battle_music)
        .add_systems(OnExit(AppState::Battle), stop_music)
        .add_observer(|_: On<Activate>, mut commands: Commands, sfx: Res<Sfx>, mixer: Res<Mixer>| sfx.play(&mut commands, "click", &mixer, 1.0));
}

/// Player volume settings, 0..1.
#[derive(Resource)]
pub struct Mixer {
    pub music: f32,
    pub sfx: f32,
    pub narrator: f32,
    pub voices: f32,
    /// What is left of the music while the storyteller speaks: 1.0 when he is silent.
    pub duck: f32,
}

/// The storyteller's voice. While one plays, the music steps back.
#[derive(Component)]
pub struct Narration;

/// Share of the music's volume kept under the storyteller.
const DUCKED: f32 = 0.3;

fn duck_music(time: Res<Time>, mut mixer: ResMut<Mixer>, speaking: Query<(), With<Narration>>) {
    let want = if speaking.is_empty() { 1.0 } else { DUCKED };
    if (mixer.duck - want).abs() > 1e-3 {
        // down quickly so the first word is heard, back up slowly
        let rate = if want < mixer.duck { 2.5 } else { 0.8 };
        let step = rate * time.delta_secs();
        mixer.duck = if want < mixer.duck { (mixer.duck - step).max(want) } else { (mixer.duck + step).min(want) };
    }
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

/// A hero's or a master's spoken line. Lines never talk over each other or over the storyteller.
#[derive(Component)]
pub struct Line;

/// The masters of the acts, who speak when a fight with them begins, turns and ends.
const MASTERS: [&str; 4] = ["leshy", "bolotnik", "mara", "koschei"];
/// Seconds between two of the lesser lines (a kill, a heavy blow), so a fight is not all talk.
const BETWEEN_LINES: f32 = 7.0;

#[derive(Default)]
struct Lines {
    /// Who last struck each enemy: the one who speaks when it dies.
    struck_by: HashMap<u32, u8>,
    /// Heroes who have already complained of a heavy blow this fight.
    hurt: Vec<u8>,
    quiet: f32,
}

/// Picks the line an event deserves: `(speaker id, event, always)`; lesser lines wait their turn.
fn line_for(e: &Event, shown: &nav_core::Combat, me: u8, st: &mut Lines) -> Option<(String, &'static str, bool)> {
    let hero = |slot: u8| shown.hero(slot).map(|h| h.def.to_string());
    let master = |uid: u32| shown.enemy(uid).map(|e| e.def.to_string()).filter(|d| MASTERS.contains(&d.as_str()));
    match e {
        Event::RoundStart(1) => {
            *st = Lines::default();
            match shown.living_enemies().map(|e| e.def.to_string()).find(|d| MASTERS.contains(&d.as_str())) {
                Some(m) => Some((m, "start", true)),
                None => hero(me).map(|h| (h, "start", true)),
            }
        }
        Event::Damage { src: Some(UnitRef::Hero(s)), dst: UnitRef::Enemy(uid), .. } => {
            st.struck_by.insert(*uid, *s);
            None
        }
        Event::Damage { dst: UnitRef::Hero(s), lost, hp, .. } if *hp > 0 && (*lost >= 12 || shown.hero(*s).is_some_and(|h| *hp * 3 <= h.max_hp)) => {
            if st.hurt.contains(s) {
                return None;
            }
            st.hurt.push(*s);
            hero(*s).map(|h| (h, "hurt", false))
        }
        Event::EnemyDied { enemy } => match master(*enemy) {
            Some(m) => Some((m, "death", true)),
            None => st.struck_by.get(enemy).and_then(|s| hero(*s)).map(|h| (h, "kill", false)),
        },
        Event::Phase2 { enemy } => master(*enemy).map(|m| (m, "phase2", true)),
        Event::Downed { player } => hero(*player).map(|h| (h, "down", true)),
        Event::Raised { player, .. } => hero(*player).map(|h| (h, "raise", true)),
        Event::Ended { won: true } => hero(me).map(|h| (h, "win", true)),
        _ => None,
    }
}

fn speak_lines(
    mut commands: Commands,
    time: Res<Time>,
    cues: Res<Cues>,
    session: Res<Session>,
    mixer: Res<Mixer>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    l10n: Res<crate::l10n::L10n>,
    profile: Res<crate::profile::Profile>,
    talking: Query<(), Or<(With<Line>, With<Narration>)>>,
    mut st: Local<Lines>,
) {
    st.quiet = (st.quiet - time.delta_secs()).max(0.0);
    let lang = l10n.lang.voice(profile.settings.voice_lang).code();
    let mut busy = !talking.is_empty();
    for e in &cues.0 {
        let Some((who, what, always)) = line_for(e, &session.shown, session.me, &mut st) else { continue };
        if busy || mixer.voices <= 0.0 || (!always && st.quiet > 0.0) {
            continue;
        }
        let file = format!("voice/{lang}/bark-{who}-{what}.wav");
        if !art.root().join(&file).is_file() {
            continue;
        }
        commands.spawn((Line, AudioPlayer::new(assets.load::<AudioSource>(file)), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(mixer.voices))));
        busy = true;
        st.quiet = BETWEEN_LINES;
    }
}

/// A generated track if there is one, else the synthesised stand-in.
fn music_file(art: &Art, name: &str) -> Option<String> {
    let stem = format!("music/{name}");
    art.find(&stem, "ogg", "flow").filter(|p| p.ends_with("-flow.ogg")).or_else(|| art.find(&stem, "wav", "flow"))
}

// ---- music between battles ----

/// The loop that plays outside battle. One is on its way in; older ones fade out and go.
#[derive(Component)]
struct Ambient {
    track: String,
    target: f32,
    level: f32,
}

/// Which piece belongs to which screen, most fitting first: the first one that exists plays.
/// The flag marks a piece that sounds once and is not looped (the boss fanfare, the lament of defeat).
/// Battle has its own layered music.
fn ambient_tracks(state: AppState, run: Option<&nav_core::Run>) -> (Vec<String>, bool) {
    let act = run.map(|r| r.act).unwrap_or(1).clamp(1, 3);
    let one = |name: &str| (vec![name.to_string()], false);
    match state {
        AppState::Menu | AppState::Settings => one("menu"),
        AppState::Coop | AppState::Lobby => (vec!["lobby".into(), "menu".into()], false),
        AppState::Village | AppState::Chronicle => one("village"),
        // the fanfare for a fallen boss sounds once over the spoils
        AppState::Reward if run.is_some_and(|r| r.fighting == Some(NodeKind::Boss)) => (vec!["victory".into()], true),
        AppState::Map | AppState::Reward => (vec![format!("map_act{act}"), "map_act1".into(), "map".into()], false),
        AppState::Rest => one("rest"),
        AppState::Shop => one("shop"),
        AppState::Omen => one("omen"),
        AppState::RunEnd if run.is_some_and(|r| r.stage == nav_core::Stage::Victory) => (vec!["credits".into(), "end".into()], false),
        AppState::RunEnd => (vec!["defeat".into(), "end".into()], true),
        _ => (vec![], false),
    }
}

fn ambient(
    mut commands: Commands,
    state: Res<State<AppState>>,
    net: Option<Res<crate::net::Net>>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    mut playing: Query<&mut Ambient>,
    mut last: Local<Option<String>>,
) {
    let run = net.as_ref().and_then(|n| n.node.state.as_ref());
    let (names, once) = ambient_tracks(*state.get(), run);
    let found = names.iter().find_map(|n| music_file(&art, n).map(|path| (n.clone(), path)));
    let want = found.as_ref().map(|(n, _)| n.clone());
    for mut a in &mut playing {
        a.target = if Some(&a.track) == want.as_ref() { 1.0 } else { 0.0 };
    }
    // a one-shot is started once per visit, not again when it has run out
    if *last != want {
        *last = want.clone();
        if let Some((track, path)) = found {
            let settings = if once { PlaybackSettings::DESPAWN } else { PlaybackSettings::LOOP };
            commands.spawn((Ambient { track, target: 1.0, level: 0.0 }, AudioPlayer::<AudioSource>::new(assets.load(path)), settings.with_volume(Volume::Linear(0.0))));
        }
    }
}

fn fade_ambient(mut commands: Commands, time: Res<Time>, mixer: Res<Mixer>, mut playing: Query<(Entity, &mut Ambient, Option<&mut AudioSink>)>) {
    for (entity, mut a, sink) in &mut playing {
        // a second and a half to cross from one place's music to the next
        let step = time.delta_secs() / 1.5;
        a.level = if a.level < a.target { (a.level + step).min(a.target) } else { (a.level - step).max(a.target) };
        if let Some(mut sink) = sink {
            sink.set_volume(Volume::Linear(a.level * mixer.music * mixer.duck * 0.8));
        }
        if a.target == 0.0 && a.level == 0.0 {
            commands.entity(entity).despawn();
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

/// The battle track playing now, by name.
#[derive(Resource, Default)]
struct BattleTrack(String);

/// Starts the layers of `track` together, so they stay in step; only their volume moves.
fn spawn_layers(commands: &mut Commands, art: &Art, assets: &AssetServer, track: &str) {
    // a generated track never shares the stage with stand-in layers: they were not written together.
    // Until its own stems are in place it simply plays whole, as layer one
    let generated = |index: u8| art.find(&format!("music/{track}_l{index}"), "ogg", "flow").filter(|p| p.ends_with("-flow.ogg"));
    let real = generated(1).is_some();
    for index in 1..=3u8 {
        let path = if real { generated(index) } else { music_file(art, &format!("{track}_l{index}")) };
        let Some(path) = path else { continue };
        commands.spawn((
            MusicLayer { index, target: if index == 1 { 1.0 } else { 0.0 }, level: 0.0 },
            AudioPlayer::<AudioSource>::new(assets.load(path)),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
        ));
    }
}

fn start_battle_music(mut commands: Commands, art: Res<Art>, assets: Res<AssetServer>, net: Option<Res<crate::net::Net>>, mut now: ResMut<BattleTrack>) {
    let run = net.as_ref().and_then(|n| n.node.state.as_ref());
    let act = run.map(|r| r.act).unwrap_or(1);
    let track = match run.and_then(|r| r.fighting) {
        Some(NodeKind::Boss) if act >= 4 => "koschei".to_string(),
        Some(NodeKind::Boss) => format!("boss_act{act}"),
        Some(NodeKind::Elite) => "elite".to_string(),
        _ => format!("battle_act{act}"),
    };
    spawn_layers(&mut commands, &art, &assets, &track);
    now.0 = track;
}

/// Koschei's fight has three movements: on his throne, risen once the needle drops,
/// and the last breath after it is broken. The music follows him from one to the next.
fn koschei_movements(mut commands: Commands, session: Res<Session>, art: Res<Art>, assets: Res<AssetServer>, mut now: ResMut<BattleTrack>, layers: Query<Entity, With<MusicLayer>>) {
    if !now.0.starts_with("koschei") {
        return;
    }
    let Some(k) = session.truth.enemies.iter().find(|e| e.def == "koschei" && e.alive) else { return };
    let want = if !k.flags.contains_key("immortal") {
        "koschei3"
    } else if k.flags.contains_key("risen") {
        "koschei2"
    } else {
        "koschei"
    };
    // a movement nobody has written yet leaves the previous one playing
    if now.0 != want && music_file(&art, &format!("{want}_l1")).is_some() {
        for e in &layers {
            commands.entity(e).despawn();
        }
        spawn_layers(&mut commands, &art, &assets, want);
        now.0 = want.to_string();
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
        sink.set_volume(Volume::Linear(layer.level * mixer.music * mixer.duck));
    }
}
