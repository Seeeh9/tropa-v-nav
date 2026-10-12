//! Screens between fights: the path map with voting, rewards, the fire, the end of a run.
//! Each one is redrawn whole whenever the run changes; they hold no state of their own.

use crate::art::Art;
use crate::battle::{spawn_card, Btn};
use crate::l10n::L10n;
use crate::menu::menu_button;
use crate::net::{self, Net, NetStatus};
use crate::session::{Db, RunOutbox};
use crate::steam::Steam;
use crate::theme::{self, Fonts};
use crate::{AppState, Stage as Canvas, STAGE_H, STAGE_W};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};
use nav_core::*;

pub fn plugin(app: &mut App) {
    for state in [AppState::Map, AppState::Reward, AppState::Rest, AppState::RunEnd] {
        app.add_systems(OnEnter(state), |mut status: ResMut<NetStatus>, mut pick: ResMut<UpgradePick>| {
            status.run_dirty = true;
            pick.0 = false;
        });
    }
    app.init_resource::<UpgradePick>()
        .add_systems(Update, draw_map.run_if(in_state(AppState::Map)))
        .add_systems(Update, draw_reward.run_if(in_state(AppState::Reward)))
        .add_systems(Update, draw_rest.run_if(in_state(AppState::Rest)))
        .add_systems(Update, draw_end.run_if(in_state(AppState::RunEnd)));
}

#[derive(Component)]
struct ScreenRoot;

/// At the fire: the player has chosen "upgrade" and is now picking the card.
#[derive(Resource, Default)]
struct UpgradePick(bool);

fn node_color(kind: NodeKind) -> Color {
    match kind {
        NodeKind::Fight => theme::RUST_BLOOD,
        NodeKind::Elite => theme::FRESH_BLOOD,
        NodeKind::Event => theme::NAV_BLUE,
        NodeKind::Rest => theme::CANDLE,
        NodeKind::Shop => theme::SWAMP,
        NodeKind::Chest => Color::srgb(0.55, 0.42, 0.16),
        NodeKind::Boss => Color::srgb(0.35, 0.05, 0.05),
    }
}

/// Clears the previous drawing of a screen and returns a fresh full-stage root, or `None` if nothing changed.
fn redraw(commands: &mut Commands, status: &mut NetStatus, old: &Query<Entity, With<ScreenRoot>>, state: AppState, canvas: Entity, bg: Color) -> Option<Entity> {
    if !status.run_dirty {
        return None;
    }
    status.run_dirty = false;
    for e in old {
        commands.entity(e).despawn();
    }
    Some(
        commands
            .spawn((
                ScreenRoot,
                DespawnOnExit(state),
                Node { position_type: PositionType::Absolute, width: px(STAGE_W), height: px(STAGE_H), ..default() },
                BackgroundColor(bg),
                Pickable::IGNORE,
                ChildOf(canvas),
            ))
            .id(),
    )
}

/// The party's health and purses, top-left on every between-fights screen.
fn party_panel(commands: &mut Commands, root: Entity, run: &Run, me: u8, fonts: &Fonts, l10n: &L10n) {
    commands
        .spawn((
            theme::panel(Node {
                position_type: PositionType::Absolute,
                left: px(16),
                top: px(16),
                padding: UiRect::all(px(12)),
                border: UiRect::all(px(2)),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                ..default()
            }),
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .with_children(|p| {
            for (i, pl) in run.players.iter().enumerate() {
                let name = l10n.t(&format!("hero-{}", pl.hero));
                let line = l10n.tf("run-party-line", &[("name", name.into()), ("hp", pl.hp.into()), ("max", pl.max_hp.into()), ("gold", pl.gold.into()), ("cards", (pl.deck.len() as i32).into())]);
                let color = if i as u8 == me { theme::BONE } else { theme::DIM };
                p.spawn(theme::text(line, &fonts.body, 20.0, color));
            }
        });
}

// ---- map ----

fn map_pos(floor: usize, col: u8) -> Vec2 {
    Vec2::new(560.0 + col as f32 * 135.0, 1000.0 - floor as f32 * 58.0)
}

const BOSS_POS: Vec2 = Vec2::new(965.0, 96.0);

fn draw_map(
    mut commands: Commands,
    mut status: ResMut<NetStatus>,
    net: Option<Res<Net>>,
    old: Query<Entity, With<ScreenRoot>>,
    canvas: Single<Entity, With<Canvas>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
) {
    let Some(net) = net else { return };
    let (Some(run), Some(me)) = (net.node.state.as_ref(), net.node.my_slot()) else { return };
    let Stage::Map { votes } = &run.stage else { return };
    let Some(root) = redraw(&mut commands, &mut status, &old, AppState::Map, *canvas, theme::SOOT) else { return };

    commands.spawn((
        theme::text(l10n.tf("map-title", &[("act", (run.act as i32).into())]), &fonts.title, 44.0, theme::BONE),
        Node { position_type: PositionType::Absolute, right: px(40), top: px(24), ..default() },
        ChildOf(root),
    ));
    commands.spawn((
        theme::text(l10n.t(if votes[me as usize].is_some() { "map-voted" } else { "map-hint" }), &fonts.body, 22.0, theme::DIM),
        Node { position_type: PositionType::Absolute, right: px(40), top: px(84), max_width: px(380), ..default() },
        TextLayout::justify(Justify::Right),
        ChildOf(root),
    ));
    // legend
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, right: px(40), top: px(250), flex_direction: FlexDirection::Column, row_gap: px(8), ..default() },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .with_children(|p| {
            for kind in [NodeKind::Fight, NodeKind::Elite, NodeKind::Event, NodeKind::Rest, NodeKind::Shop, NodeKind::Chest, NodeKind::Boss] {
                p.spawn((
                    Node { column_gap: px(10), align_items: AlignItems::Center, ..default() },
                    Pickable::IGNORE,
                    children![
                        (Node { width: px(22), height: px(22), border_radius: BorderRadius::MAX, ..default() }, BackgroundColor(node_color(kind)), Pickable::IGNORE),
                        theme::text(l10n.t(&format!("node-{kind:?}")), &fonts.body, 20.0, theme::BONE),
                    ],
                ));
            }
        });
    party_panel(&mut commands, root, run, me, &fonts, &l10n);

    let line = |commands: &mut Commands, a: Vec2, b: Vec2, lit: bool| {
        let d = b - a;
        let len = d.length();
        let mid = (a + b) / 2.0;
        commands.spawn((
            Node { position_type: PositionType::Absolute, left: px(mid.x - len / 2.0), top: px(mid.y - 2.0), width: px(len), height: px(if lit { 4.0 } else { 3.0 }), ..default() },
            BackgroundColor(if lit { theme::BONE.with_alpha(0.8) } else { theme::DIM.with_alpha(0.35) }),
            UiTransform { rotation: Rot2::radians(d.y.atan2(d.x)), ..UiTransform::IDENTITY },
            Pickable::IGNORE,
            ChildOf(root),
        ));
    };

    let choices = run.choices();
    let here = run.at;
    let next_floor = here.map(|(f, _)| f as usize + 1).unwrap_or(0);
    // paths first, so the nodes sit on top of them
    for (f, floor) in run.map.floors.iter().enumerate() {
        for (i, node) in floor.iter().enumerate() {
            let from = map_pos(f, node.col);
            if f + 1 < run.map.floors.len() {
                for n in &node.next {
                    let lit = here == Some((f as u8, i as u8));
                    line(&mut commands, from, map_pos(f + 1, run.map.floors[f + 1][*n as usize].col), lit);
                }
            } else {
                line(&mut commands, from, BOSS_POS, here == Some((f as u8, i as u8)));
            }
        }
    }
    let spawn_node = |commands: &mut Commands, at: Vec2, kind: NodeKind, choice: Option<u8>, standing: bool, passed: bool| {
        let size = if kind == NodeKind::Boss { 76.0 } else { 44.0 };
        let base = if passed { node_color(kind).mix(&theme::SOOT, 0.6) } else { node_color(kind) };
        let edge = if standing { theme::BONE } else if choice.is_some() { theme::CANDLE } else { theme::SOOT };
        let mut e = commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(at.x - size / 2.0),
                top: px(at.y - size / 2.0),
                width: px(size),
                height: px(size),
                border: UiRect::all(px(if choice.is_some() || standing { 4.0 } else { 2.0 })),
                border_radius: BorderRadius::MAX,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: px(2),
                ..default()
            },
            BackgroundColor(base),
            BorderColor::all(edge),
            ChildOf(root),
        ));
        if let Some(c) = choice {
            // only the nodes the party can step to are buttons
            e.insert((Button, Hovered::default(), Btn(base)));
            e.observe(move |_: On<Activate>, mut out: ResMut<RunOutbox>| out.0.push(RunAction::Vote(c)));
            // each player's token, in their hero's colour
            let tokens: Vec<Color> = votes.iter().enumerate().filter(|(_, v)| **v == Some(c)).map(|(p, _)| theme::hero_color(&run.players[p].hero)).collect();
            e.with_children(|t| {
                for color in tokens {
                    t.spawn((
                        Node { width: px(14), height: px(14), border: UiRect::all(px(2)), border_radius: BorderRadius::MAX, ..default() },
                        BackgroundColor(color.lighter(0.15)),
                        BorderColor::all(theme::BONE),
                        Pickable::IGNORE,
                    ));
                }
            });
        } else {
            e.insert(Pickable::IGNORE);
        }
    };
    for (f, floor) in run.map.floors.iter().enumerate() {
        for (i, node) in floor.iter().enumerate() {
            let choice = if f == next_floor { choices.iter().position(|(n, _)| *n as usize == i).map(|c| c as u8) } else { None };
            let standing = here == Some((f as u8, i as u8));
            spawn_node(&mut commands, map_pos(f, node.col), node.kind, choice, standing, f < next_floor && !standing);
        }
    }
    let boss_choice = (next_floor >= run.map.floors.len()).then_some(0);
    spawn_node(&mut commands, BOSS_POS, NodeKind::Boss, boss_choice, false, false);
}

// ---- rewards ----

fn waiting_line(commands: &mut Commands, root: Entity, fonts: &Fonts, l10n: &L10n) {
    commands.spawn((
        theme::text(l10n.t("run-waiting"), &fonts.body, 26.0, theme::DIM),
        Node { position_type: PositionType::Absolute, bottom: px(60), width: px(STAGE_W), justify_content: JustifyContent::Center, ..default() },
        TextLayout::justify(Justify::Center),
        ChildOf(root),
    ));
}

fn draw_reward(
    mut commands: Commands,
    mut status: ResMut<NetStatus>,
    net: Option<Res<Net>>,
    db: Res<Db>,
    old: Query<Entity, With<ScreenRoot>>,
    canvas: Single<Entity, With<Canvas>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    let Some(net) = net else { return };
    let (Some(run), Some(me)) = (net.node.state.as_ref(), net.node.my_slot()) else { return };
    let Stage::Reward { rewards } = &run.stage else { return };
    let Some(root) = redraw(&mut commands, &mut status, &old, AppState::Reward, *canvas, theme::SOOT) else { return };
    party_panel(&mut commands, root, run, me, &fonts, &l10n);
    let r = &rewards[me as usize];

    let column = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: px(STAGE_W),
                height: px(STAGE_H),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(22),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    commands.spawn((theme::text(l10n.t("reward-title"), &fonts.title, 64.0, theme::CANDLE), ChildOf(column)));
    if r.gold > 0 {
        commands.spawn((theme::text(l10n.n("reward-gold", r.gold), &fonts.body, 28.0, theme::BONE), ChildOf(column)));
    }
    if let Some(relic) = &r.relic {
        let line = format!("{} — {}", l10n.t(&format!("relic-{relic}")), l10n.t(&format!("relic-{relic}-desc")));
        commands.spawn((theme::text(l10n.tf("reward-relic", &[("relic", line.into())]), &fonts.body, 26.0, theme::ROTLIGHT), Node { max_width: px(1100), ..default() }, TextLayout::justify(Justify::Center), ChildOf(column)));
    }
    if r.done {
        waiting_line(&mut commands, root, &fonts, &l10n);
        return;
    }
    commands.spawn((theme::text(l10n.t("reward-pick"), &fonts.body, 26.0, theme::DIM), ChildOf(column)));
    let row = commands.spawn((Node { column_gap: px(40), margin: UiRect::vertical(px(10)), ..default() }, Pickable::IGNORE, ChildOf(column))).id();
    for (i, id) in r.cards.iter().enumerate() {
        let card = Card { uid: i as u32, def: id.clone(), upgraded: false, free: false };
        let frame = db.0.cards.get(id).map(crate::battle::card_frame).unwrap_or(theme::PEAT);
        if let Some(e) = spawn_card(&mut commands, row, &card, Form::Human, None, (Button, Btn(frame)), &db.0, &l10n, &fonts, &art, &assets) {
            commands.entity(e).observe(move |_: On<Activate>, mut out: ResMut<RunOutbox>| out.0.push(RunAction::TakeCard(Some(i as u8))));
        }
    }
    commands.spawn((
        menu_button(l10n.t("reward-skip"), &fonts),
        observe(|_: On<Activate>, mut out: ResMut<RunOutbox>| out.0.push(RunAction::TakeCard(None))),
        ChildOf(column),
    ));
}

// ---- the fire ----

fn draw_rest(
    mut commands: Commands,
    mut status: ResMut<NetStatus>,
    net: Option<Res<Net>>,
    db: Res<Db>,
    old: Query<Entity, With<ScreenRoot>>,
    canvas: Single<Entity, With<Canvas>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    pick: Res<UpgradePick>,
) {
    let Some(net) = net else { return };
    let (Some(run), Some(me)) = (net.node.state.as_ref(), net.node.my_slot()) else { return };
    let Stage::Rest { done } = &run.stage else { return };
    if pick.is_changed() {
        status.run_dirty = true;
    }
    let Some(root) = redraw(&mut commands, &mut status, &old, AppState::Rest, *canvas, Color::srgb(0.09, 0.06, 0.04)) else { return };
    party_panel(&mut commands, root, run, me, &fonts, &l10n);
    let pl = &run.players[me as usize];

    let column = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: px(STAGE_W),
                height: px(STAGE_H),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(18),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    commands.spawn((theme::text(l10n.t("rest-title"), &fonts.title, 64.0, theme::CANDLE), ChildOf(column)));
    if done[me as usize] {
        waiting_line(&mut commands, root, &fonts, &l10n);
        return;
    }
    if !pick.0 {
        let heal = (pl.max_hp * run.rules().rest_percent() / 100).min(pl.max_hp - pl.hp);
        commands.spawn((
            menu_button(l10n.n("rest-sleep", heal), &fonts),
            observe(|_: On<Activate>, mut out: ResMut<RunOutbox>| out.0.push(RunAction::Sleep)),
            ChildOf(column),
        ));
        commands.spawn((
            menu_button(l10n.t("rest-upgrade"), &fonts),
            observe(|_: On<Activate>, mut pick: ResMut<UpgradePick>| pick.0 = true),
            ChildOf(column),
        ));
        return;
    }
    commands.spawn((theme::text(l10n.t("rest-upgrade-hint"), &fonts.body, 24.0, theme::DIM), ChildOf(column)));
    let grid = commands
        .spawn((Node { width: px(1500), flex_wrap: FlexWrap::Wrap, justify_content: JustifyContent::Center, column_gap: px(10), row_gap: px(10), ..default() }, Pickable::IGNORE, ChildOf(column)))
        .id();
    for (i, (id, upgraded)) in pl.deck.iter().enumerate() {
        let Some(def) = db.0.cards.get(id) else { continue };
        if *upgraded || def.up.is_none() {
            continue;
        }
        // what the card becomes, in words
        let after = l10n.card_text(id, def, &def.view(true, Form::Human));
        let label = format!("{}+  ·  {}", l10n.card_name(id, false), after);
        commands.spawn((
            Button,
            Hovered::default(),
            Btn(theme::PEAT),
            Node { width: px(480), padding: UiRect::axes(px(12), px(8)), border: UiRect::all(px(2)), ..default() },
            BackgroundColor(theme::PEAT),
            BorderColor::all(if def.hero.is_empty() { theme::DIM } else { theme::hero_color(&def.hero) }),
            children![theme::text(label, &fonts.body, 18.0, theme::BONE)],
            observe(move |_: On<Activate>, mut out: ResMut<RunOutbox>| out.0.push(RunAction::Upgrade(i as u16))),
            ChildOf(grid),
        ));
    }
    commands.spawn((
        menu_button(l10n.t("coop-back"), &fonts),
        observe(|_: On<Activate>, mut pick: ResMut<UpgradePick>| pick.0 = false),
        ChildOf(column),
    ));
}

// ---- the end ----

fn draw_end(
    mut commands: Commands,
    mut status: ResMut<NetStatus>,
    net: Option<Res<Net>>,
    old: Query<Entity, With<ScreenRoot>>,
    canvas: Single<Entity, With<Canvas>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
) {
    let Some(net) = net else { return };
    let Some(run) = net.node.state.as_ref() else { return };
    let Some(root) = redraw(&mut commands, &mut status, &old, AppState::RunEnd, *canvas, theme::SOOT) else { return };
    let won = run.stage == Stage::Victory;
    let heroes = run.players.iter().map(|p| l10n.t(&format!("hero-{}", p.hero))).collect::<Vec<_>>().join(", ");
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: px(STAGE_W),
            height: px(STAGE_H),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(24),
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(root),
        children![
            theme::text(l10n.t(if won { "end-victory" } else { "end-defeat" }), &fonts.title, 96.0, if won { theme::CANDLE } else { theme::FRESH_BLOOD }),
            theme::text(heroes, &fonts.body, 30.0, theme::BONE),
            theme::text(
                l10n.tf("end-stats", &[("floors", (run.floors_cleared as i32).into()), ("elites", (run.elites_slain as i32).into()), ("bosses", (run.bosses_slain as i32).into())]),
                &fonts.body,
                26.0,
                theme::DIM
            ),
            theme::text(l10n.n("end-oberegi", run.oberegi() as i32), &fonts.num, 40.0, theme::CANDLE),
            (
                menu_button(l10n.t("battle-to-menu"), &fonts),
                observe(|_: On<Activate>, mut commands: Commands, steam: Option<Res<Steam>>, net: Res<Net>, mut next: ResMut<NextState<AppState>>| {
                    net::leave(&mut commands, steam.as_deref(), &net);
                    next.set(AppState::Menu);
                }),
            ),
        ],
    ));
}
