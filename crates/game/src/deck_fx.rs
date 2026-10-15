//! What happens to the deck between fights, shown as it happens, over whatever screen is up (the
//! tale's book too): a card gained shows itself and flies into the deck, an upgraded one stands
//! beside what it was before it goes, a card lost burns away. One at a time, in order.

use crate::art::Art;
use crate::audio::{Mixer, Sfx};
use crate::battle::{spawn_card, CARD_H, CARD_W};
use crate::l10n::L10n;
use crate::net::NetStatus;
use crate::session::Db;
use crate::theme::{self, Fonts};
use crate::{Stage as Canvas, STAGE_W};
use bevy::prelude::*;
use nav_core::{Card, Form, RunEvent};

/// Where the deck is: the party plate in the top left corner, which counts the cards.
const DECK_AT: Vec2 = Vec2::new(240.0, 40.0);
/// The middle of the stage, a little high, where a card shows itself.
const SHOW_AT: Vec2 = Vec2::new(STAGE_W / 2.0, 470.0);
/// Seconds between one deck change and the next being shown.
const GAP: f32 = 0.45;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (cue, play, dim).chain());
}

/// The dark laid over the screen while a card is being shown, so the eye goes to it.
#[derive(Component)]
struct Scrim(f32);

fn dim(
    mut commands: Commands,
    time: Res<Time<Real>>,
    canvas: Single<Entity, With<Canvas>>,
    shown: Query<&Visibility, With<DeckFx>>,
    mut scrim: Query<(Entity, &mut Scrim, &mut BackgroundColor)>,
) {
    let busy = shown.iter().any(|v| *v == Visibility::Inherited);
    let dt = time.delta_secs();
    match scrim.single_mut() {
        Ok((e, mut s, mut bg)) => {
            s.0 = (s.0 + if busy { dt * 4.0 } else { -dt * 3.0 }).clamp(0.0, 1.0);
            bg.0 = Color::BLACK.with_alpha(0.6 * s.0);
            if s.0 <= 0.0 && !busy {
                commands.entity(e).despawn();
            }
        }
        Err(_) if busy => {
            commands.spawn((
                Scrim(0.0),
                Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
                BackgroundColor(Color::NONE),
                GlobalZIndex(590),
                Pickable::IGNORE,
                ChildOf(*canvas),
            ));
        }
        Err(_) => {}
    }
}

#[derive(Clone, Copy)]
enum Motion {
    /// Shows, holds, then flies into the deck, shrinking.
    IntoDeck,
    /// Shows, holds, then burns away where it stands.
    Burn,
    /// Shows and holds, then fades: what the card was before an upgrade.
    Fade,
}

#[derive(Component)]
struct DeckFx {
    at: Vec2,
    wait: f32,
    t: f32,
    hold: f32,
    motion: Motion,
}

/// The arrow between an upgraded card's old face and new; fades with the old face.
#[derive(Component)]
struct UpgradeArrow(f32);

#[allow(clippy::too_many_arguments)]
fn cue(
    mut commands: Commands,
    mut status: ResMut<NetStatus>,
    canvas: Single<Entity, With<Canvas>>,
    db: Res<Db>,
    l10n: Res<L10n>,
    fonts: Res<Fonts>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    time: Res<Time<Real>>,
    sfx: Res<Sfx>,
    mixer: Res<Mixer>,
    mut free_at: Local<f32>,
) {
    if status.deck.is_empty() {
        return;
    }
    let now = time.elapsed_secs();
    let mut start = (*free_at - now).max(0.0);
    let k = 0.72;
    let size = Vec2::new(CARD_W, CARD_H);
    let show = |commands: &mut Commands, id: &str, upgraded: bool, at: Vec2, wait: f32, hold: f32, motion: Motion| {
        let card = Card { uid: u32::MAX, def: id.to_string(), upgraded, free: false };
        if let Some(e) = spawn_card(commands, *canvas, &card, Form::Human, Some(at - size / 2.0), false, (), &db.0, &l10n, &fonts, &art, &assets) {
            commands.entity(e).insert((
                DeckFx { at, wait, t: 0.0, hold, motion },
                UiTransform { scale: Vec2::splat(0.01), ..default() },
                Visibility::Hidden,
                GlobalZIndex(600),
                Pickable::IGNORE,
            ));
        }
    };
    for ev in std::mem::take(&mut status.deck) {
        let length = match &ev {
            RunEvent::CardGained { card, .. } | RunEvent::CardTaken { card: Some(card), .. } => {
                show(&mut commands, card, false, SHOW_AT, start, 0.75, Motion::IntoDeck);
                sfx.play(&mut commands, "card_gain", &mixer, 1.0);
                1.5
            }
            RunEvent::Upgraded { card, .. } => {
                // the old face on the left, the new on the right, then the new goes into the deck
                let apart = CARD_W * k * 0.5 + 80.0;
                show(&mut commands, card, false, SHOW_AT - Vec2::X * apart, start, 1.3, Motion::Fade);
                show(&mut commands, card, true, SHOW_AT + Vec2::X * apart, start, 1.3, Motion::IntoDeck);
                commands.spawn((
                    UpgradeArrow(start),
                    theme::text("→", &fonts.num, 72.0, theme::CANDLE),
                    TextShadow { offset: Vec2::splat(3.0), color: Color::BLACK },
                    Node { position_type: PositionType::Absolute, left: px(SHOW_AT.x - 26.0), top: px(SHOW_AT.y - 48.0), ..default() },
                    Visibility::Hidden,
                    GlobalZIndex(601),
                    ChildOf(*canvas),
                ));
                sfx.play(&mut commands, "rune", &mixer, 1.0);
                2.1
            }
            RunEvent::CardRemoved { card, .. } => {
                show(&mut commands, card, false, SHOW_AT, start, 0.6, Motion::Burn);
                sfx.play(&mut commands, "status", &mixer, 1.0);
                1.4
            }
            _ => 0.0,
        };
        start += length + GAP;
    }
    *free_at = now + start;
}

fn play(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut cards: Query<(Entity, &mut DeckFx, &mut Node, &mut UiTransform, &mut Visibility, &mut BackgroundColor)>,
    mut arrows: Query<(Entity, &mut UpgradeArrow, &mut Visibility, &mut TextColor), Without<DeckFx>>,
) {
    let dt = time.delta_secs();
    let size = Vec2::new(CARD_W, CARD_H);
    for (e, mut fx, mut node, mut tf, mut vis, mut bg) in &mut cards {
        if fx.wait > 0.0 {
            fx.wait -= dt;
            continue;
        }
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
        fx.t += dt;
        // a quick swell into view, a little past full size and back
        let pop = (fx.t / 0.22).min(1.0);
        let shown = 0.72 * (1.0 + 0.12 * (pop * std::f32::consts::PI).sin()) * pop.max(0.01);
        let after = fx.t - 0.22 - fx.hold;
        if after <= 0.0 {
            tf.scale = Vec2::splat(shown);
            continue;
        }
        match fx.motion {
            Motion::IntoDeck => {
                let p = (after / 0.5).min(1.0);
                let ease = p * p;
                let at = fx.at.lerp(DECK_AT, ease) - Vec2::Y * 120.0 * (ease * std::f32::consts::PI).sin();
                node.left = px(at.x - size.x / 2.0);
                node.top = px(at.y - size.y / 2.0);
                tf.scale = Vec2::splat(0.72 + (0.08 - 0.72) * ease);
                tf.rotation = Rot2::radians(-0.5 * ease);
                if p >= 1.0 {
                    commands.entity(e).despawn();
                }
            }
            Motion::Burn => {
                let p = (after / 0.7).min(1.0);
                // it chars, curls and is gone
                bg.0 = bg.0.mix(&Color::srgb(0.55, 0.12, 0.04), p * 0.4);
                tf.scale = Vec2::new(0.72 * (1.0 - 0.25 * p), 0.72 * (1.0 - p));
                node.top = px(fx.at.y - size.y / 2.0 - 60.0 * p);
                if p >= 1.0 {
                    commands.entity(e).despawn();
                }
            }
            Motion::Fade => {
                let p = (after / 0.35).min(1.0);
                tf.scale = Vec2::splat(0.72 * (1.0 - 0.6 * p));
                if p >= 1.0 {
                    commands.entity(e).despawn();
                }
            }
        }
    }
    for (e, mut arrow, mut vis, mut color) in &mut arrows {
        arrow.0 -= dt;
        if arrow.0 > 0.0 {
            continue;
        }
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
        // shown with the two faces, gone with the old one
        let life = -arrow.0;
        let fade = ((life - 1.52) / 0.35).clamp(0.0, 1.0);
        color.0.set_alpha((life / 0.22).min(1.0) * (1.0 - fade));
        if fade >= 1.0 {
            commands.entity(e).despawn();
        }
    }
}
