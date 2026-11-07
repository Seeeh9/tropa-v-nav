//! A chest shared by the party: a relic for every player, claimed in turn; those who claim the
//! same one throw rock-paper-scissors for it, hand against hand.

use super::{party_panel, redraw, Canvas, ScreenRoot, STAGE_H, STAGE_W};
use crate::art::Art;
use crate::battle::Btn;
use crate::l10n::L10n;
use crate::menu::menu_button;
use crate::net::{Net, NetStatus};
use crate::session::RunOutbox;
use crate::theme::{self, Fonts};
use crate::AppState;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};
use nav_core::*;

const SHAPES: [&str; 3] = ["rock", "paper", "scissors"];
/// How long the fists are shaken before the throws are shown.
const SHAKE: f32 = 1.1;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (draw_chest, shake_hands).run_if(in_state(AppState::Omen)));
}

/// A thrown hand: shaken as a fist three times, then opened into its throw.
#[derive(Component)]
struct Shake {
    t: f32,
    throw: Handle<Image>,
    /// It lost the round: dimmed once shown.
    lost: bool,
}

/// What is shown once the throws are opened: who won or lost, the results.
#[derive(Component)]
struct AfterShake(f32);

fn hand(assets: &AssetServer, hero: &str, shape: u8) -> Handle<Image> {
    assets.load(crate::art::path(&format!("hands/{hero}_{}.png", SHAPES[shape.min(2) as usize])))
}

fn draw_chest(
    mut commands: Commands,
    mut status: ResMut<NetStatus>,
    net: Option<Res<Net>>,
    old: Query<Entity, With<ScreenRoot>>,
    canvas: Single<Entity, With<Canvas>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    // the last round already shaken out, so a redraw does not shake it again
    mut seen: Local<u32>,
) {
    let Some(net) = net else { return };
    let (Some(run), Some(me)) = (net.node.state.as_ref(), net.node.my_slot()) else { return };
    let Stage::Chest(ch) = &run.stage else {
        *seen = 0;
        return;
    };
    let Some(root) = redraw(&mut commands, &mut status, &old, AppState::Omen, *canvas, Color::srgb(0.06, 0.06, 0.08)) else { return };
    crate::between::act_backdrop(&mut commands, root, run, &art, &assets, Color::srgb(0.28, 0.26, 0.24));
    party_panel(&mut commands, root, run, me, &fonts, &l10n);
    let column = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: px(STAGE_W),
                height: px(STAGE_H),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(16),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    let picking = ch.picks.iter().any(|v| v.is_none());
    let fresh = ch.rounds != *seen && !ch.last.is_empty();
    *seen = ch.rounds;
    // what the screen says shows only once the hands are open
    let wait = if fresh { SHAKE } else { 0.0 };
    commands.spawn((theme::text(l10n.t("chest-title"), &fonts.title, 56.0, theme::BONE), ChildOf(column)));
    let hint = if picking {
        "chest-hint"
    } else if ch.settled {
        "chest-shared"
    } else if ch.contest.contains(&me) {
        if ch.throws[me as usize].is_some() { "chest-thrown" } else { "chest-throw" }
    } else {
        "chest-watch"
    };
    commands.spawn((theme::text(l10n.t(hint), &fonts.body, 22.0, theme::DIM), Node { margin: UiRect::bottom(px(8)), ..default() }, AfterShake(wait), ChildOf(column)));

    // the relics, each with the claims on it and, at the end, whose it is
    let row = commands.spawn((Node { column_gap: px(22), ..default() }, Pickable::IGNORE, ChildOf(column))).id();
    for (i, id) in ch.offers.iter().enumerate() {
        let i = i as u8;
        let mine = ch.picks[me as usize] == Some(i);
        let prize = ch.prize == Some(i);
        let owner = ch.won.iter().position(|w| *w == Some(i));
        let base = if mine || prize { theme::SWAMP } else { theme::PEAT };
        let mut b = commands.spawn((
            Node {
                width: px(330),
                min_height: px(330),
                padding: UiRect::all(px(14)),
                border: UiRect::all(px(2)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(8),
                ..default()
            },
            BackgroundColor(base),
            BorderColor::all(if prize { theme::CANDLE } else if mine { theme::ROTLIGHT } else { Color::srgb(0.42, 0.33, 0.22) }),
            ChildOf(row),
        ));
        if picking {
            b.insert((Button, Hovered::default(), Btn(base)));
            b.observe(move |_: On<Activate>, mut out: ResMut<RunOutbox>| out.0.push(RunAction::Choose(i)));
        } else {
            b.insert(Pickable::IGNORE);
        }
        b.with_children(|b| {
            b.spawn((Node { width: px(110), height: px(110), flex_shrink: 0.0, ..default() }, art.node(&assets, &format!("relics/{id}"), theme::SOOT), Pickable::IGNORE));
            b.spawn(theme::text(l10n.t(&format!("relic-{id}")), &fonts.title, 26.0, theme::CANDLE));
            b.spawn((theme::text(l10n.t(&format!("relic-{id}-desc")), &fonts.body, 19.0, theme::BONE), Node { max_width: px(300), ..default() }));
            // the claims: a dot in each claimant's colour; a struck-out one lost the throw
            b.spawn((Node { column_gap: px(8), margin: UiRect::top(px(4)), ..default() }, Pickable::IGNORE)).with_children(|d| {
                for (seat, v) in ch.picks.iter().enumerate() {
                    if *v == Some(i) {
                        let color = run.players.get(seat).map(|p| theme::hero_color(&p.hero)).unwrap_or(theme::BONE);
                        d.spawn((
                            Node { width: px(20), height: px(20), border: UiRect::all(px(2)), border_radius: BorderRadius::MAX, ..default() },
                            BackgroundColor(color),
                            BorderColor::all(theme::BONE),
                            Pickable::IGNORE,
                        ));
                    }
                }
            });
            if let (true, Some(q)) = (ch.settled, owner) {
                let hero = run.players.get(q).map(|p| p.hero.clone()).unwrap_or_default();
                b.spawn((theme::text(l10n.t(&format!("hero-{hero}")), &fonts.title, 24.0, theme::hero_color(&hero)), AfterShake(wait)));
            }
        });
    }

    // the hands of the last round thrown, facing each other
    if !ch.last.is_empty() {
        let hands = commands.spawn((Node { column_gap: px(40), margin: UiRect::top(px(10)), align_items: AlignItems::Center, ..default() }, Pickable::IGNORE, ChildOf(column))).id();
        let mut shapes: Vec<u8> = ch.last.iter().map(|(_, s)| *s).collect();
        shapes.sort_unstable();
        shapes.dedup();
        let best = match shapes[..] {
            [a, b] => Some(if beats(a, b) { a } else { b }),
            _ => None,
        };
        let n = ch.last.len();
        for (k, (q, shape)) in ch.last.iter().enumerate() {
            let hero = run.players.get(*q as usize).map(|p| p.hero.as_str()).unwrap_or("bogatyr");
            let lost = best.is_some_and(|b| b != *shape);
            // the left half reach in from the left, the right half from the right
            let flip = k * 2 >= n;
            let shown = if fresh { hand(&assets, hero, 0) } else { hand(&assets, hero, *shape) };
            let mut e = commands.spawn((
                ImageNode { flip_x: flip, color: if lost && !fresh { Color::srgba(1.0, 1.0, 1.0, 0.4) } else { Color::WHITE }, ..ImageNode::new(shown) },
                Node { width: px(300), height: px(162), ..default() },
                UiTransform::IDENTITY,
                Pickable::IGNORE,
                ChildOf(hands),
            ));
            if fresh {
                e.insert(Shake { t: 0.0, throw: hand(&assets, hero, *shape), lost });
            }
        }
        // a draw, or who is out of it
        let out: Vec<String> = ch.last.iter().filter(|(_, s)| best.is_some_and(|b| b != *s)).filter_map(|(q, _)| run.players.get(*q as usize)).map(|p| l10n.t(&format!("hero-{}", p.hero))).collect();
        let verdict = if best.is_none() { l10n.t("chest-draw") } else { l10n.tf("chest-out", &[("who", out.join(", ").into())]) };
        if !verdict.is_empty() {
            commands.spawn((theme::text(verdict, &fonts.title, 30.0, theme::CANDLE), AfterShake(wait), ChildOf(column)));
        }
    }

    // my throw, while I am still in the contest and have not thrown
    if !picking && !ch.settled && ch.contest.contains(&me) && ch.throws[me as usize].is_none() {
        let hero = run.players.get(me as usize).map(|p| p.hero.clone()).unwrap_or_default();
        let row = commands.spawn((Node { column_gap: px(24), margin: UiRect::top(px(10)), ..default() }, AfterShake(wait), ChildOf(column))).id();
        for shape in 0..3u8 {
            let mut b = commands.spawn((
                Button,
                Hovered::default(),
                Btn(theme::PEAT),
                Node { width: px(240), height: px(170), padding: UiRect::all(px(8)), border: UiRect::all(px(2)), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() },
                BackgroundColor(theme::PEAT),
                BorderColor::all(Color::srgb(0.42, 0.33, 0.22)),
                ChildOf(row),
            ));
            b.observe(move |_: On<Activate>, mut out: ResMut<RunOutbox>| out.0.push(RunAction::Throw(shape)));
            b.with_children(|b| {
                b.spawn((ImageNode::new(hand(&assets, &hero, shape)), Node { width: px(220), height: px(119), ..default() }, Pickable::IGNORE));
                b.spawn(theme::text(l10n.t(&format!("chest-{}", SHAPES[shape as usize])), &fonts.body, 20.0, theme::BONE));
            });
        }
    }

    if ch.settled && !ch.done[me as usize] {
        commands.spawn((menu_button(l10n.t("chest-onward"), &fonts), AfterShake(wait), observe(|_: On<Activate>, mut out: ResMut<RunOutbox>| out.0.push(RunAction::Onward)), ChildOf(column)));
    }
}

fn shake_hands(time: Res<Time>, mut hands: Query<(&mut Shake, &mut ImageNode, &mut UiTransform)>, mut after: Query<(&mut AfterShake, &mut Visibility)>) {
    let dt = time.delta_secs();
    for (mut s, mut image, mut tf) in &mut hands {
        if s.t >= SHAKE {
            continue;
        }
        s.t += dt;
        if s.t < SHAKE {
            // three beats down, as the words are said: rock, paper, scissors
            let beat = (s.t / SHAKE * 3.0).fract();
            tf.translation = Val2::px(0.0, -46.0 * (beat * std::f32::consts::PI).sin());
        } else {
            tf.translation = Val2::ZERO;
            image.image = s.throw.clone();
            if s.lost {
                image.color = Color::srgba(1.0, 1.0, 1.0, 0.4);
            }
        }
    }
    for (mut a, mut vis) in &mut after {
        a.0 -= dt;
        *vis = if a.0 > 0.0 { Visibility::Hidden } else { Visibility::Inherited };
    }
}
