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
    for state in [AppState::Map, AppState::Reward, AppState::Rest, AppState::Shop, AppState::Omen, AppState::RunEnd] {
        app.add_systems(OnEnter(state), |mut status: ResMut<NetStatus>, mut pick: ResMut<UpgradePick>| {
            status.run_dirty = true;
            pick.0 = false;
        });
    }
    app.init_resource::<UpgradePick>()
        .add_systems(Update, (draw_map, beckon).run_if(in_state(AppState::Map)))
        .add_systems(Update, draw_reward.run_if(in_state(AppState::Reward)))
        .add_systems(Update, draw_rest.run_if(in_state(AppState::Rest)))
        .add_systems(Update, draw_shop.run_if(in_state(AppState::Shop)))
        .add_systems(Update, (draw_omen, draw_trophy).run_if(in_state(AppState::Omen)))
        .add_systems(Update, draw_end.run_if(in_state(AppState::RunEnd)))
        .add_systems(Update, explain_cards);
}

/// What a card shown away from the fight mentions — its keywords and the statuses it lays — told
/// beside it while it is hovered, so nobody has to guess what Weak or Brew means.
#[derive(Component)]
pub struct CardHints {
    lines: Vec<(String, String)>,
    open: Option<Entity>,
}

impl CardHints {
    pub fn new(lines: Vec<(String, String)>) -> CardHints {
        CardHints { lines, open: None }
    }
}

fn explain_cards(mut commands: Commands, mut cards: Query<(Entity, &mut CardHints, &Hovered, Option<&UiTransform>)>, fonts: Res<Fonts>) {
    for (card, mut hints, hovered, shrunk) in &mut cards {
        match (hovered.get(), hints.open) {
            (true, None) if !hints.lines.is_empty() => {
                // a card shown small still explains itself at full size
                let k = 1.0 / shrunk.map(|t| t.scale.x).filter(|s| *s > 0.0).unwrap_or(1.0);
                let panel = commands
                    .spawn((
                        theme::panel(Node {
                            position_type: PositionType::Absolute,
                            left: percent(100),
                            top: px(0),
                            margin: UiRect::left(px(14.0 * k)),
                            width: px(320.0 * k),
                            padding: UiRect::all(px(12.0 * k)),
                            border: UiRect::all(px(2.0 * k)),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(10.0 * k),
                            ..default()
                        }),
                        GlobalZIndex(400),
                        Pickable::IGNORE,
                        ChildOf(card),
                    ))
                    .with_children(|p| {
                        for (name, help) in &hints.lines {
                            p.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(2.0 * k), ..default() }, Pickable::IGNORE)).with_children(|e| {
                                e.spawn(theme::text(name.clone(), &fonts.title, 22.0 * k, theme::CANDLE));
                                e.spawn(theme::text(help.clone(), &fonts.body, 20.0 * k, theme::BONE));
                            });
                        }
                    })
                    .id();
                hints.open = Some(panel);
            }
            (false, Some(panel)) => {
                commands.entity(panel).try_despawn();
                hints.open = None;
            }
            _ => {}
        }
    }
}

#[derive(Component)]
pub struct ScreenRoot;

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
    // alone, the bar along the top says it all
    if run.players.len() < 2 {
        return;
    }
    commands
        .spawn((
            theme::panel(Node {
                position_type: PositionType::Absolute,
                left: px(16),
                top: px(crate::between::BAR_H + 14.0),
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
/// The ink the map is drawn in, on its parchment.
const INK: Color = Color::srgb(0.17, 0.11, 0.07);

/// A node the party may step to next: it swells and settles, asking to be picked.
#[derive(Component)]
struct Beckon;

fn beckon(time: Res<Time>, mut q: Query<&mut UiTransform, With<Beckon>>) {
    let s = 1.0 + 0.12 * (time.elapsed_secs() * 4.0).sin().abs();
    for mut tf in &mut q {
        tf.scale = Vec2::splat(s);
    }
}

/// The ink colour a kind of place is drawn in on the map.
fn node_ink(kind: NodeKind) -> Color {
    node_color(kind).mix(&INK, 0.45)
}

fn draw_map(
    mut commands: Commands,
    mut status: ResMut<NetStatus>,
    net: Option<Res<Net>>,
    old: Query<Entity, With<ScreenRoot>>,
    canvas: Single<Entity, With<Canvas>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    mut spots: ResMut<crate::map_party::MapSpots>,
) {
    let Some(net) = net else { return };
    let (Some(run), Some(me)) = (net.node.state.as_ref(), net.node.my_slot()) else { return };
    let Stage::Map { votes } = &run.stage else { return };
    let Some(root) = redraw(&mut commands, &mut status, &old, AppState::Map, *canvas, theme::SOOT) else { return };

    // the act's forest, dimmed, and the map unrolled over it
    let act = run.act.clamp(1, 3);
    let forest = [format!("map/forest_act{act}"), format!("bg/act{act}_battle"), "bg/act1_battle".to_string()].into_iter().find(|s| art.image(&assets, s).is_some());
    if let Some(stem) = forest {
        let (mut image, _) = art.node(&assets, &stem, theme::SOOT);
        image.color = Color::srgb(0.32, 0.32, 0.34);
        commands.spawn((image, Node { position_type: PositionType::Absolute, width: px(STAGE_W), height: px(STAGE_H), ..default() }, Pickable::IGNORE, ChildOf(root)));
    }
    let scroll = [format!("map/scroll_act{act}"), "map/scroll".to_string(), "book/parchment".to_string()].into_iter().find(|s| art.image(&assets, s).is_some());
    let on_parchment = scroll.is_some();
    if let Some(stem) = scroll {
        commands.spawn((
            art.node(&assets, &stem, theme::PEAT),
            Node { position_type: PositionType::Absolute, left: px(420), top: px(crate::between::BAR_H + 4.0), width: px(1090), height: px(STAGE_H - crate::between::BAR_H - 10.0), ..default() },
            Pickable::IGNORE,
            ChildOf(root),
        ));
    }
    let icon = |kind: NodeKind| art.image(&assets, &format!("map/{}", format!("{kind:?}").to_lowercase()));

    commands.spawn((
        theme::text(l10n.tf("map-title", &[("act", (run.act as i32).into())]), &fonts.title, 44.0, theme::BONE),
        Node { position_type: PositionType::Absolute, right: px(40), top: px(90), ..default() },
        ChildOf(root),
    ));
    commands.spawn((
        theme::text(l10n.t(if votes[me as usize].is_some() { "map-voted" } else { "map-hint" }), &fonts.body, 22.0, theme::DIM),
        Node { position_type: PositionType::Absolute, right: px(40), top: px(150), max_width: px(380), ..default() },
        TextLayout::justify(Justify::Right),
        ChildOf(root),
    ));
    // legend
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, right: px(40), top: px(290), flex_direction: FlexDirection::Column, row_gap: px(8), ..default() },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .with_children(|p| {
            for kind in [NodeKind::Fight, NodeKind::Elite, NodeKind::Event, NodeKind::Rest, NodeKind::Shop, NodeKind::Chest, NodeKind::Boss] {
                p.spawn((
                    Node { column_gap: px(10), align_items: AlignItems::Center, ..default() },
                    Pickable::IGNORE,
                    children![
                        match icon(kind) {
                            Some(h) => (Node { width: px(34), height: px(34), ..default() }, ImageNode { color: node_color(kind).lighter(0.25), ..ImageNode::new(h) }, BackgroundColor(Color::NONE), Pickable::IGNORE),
                            None => (Node { width: px(22), height: px(22), border_radius: BorderRadius::MAX, ..default() }, ImageNode::solid_color(node_color(kind)), BackgroundColor(Color::NONE), Pickable::IGNORE),
                        },
                        theme::text(l10n.t(&format!("node-{kind:?}")), &fonts.body, 20.0, theme::BONE),
                    ],
                ));
            }
        });
    party_panel(&mut commands, root, run, me, &fonts, &l10n);

    // a path is a row of ink dots, as if pricked out with a pen, leaving room round the nodes
    let line = |commands: &mut Commands, a: Vec2, b: Vec2, lit: bool| {
        let d = b - a;
        let len = d.length();
        let steps = ((len - 44.0) / 12.0).floor().max(0.0) as i32;
        let (size, color) = match (lit, on_parchment) {
            (true, true) => (6.0, INK.with_alpha(0.95)),
            (false, true) => (4.5, INK.with_alpha(0.5)),
            (true, false) => (6.0, theme::BONE.with_alpha(0.85)),
            (false, false) => (4.5, theme::DIM.with_alpha(0.45)),
        };
        for k in 0..=steps {
            let p = a + d * ((22.0 + k as f32 * 12.0) / len);
            commands.spawn((
                Node { position_type: PositionType::Absolute, left: px(p.x - size / 2.0), top: px(p.y - size / 2.0), width: px(size), height: px(size), border_radius: BorderRadius::MAX, ..default() },
                BackgroundColor(color),
                Pickable::IGNORE,
                ChildOf(root),
            ));
        }
    };

    let choices = run.choices();
    let here = run.at;
    let next_floor = here.map(|(f, _)| f as usize + 1).unwrap_or(0);
    // where each choice sits, for the party's rings and the lot (see `map_party`)
    spots.0 = match run.map.floors.get(next_floor) {
        Some(floor) => choices.iter().map(|(n, _)| floor.get(*n as usize).map(|node| map_pos(next_floor, node.col)).unwrap_or(BOSS_POS)).collect(),
        None => vec![BOSS_POS],
    };
    let party = run.players.len() > 1;
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
        if let Some(h) = icon(kind) {
            // drawn in ink: places behind fade, the one the party stands on is ringed
            let ink = if passed { node_ink(kind).with_alpha(0.35) } else { node_ink(kind) };
            let side = size * if kind == NodeKind::Boss { 1.7 } else { 1.3 };
            let mut e = commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(at.x - side / 2.0),
                    top: px(at.y - side / 2.0),
                    width: px(side),
                    height: px(side),
                    border: UiRect::all(px(if standing || choice.is_some() { 3.0 } else { 0.0 })),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BorderColor::all(if standing { INK } else { theme::CANDLE.mix(&INK, 0.35) }),
                BackgroundColor(if choice.is_some() { theme::CANDLE.with_alpha(0.22) } else { Color::NONE }),
                UiTransform::IDENTITY,
                ChildOf(root),
            ));
            e.with_child((
                Node { position_type: PositionType::Absolute, left: percent(8), top: percent(8), width: percent(84), height: percent(84), ..default() },
                ImageNode { color: ink, ..ImageNode::new(h) },
                Pickable::IGNORE,
            ));
            if let Some(c) = choice {
                e.insert((Button, Hovered::default(), Btn(theme::CANDLE.with_alpha(0.22)), Beckon));
                let mine = party && votes[me as usize] == Some(c);
                e.observe(move |_: On<Activate>, mut out: ResMut<RunOutbox>, net: Option<ResMut<Net>>, mut status: ResMut<NetStatus>| {
                    if mine {
                        if let Some(mut net) = net {
                            net.node.nudge(c);
                        }
                        status.nudges.push((me, c));
                    } else {
                        out.0.push(RunAction::Vote(c));
                    }
                });
                // each player's token, in their hero's colour, along the node's foot
                let tokens: Vec<Color> = votes.iter().enumerate().filter(|(_, v)| **v == Some(c)).map(|(p, _)| theme::hero_color(&run.players[p].hero)).collect();
                e.with_children(|t| {
                    for (k, color) in tokens.into_iter().enumerate() {
                        t.spawn((
                            Node { position_type: PositionType::Absolute, left: px(-8.0 + k as f32 * 16.0), bottom: px(-8.0), width: px(16), height: px(16), border: UiRect::all(px(2)), border_radius: BorderRadius::MAX, ..default() },
                            BackgroundColor(color.lighter(0.15)),
                            BorderColor::all(INK),
                            Pickable::IGNORE,
                        ));
                    }
                });
            } else {
                e.insert(Pickable::IGNORE);
            }
            return;
        }
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
            // clicked again once chosen, the node hurries the others along
            let mine = party && votes[me as usize] == Some(c);
            e.observe(move |_: On<Activate>, mut out: ResMut<RunOutbox>, net: Option<ResMut<Net>>, mut status: ResMut<NetStatus>| {
                if mine {
                    if let Some(mut net) = net {
                        net.node.nudge(c);
                    }
                    status.nudges.push((me, c));
                } else {
                    out.0.push(RunAction::Vote(c));
                }
            });
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
    crate::between::act_backdrop(&mut commands, root, run, &art, &assets, Color::srgb(0.3, 0.3, 0.32));
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
        // the relic as itself: its picture, its name and what it does
        commands
            .spawn((
                theme::panel(Node { padding: UiRect::all(px(14)), border: UiRect::all(px(2)), column_gap: px(18), align_items: AlignItems::Center, max_width: px(1000), ..default() }),
                Pickable::IGNORE,
                ChildOf(column),
            ))
            .with_children(|p| {
                p.spawn((Node { width: px(112), height: px(112), flex_shrink: 0.0, ..default() }, art.node(&assets, &format!("relics/{relic}"), theme::PEAT), Pickable::IGNORE));
                p.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(6), ..default() }, Pickable::IGNORE)).with_children(|t| {
                    t.spawn(theme::text(l10n.t(&format!("relic-{relic}")), &fonts.title, 34.0, theme::CANDLE));
                    t.spawn((theme::text(l10n.t(&format!("relic-{relic}-desc")), &fonts.body, 24.0, theme::BONE), Node { max_width: px(820), ..default() }));
                });
            });
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
        let hints = db.0.cards.get(id).map(|def| l10n.card_glossary(&def.view(false, Form::Human))).unwrap_or_default();
        if let Some(e) = spawn_card(&mut commands, row, &card, Form::Human, None, false, (Button, Btn(frame), CardHints::new(hints)), &db.0, &l10n, &fonts, &art, &assets) {
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
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    let Some(net) = net else { return };
    let (Some(run), Some(me)) = (net.node.state.as_ref(), net.node.my_slot()) else { return };
    let Stage::Rest { done } = &run.stage else { return };
    if pick.is_changed() {
        status.run_dirty = true;
    }
    let Some(root) = redraw(&mut commands, &mut status, &old, AppState::Rest, *canvas, Color::srgb(0.09, 0.06, 0.04)) else { return };
    // the forest by firelight
    crate::between::act_backdrop(&mut commands, root, run, &art, &assets, Color::srgb(0.42, 0.27, 0.16));
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
    let open: Vec<(usize, &String, bool)> = pl.deck.iter().enumerate().filter(|(_, (id, up))| !*up && db.0.cards.get(id).is_some_and(|d| d.up.is_some())).map(|(i, (id, _))| (i, id, true)).collect();
    deck_grid(&mut commands, column, &open, RunAction::Upgrade, &db.0, &l10n, &fonts, &art, &assets);
    commands.spawn((
        menu_button(l10n.t("coop-back"), &fonts),
        observe(|_: On<Activate>, mut pick: ResMut<UpgradePick>| pick.0 = false),
        ChildOf(column),
    ));
}

// ---- the pedlar ----

/// A card at `k` of its size in a slot that size, as a button telling its keywords on hover.
#[allow(clippy::too_many_arguments)]
pub fn shown_card(commands: &mut Commands, slot: Entity, uid: usize, id: &str, up: bool, k: f32, db: &Content, l10n: &L10n, fonts: &Fonts, art: &Art, assets: &AssetServer) -> Option<Entity> {
    use crate::battle::{CARD_H, CARD_W};
    let def = db.cards.get(id)?;
    let card = Card { uid: uid as u32, def: id.to_string(), upgraded: up, free: false };
    let at = Vec2::new(-CARD_W * (1.0 - k) / 2.0, -CARD_H * (1.0 - k) / 2.0);
    let hints = l10n.card_glossary(&def.view(up, Form::Human));
    let e = spawn_card(commands, slot, &card, Form::Human, Some(at), false, (Button, Btn(crate::battle::card_frame(def)), CardHints::new(hints)), db, l10n, fonts, art, assets)?;
    commands.entity(e).insert(UiTransform { scale: Vec2::splat(k), ..default() });
    Some(e)
}

/// Cards of the deck laid out to pick one, as small as it takes for all to fit: `(index in the
/// deck, card, shown upgraded)`.
#[allow(clippy::too_many_arguments)]
fn deck_grid(commands: &mut Commands, parent: Entity, picks: &[(usize, &String, bool)], act: fn(u16) -> RunAction, db: &Content, l10n: &L10n, fonts: &Fonts, art: &Art, assets: &AssetServer) {
    use crate::battle::{CARD_H, CARD_W};
    let grid = commands
        .spawn((Node { width: px(1700), flex_wrap: FlexWrap::Wrap, justify_content: JustifyContent::Center, column_gap: px(12), row_gap: px(12), ..default() }, Pickable::IGNORE, ChildOf(parent)))
        .id();
    let k = if picks.len() <= 14 { 0.7 } else if picks.len() <= 24 { 0.56 } else { 0.46 };
    for &(i, id, up) in picks {
        let slot = commands.spawn((Node { width: px(CARD_W * k), height: px(CARD_H * k), ..default() }, Pickable::IGNORE, ChildOf(grid))).id();
        if let Some(e) = shown_card(commands, slot, i, id, up, k, db, l10n, fonts, art, assets) {
            commands.entity(e).observe(move |_: On<Activate>, mut out: ResMut<RunOutbox>| out.0.push(act(i as u16)));
        }
    }
}

/// A price as the pedlar shows it: a coin and the sum, in red when it cannot be paid.
fn price_tag(commands: &mut Commands, parent: Entity, price: i32, can_pay: bool, fonts: &Fonts) {
    commands.spawn((Node { align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: px(8), margin: UiRect::top(px(8)), ..default() }, Pickable::IGNORE, ChildOf(parent))).with_children(|t| {
        t.spawn((
            Node { width: px(20), height: px(20), border: UiRect::all(px(2)), border_radius: BorderRadius::MAX, ..default() },
            BackgroundColor(theme::CANDLE),
            BorderColor::all(Color::srgb(0.45, 0.30, 0.10)),
            Pickable::IGNORE,
        ));
        t.spawn((theme::text(price.to_string(), &fonts.num, 26.0, if can_pay { theme::CANDLE } else { theme::FRESH_BLOOD }), TextShadow { offset: Vec2::splat(2.0), color: Color::BLACK }));
    });
}

/// A ware that is not a card, laid on the counter: its picture (a disc in `tint` while it has
/// none), its name and use told on hover, its price beneath. `None` is a place sold empty.
#[allow(clippy::too_many_arguments)]
fn ware(commands: &mut Commands, parent: Entity, sold: Option<(String, String, String, i32, RunAction)>, tint: Color, can_pay: bool, fonts: &Fonts, art: &Art, assets: &AssetServer) {
    const SIZE: f32 = 116.0;
    let col = commands.spawn((Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, width: px(SIZE + 24.0), ..default() }, Pickable::IGNORE, ChildOf(parent))).id();
    let Some((picture, name, desc, price, action)) = sold else {
        commands.spawn((Node { width: px(SIZE + 12.0), height: px(SIZE + 12.0), ..default() }, Pickable::IGNORE, ChildOf(col)));
        return;
    };
    let base = if can_pay { theme::PEAT } else { theme::SOOT };
    let letters: String = name.chars().take(2).collect();
    let mut e = commands.spawn((
        Button,
        Hovered::default(),
        Btn(base),
        CardHints::new(vec![(name, desc)]),
        Node { width: px(SIZE + 12.0), height: px(SIZE + 12.0), padding: UiRect::all(px(4)), border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(14)), ..default() },
        BackgroundColor(base),
        BorderColor::all(tint),
        ChildOf(col),
    ));
    e.with_children(|b| {
        match art.image(assets, &picture) {
            Some(h) => b.spawn((Node { width: percent(100), height: percent(100), ..default() }, ImageNode::new(h), Pickable::IGNORE)),
            // until it has a picture, the first letters of its name tell one from another
            None => b.spawn((
                Node { width: percent(100), height: percent(100), border: UiRect::all(px(4)), border_radius: BorderRadius::MAX, align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
                BackgroundColor(tint.with_alpha(0.55)),
                BorderColor::all(tint),
                Pickable::IGNORE,
                children![theme::text(letters, &fonts.title, 36.0, theme::SOOT)],
            )),
        };
    });
    e.observe(move |_: On<Activate>, mut out: ResMut<RunOutbox>| out.0.push(action.clone()));
    price_tag(commands, col, price, can_pay, fonts);
}

#[allow(clippy::too_many_arguments)]
fn draw_shop(
    mut commands: Commands,
    mut status: ResMut<NetStatus>,
    net: Option<Res<Net>>,
    db: Res<Db>,
    old: Query<Entity, With<ScreenRoot>>,
    canvas: Single<Entity, With<Canvas>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    pick: Res<UpgradePick>,
    open: Res<crate::between::ShopOpen>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    use crate::battle::{CARD_H, CARD_W};
    let Some(net) = net else { return };
    let (Some(run), Some(me)) = (net.node.state.as_ref(), net.node.my_slot()) else { return };
    let Stage::Shop { stock, done } = &run.stage else { return };
    if pick.is_changed() || open.is_changed() {
        status.run_dirty = true;
    }
    let Some(root) = redraw(&mut commands, &mut status, &old, AppState::Shop, *canvas, Color::srgb(0.07, 0.08, 0.06)) else { return };
    // the party in the clearing before the pedlar; clicked, he lays out his counter over it
    crate::between::pedlar_clearing(&mut commands, root, run, &fonts, &l10n, &art, &assets);
    party_panel(&mut commands, root, run, me, &fonts, &l10n);
    let pl = &run.players[me as usize];
    if done[me as usize] {
        waiting_line(&mut commands, root, &fonts, &l10n);
        return;
    }
    if !open.0 {
        commands
            .spawn((menu_button(l10n.t("shop-leave"), &fonts), observe(|_: On<Activate>, mut out: ResMut<RunOutbox>| out.0.push(RunAction::LeaveShop)), ChildOf(root)))
            .insert(Node { position_type: PositionType::Absolute, right: px(60), bottom: px(50), width: px(300), padding: UiRect::axes(px(30), px(16)), border: UiRect::all(px(3)), justify_content: JustifyContent::Center, ..default() });
        return;
    }
    // the counter: the pedlar's cloth spread over the clearing, the wares laid on it
    let cloth = commands
        .spawn((
            Button,
            Node { position_type: PositionType::Absolute, left: px(0), top: px(crate::between::BAR_H), width: px(STAGE_W), height: px(STAGE_H - crate::between::BAR_H), ..default() },
            BackgroundColor(Color::BLACK.with_alpha(0.6)),
            observe(|_: On<Activate>, mut open: ResMut<crate::between::ShopOpen>| open.0 = false),
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        Button,
        Node { position_type: PositionType::Absolute, left: px(110), top: px(32), width: px(1700), height: px(900), border: UiRect::all(px(4)), border_radius: BorderRadius::all(px(18)), ..default() },
        match art.image(&assets, "npc/counter") {
            Some(h) => ImageNode::new(h).with_mode(bevy::ui::widget::NodeImageMode::Stretch),
            None => ImageNode::solid_color(Color::srgb(0.35, 0.10, 0.07)),
        },
        BorderColor::all(Color::srgb(0.30, 0.20, 0.12)),
        BoxShadow::new(Color::BLACK.with_alpha(0.7), px(0), px(10), px(0), px(30)),
        ChildOf(cloth),
    ));
    commands.spawn((
        theme::text(l10n.t("shop-title"), &fonts.title, 50.0, theme::CANDLE),
        TextShadow { offset: Vec2::splat(3.0), color: Color::BLACK },
        Node { position_type: PositionType::Absolute, top: px(104), width: px(STAGE_W), justify_content: JustifyContent::Center, ..default() },
        TextLayout::justify(Justify::Center),
        ChildOf(root),
    ));
    let counter = commands
        .spawn((
            Node { position_type: PositionType::Absolute, top: px(176), width: px(STAGE_W), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(22), ..default() },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    commands
        .spawn((menu_button(l10n.t("shop-close"), &fonts), observe(|_: On<Activate>, mut open: ResMut<crate::between::ShopOpen>| open.0 = false), ChildOf(root)))
        .insert(Node { position_type: PositionType::Absolute, left: px(150), bottom: px(90), width: px(300), padding: UiRect::axes(px(30), px(14)), border: UiRect::all(px(3)), justify_content: JustifyContent::Center, ..default() });
    let price = 75 + 25 * pl.removals as i32;
    if pick.0 {
        // choosing which card the pedlar takes away
        commands.spawn((theme::text(l10n.t("shop-remove-pick"), &fonts.title, 30.0, theme::BONE), ChildOf(counter)));
        price_tag(&mut commands, counter, price, pl.gold >= price, &fonts);
        let deck: Vec<(usize, &String, bool)> = pl.deck.iter().enumerate().map(|(i, (id, up))| (i, id, *up)).collect();
        deck_grid(&mut commands, counter, &deck, RunAction::RemoveCard, &db.0, &l10n, &fonts, &art, &assets);
        commands.spawn((menu_button(l10n.t("coop-back"), &fonts), observe(|_: On<Activate>, mut pick: ResMut<UpgradePick>| pick.0 = false), ChildOf(counter)));
        return;
    }
    let s = &stock[me as usize];
    // the cards lie on the counter as cards, each priced beneath; a sold one leaves its place
    let k = 0.8;
    let shelf = commands.spawn((Node { column_gap: px(26), ..default() }, Pickable::IGNORE, ChildOf(counter))).id();
    for (i, slot) in s.cards.iter().enumerate() {
        let col = commands.spawn((Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() }, Pickable::IGNORE, ChildOf(shelf))).id();
        let place = commands.spawn((Node { width: px(CARD_W * k), height: px(CARD_H * k), ..default() }, Pickable::IGNORE, ChildOf(col))).id();
        let Some((id, cost)) = slot else { continue };
        if let Some(e) = shown_card(&mut commands, place, i, id, false, k, &db.0, &l10n, &fonts, &art, &assets) {
            commands.entity(e).observe(move |_: On<Activate>, mut out: ResMut<RunOutbox>| out.0.push(RunAction::BuyCard(i as u8)));
        }
        price_tag(&mut commands, col, *cost, pl.gold >= *cost, &fonts);
    }
    // relics and potions, and the pedlar's service of taking a card away
    let wares = commands.spawn((Node { column_gap: px(22), align_items: AlignItems::FlexStart, margin: UiRect::top(px(10)), ..default() }, Pickable::IGNORE, ChildOf(counter))).id();
    for (i, slot) in s.relics.iter().enumerate() {
        let sold = slot.as_ref().map(|(id, cost)| (format!("relics/{id}"), l10n.t(&format!("relic-{id}")), l10n.t(&format!("relic-{id}-desc")), *cost, RunAction::BuyRelic(i as u8)));
        let can = slot.as_ref().is_some_and(|(_, c)| pl.gold >= *c);
        ware(&mut commands, wares, sold, theme::CANDLE, can, &fonts, &art, &assets);
    }
    commands.spawn((Node { width: px(30), ..default() }, Pickable::IGNORE, ChildOf(wares)));
    let room = pl.potions.len() < POTION_SLOTS;
    for (i, slot) in s.potions.iter().enumerate() {
        let sold = slot.as_ref().map(|(id, cost)| (format!("potions/{id}"), l10n.t(&format!("potion-{id}")), l10n.t(&format!("potion-{id}-desc")), *cost, RunAction::BuyPotion(i as u8)));
        let can = room && slot.as_ref().is_some_and(|(_, c)| pl.gold >= *c);
        ware(&mut commands, wares, sold, theme::ROTLIGHT, can, &fonts, &art, &assets);
    }
    commands.spawn((Node { width: px(30), ..default() }, Pickable::IGNORE, ChildOf(wares)));
    let service = commands.spawn((Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() }, Pickable::IGNORE, ChildOf(wares))).id();
    let base = if pl.gold >= price { theme::PEAT } else { theme::SOOT };
    commands.spawn((
        Button,
        Hovered::default(),
        Btn(base),
        Node {
            width: px(CARD_W * 0.62),
            height: px(CARD_H * 0.62),
            padding: UiRect::all(px(10)),
            border: UiRect::all(px(3)),
            border_radius: BorderRadius::all(px(12)),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: px(10),
            ..default()
        },
        BackgroundColor(base),
        BorderColor::all(theme::RUST_BLOOD),
        children![
            theme::text("\u{2716}".to_string(), &fonts.num, 44.0, theme::FRESH_BLOOD),
            (theme::text(l10n.t("shop-remove-service"), &fonts.title, 22.0, theme::BONE), TextLayout::justify(Justify::Center)),
        ],
        observe(|_: On<Activate>, mut pick: ResMut<UpgradePick>| pick.0 = true),
        ChildOf(service),
    ));
    price_tag(&mut commands, service, price, pl.gold >= price, &fonts);
}

// ---- omens ----

/// How an outcome sits with the one who chooses it, for its colour.
#[derive(Clone, Copy, PartialEq)]
enum Tone {
    Good,
    Bad,
    Plain,
}

/// What choosing `o` does, in parts, each with its tone: losses in red, gains in green.
fn describe_fx(l10n: &L10n, db: &Content, o: &EventOption) -> Vec<(String, Tone)> {
    let mut parts: Vec<(String, Tone)> = Vec::new();
    if o.price > 0 {
        parts.push((l10n.n("fx-run-pay", o.price), Tone::Bad));
    }
    let mut told = false;
    for (all, fx) in o.fx.iter().map(|f| (false, f)).chain(o.all.iter().map(|f| (true, f))) {
        if all && !told {
            told = true;
            parts.push((l10n.t("fx-run-all"), Tone::Plain));
        }
        let good = |b: bool| if b { Tone::Good } else { Tone::Bad };
        parts.push(match fx {
            RunFx::Gold(n) if *n >= 0 => (l10n.n("fx-run-gold", *n), Tone::Good),
            RunFx::Gold(n) => (l10n.n("fx-run-lose-gold", -*n), Tone::Bad),
            RunFx::Hp(n) if *n >= 0 => (l10n.n("fx-run-heal", *n), Tone::Good),
            RunFx::Hp(n) => (l10n.n("fx-run-hurt", -*n), Tone::Bad),
            RunFx::HpPct(n) if *n >= 0 => (l10n.n("fx-run-heal-pct", *n), Tone::Good),
            RunFx::HpPct(n) => (l10n.n("fx-run-hurt-pct", -*n), Tone::Bad),
            RunFx::MaxHp(n) if *n >= 0 => (l10n.n("fx-run-maxhp", *n), Tone::Good),
            RunFx::MaxHp(n) => (l10n.n("fx-run-lose-maxhp", -*n), Tone::Bad),
            // a curse pressed on the deck is no gift
            RunFx::Card(id) => (
                l10n.tf("fx-run-card", &[("c", l10n.card_name(id, false).into())]),
                good(!db.cards.get(id).is_some_and(|d| matches!(d.kind, CardType::Curse | CardType::Status))),
            ),
            RunFx::RandomCard(r) => (l10n.t(&format!("fx-run-random-{r:?}")), Tone::Good),
            RunFx::Relic => (l10n.t("fx-run-relic"), Tone::Good),
            RunFx::Potion => (l10n.t("fx-run-potion"), Tone::Good),
            RunFx::RemoveRandomCard => (l10n.t("fx-run-remove"), Tone::Good),
            RunFx::UpgradeRandomCard => (l10n.t("fx-run-upgrade"), Tone::Good),
        });
    }
    parts
}

/// The omen's picture, framed: its own when painted (`omens/<id>`), else the act's forest
/// dimmed, so the page is never a black void.
fn omen_picture(commands: &mut Commands, root: Entity, id: &str, act: u8, art: &Art, assets: &AssetServer) {
    let (image, own) = match art.image(assets, &format!("omens/{id}")) {
        Some(h) => (ImageNode::new(h), true),
        None => match art.image(assets, &format!("bg/act{}_battle", act.clamp(1, 3))).or_else(|| art.image(assets, "bg/act1_battle")) {
            Some(h) => (ImageNode::new(h).with_color(Color::srgb(0.42, 0.42, 0.48)), false),
            None => (ImageNode::solid_color(Color::srgb(0.10, 0.10, 0.12)), false),
        },
    };
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(110),
            top: px(150),
            width: px(780),
            height: px(780),
            border: UiRect::all(px(4)),
            border_radius: BorderRadius::all(px(10)),
            overflow: Overflow::clip(),
            ..default()
        },
        BorderColor::all(Color::srgb(0.42, 0.33, 0.22)),
        BoxShadow::new(Color::BLACK.with_alpha(0.8), px(0), px(10), px(6), px(30)),
        BackgroundColor(Color::BLACK),
        Pickable::IGNORE,
        ChildOf(root),
        children![(
            // a borrowed forest, wide, is seen through the frame from its middle
            if own {
                Node { width: percent(100), height: percent(100), ..default() }
            } else {
                Node { position_type: PositionType::Absolute, width: px(1380), height: percent(100), left: px(-302), ..default() }
            },
            image,
            Pickable::IGNORE,
        )],
    ));
}

#[allow(clippy::too_many_arguments)]
fn draw_omen(
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
    let Stage::Omen { id, picks } = &run.stage else { return };
    let Some(def) = db.0.events.get(id) else { return };
    let Some(root) = redraw(&mut commands, &mut status, &old, AppState::Omen, *canvas, Color::srgb(0.06, 0.06, 0.08)) else { return };
    omen_picture(&mut commands, root, id, run.act, &art, &assets);
    party_panel(&mut commands, root, run, me, &fonts, &l10n);
    let pl = &run.players[me as usize];
    let column = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(960),
                top: px(150),
                width: px(860),
                height: px(780),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                row_gap: px(16),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    commands.spawn((theme::text(l10n.t(&format!("omen-{id}")), &fonts.title, 56.0, theme::CANDLE), ChildOf(column)));
    commands.spawn((
        theme::text(l10n.t(&format!("omen-{id}-text")), &fonts.body, 26.0, theme::BONE),
        Node { max_width: px(860), margin: UiRect::bottom(px(10)), ..default() },
        ChildOf(column),
    ));
    let vote = def.kind == EventKind::Vote;
    let hint = match def.kind {
        EventKind::Vote => Some("omen-vote-hint"),
        EventKind::Sacrifice => Some("omen-sacrifice-hint"),
        EventKind::Schism if picks.len() > 1 => Some("omen-schism-hint"),
        _ => None,
    };
    if let Some(hint) = hint {
        commands.spawn((theme::text(l10n.t(hint), &fonts.body, 20.0, theme::DIM), Node { max_width: px(860), ..default() }, ChildOf(column)));
    }
    let closed = if def.kind == EventKind::Schism { nav_core::run::schism_closed(picks, me as usize) } else { None };
    let mine = picks[me as usize];
    if mine.is_some() && !vote {
        waiting_line(&mut commands, root, &fonts, &l10n);
        return;
    }
    for (i, o) in def.options.iter().enumerate() {
        let can = pl.gold >= o.price && closed != Some(i as u8);
        let chosen = mine == Some(i as u8);
        let base = if chosen { theme::SWAMP } else if can { theme::PEAT } else { theme::SOOT };
        let ink = |c: Color| if can { c } else { c.with_alpha(0.45) };
        let outcome = describe_fx(&l10n, &db.0, o);
        let mut b = commands.spawn((
            Button,
            Hovered::default(),
            Btn(base),
            Node { width: px(860), padding: UiRect::axes(px(20), px(12)), border: UiRect { left: px(8), right: px(2), top: px(2), bottom: px(2) }, align_items: AlignItems::Center, column_gap: px(14), ..default() },
            BackgroundColor(base),
            BorderColor { left: if chosen { theme::ROTLIGHT } else { theme::CANDLE }, ..BorderColor::all(Color::srgb(0.42, 0.33, 0.22)) },
            ChildOf(column),
        ));
        b.observe(move |_: On<Activate>, mut out: ResMut<RunOutbox>| out.0.push(RunAction::Choose(i as u8)));
        b.with_children(|b| {
            b.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(4), flex_grow: 1.0, ..default() }, Pickable::IGNORE)).with_children(|t| {
                t.spawn(theme::text(l10n.t(&format!("omen-{id}-{}", o.id)), &fonts.body, 24.0, ink(theme::BONE)));
                if !outcome.is_empty() {
                    t.spawn((theme::text("", &fonts.body, 20.0, theme::DIM), Node { max_width: px(760), ..default() })).with_children(|line| {
                        for (part, tone) in &outcome {
                            let color = match tone {
                                Tone::Good => theme::ROTLIGHT,
                                Tone::Bad => Color::srgb(0.85, 0.32, 0.25),
                                Tone::Plain => theme::DIM,
                            };
                            line.spawn((TextSpan::new(format!("{part} ")), TextFont { font: fonts.body.clone().into(), font_size: FontSize::Px(20.0), ..default() }, TextColor(ink(color))));
                        }
                    });
                }
            });
            // who has voted for it, a dot in each voter's colour
            if vote {
                for (seat, v) in picks.iter().enumerate() {
                    if *v == Some(i as u8) {
                        let color = run.players.get(seat).map(|p| theme::hero_color(&p.hero)).unwrap_or(theme::BONE);
                        b.spawn((
                            Node { width: px(18), height: px(18), border: UiRect::all(px(2)), border_radius: BorderRadius::MAX, flex_shrink: 0.0, ..default() },
                            BackgroundColor(color),
                            BorderColor::all(theme::BONE),
                            Pickable::IGNORE,
                        ));
                    }
                }
            }
        });
    }
}

/// The boss's trophy: three relics, one vote, and the winner goes to everyone.
fn draw_trophy(
    mut commands: Commands,
    mut status: ResMut<NetStatus>,
    net: Option<Res<Net>>,
    old: Query<Entity, With<ScreenRoot>>,
    canvas: Single<Entity, With<Canvas>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    let Some(net) = net else { return };
    let (Some(run), Some(me)) = (net.node.state.as_ref(), net.node.my_slot()) else { return };
    let Stage::Trophy { offers, picks } = &run.stage else { return };
    let Some(root) = redraw(&mut commands, &mut status, &old, AppState::Omen, *canvas, Color::srgb(0.06, 0.06, 0.08)) else { return };
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
                row_gap: px(18),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    commands.spawn((theme::text(l10n.t("trophy-title"), &fonts.title, 56.0, theme::BONE), ChildOf(column)));
    commands.spawn((theme::text(l10n.t("trophy-hint"), &fonts.body, 22.0, theme::DIM), Node { margin: UiRect::bottom(px(14)), ..default() }, ChildOf(column)));
    let mine = picks[me as usize];
    for (i, id) in offers.iter().enumerate() {
        let base = if mine == Some(i as u8) { theme::SWAMP } else { theme::PEAT };
        let mut b = commands.spawn((
            Button,
            Hovered::default(),
            Btn(base),
            Node { width: px(1100), padding: UiRect::axes(px(18), px(12)), border: UiRect::all(px(2)), align_items: AlignItems::Center, column_gap: px(18), ..default() },
            BackgroundColor(base),
            BorderColor::all(if mine == Some(i as u8) { theme::ROTLIGHT } else { Color::srgb(0.42, 0.33, 0.22) }),
            ChildOf(column),
        ));
        b.observe(move |_: On<Activate>, mut out: ResMut<RunOutbox>| out.0.push(RunAction::Choose(i as u8)));
        b.with_children(|b| {
            b.spawn((Node { width: px(96), height: px(96), flex_shrink: 0.0, ..default() }, art.node(&assets, &format!("relics/{id}"), theme::SOOT), Pickable::IGNORE));
            b.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(4), flex_grow: 1.0, ..default() }, Pickable::IGNORE)).with_children(|t| {
                t.spawn(theme::text(l10n.t(&format!("relic-{id}")), &fonts.title, 28.0, theme::CANDLE));
                t.spawn((theme::text(l10n.t(&format!("relic-{id}-desc")), &fonts.body, 22.0, theme::BONE), Node { max_width: px(820), ..default() }));
            });
            // who has voted for it, a dot in each voter's colour
            for (seat, v) in picks.iter().enumerate() {
                if *v == Some(i as u8) {
                    let color = run.players.get(seat).map(|p| theme::hero_color(&p.hero)).unwrap_or(theme::BONE);
                    b.spawn((
                        Node { width: px(18), height: px(18), border: UiRect::all(px(2)), border_radius: BorderRadius::MAX, flex_shrink: 0.0, ..default() },
                        BackgroundColor(color),
                        BorderColor::all(theme::BONE),
                        Pickable::IGNORE,
                    ));
                }
            }
        });
    }
}

// ---- the end ----

fn draw_end(
    mut commands: Commands,
    mut status: ResMut<NetStatus>,
    net: Option<ResMut<Net>>,
    mut profile: ResMut<crate::profile::Profile>,
    mut opened: Local<Option<u8>>,
    old: Query<Entity, With<ScreenRoot>>,
    canvas: Single<Entity, With<Canvas>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
) {
    let Some(mut net) = net else { return };
    let me = net.node.my_slot();
    let Some(run) = net.node.state.clone() else { return };
    let won = run.stage == Stage::Victory;
    if !net.booked {
        net.booked = true;
        // each player books the full sum on their own profile;
        // a run the autopilot played for you (dev build) earns nothing
        if let Some(pl) = me.and_then(|m| run.players.get(m as usize)).filter(|_| !net.bot_me && !net.ephemeral) {
            *opened = profile.record_run(&pl.hero, run.oberegi(), won, run.circle, run.floors_cleared, run.act);
        }
        status.run_dirty = true;
    }
    let Some(root) = redraw(&mut commands, &mut status, &old, AppState::RunEnd, *canvas, theme::SOOT) else { return };
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
            theme::text(opened.map(|c| l10n.n("end-circle-opened", c as i32)).unwrap_or_default(), &fonts.body, 28.0, theme::ROTLIGHT),
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
