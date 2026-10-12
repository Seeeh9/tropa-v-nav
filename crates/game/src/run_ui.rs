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
        .add_systems(Update, draw_map.run_if(in_state(AppState::Map)))
        .add_systems(Update, draw_reward.run_if(in_state(AppState::Reward)))
        .add_systems(Update, draw_rest.run_if(in_state(AppState::Rest)))
        .add_systems(Update, draw_shop.run_if(in_state(AppState::Shop)))
        .add_systems(Update, draw_omen.run_if(in_state(AppState::Omen)))
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

// ---- the pedlar ----

/// A priced line on the pedlar's shelf.
fn shelf_button(commands: &mut Commands, parent: Entity, label: String, price: i32, can_pay: bool, edge: Color, fonts: &Fonts, action: RunAction) {
    let base = if can_pay { theme::PEAT } else { theme::SOOT };
    let mut e = commands.spawn((
        Button,
        Hovered::default(),
        Btn(base),
        Node { width: px(560), padding: UiRect::axes(px(12), px(8)), border: UiRect::all(px(2)), justify_content: JustifyContent::SpaceBetween, column_gap: px(12), ..default() },
        BackgroundColor(base),
        BorderColor::all(edge),
        children![
            (theme::text(label, &fonts.body, 18.0, if can_pay { theme::BONE } else { theme::DIM }), Node { max_width: px(440), ..default() }),
            theme::text(price.to_string(), &fonts.num, 22.0, if can_pay { theme::CANDLE } else { theme::RUST_BLOOD }),
        ],
        ChildOf(parent),
    ));
    e.observe(move |_: On<Activate>, mut out: ResMut<RunOutbox>| out.0.push(action.clone()));
}

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
) {
    let Some(net) = net else { return };
    let (Some(run), Some(me)) = (net.node.state.as_ref(), net.node.my_slot()) else { return };
    let Stage::Shop { stock, done } = &run.stage else { return };
    if pick.is_changed() {
        status.run_dirty = true;
    }
    let Some(root) = redraw(&mut commands, &mut status, &old, AppState::Shop, *canvas, Color::srgb(0.07, 0.08, 0.06)) else { return };
    party_panel(&mut commands, root, run, me, &fonts, &l10n);
    let pl = &run.players[me as usize];
    commands.spawn((
        theme::text(l10n.t("shop-title"), &fonts.title, 56.0, theme::CANDLE),
        Node { position_type: PositionType::Absolute, top: px(90), width: px(STAGE_W), justify_content: JustifyContent::Center, ..default() },
        TextLayout::justify(Justify::Center),
        ChildOf(root),
    ));
    if done[me as usize] {
        waiting_line(&mut commands, root, &fonts, &l10n);
        return;
    }
    let columns = commands
        .spawn((
            Node { position_type: PositionType::Absolute, top: px(190), width: px(STAGE_W), justify_content: JustifyContent::Center, column_gap: px(30), ..default() },
            Pickable::IGNORE,
            ChildOf(root),
        ))
        .id();
    let column = |commands: &mut Commands, title: &str| -> Entity {
        commands
            .spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(8), ..default() }, Pickable::IGNORE, ChildOf(columns), children![theme::text(l10n.t(title), &fonts.title, 26.0, theme::BONE)]))
            .id()
    };
    let s = &stock[me as usize];
    if pick.0 {
        // choosing which card the pedlar takes away
        let price = 75 + 25 * pl.removals as i32;
        let col = column(&mut commands, "shop-remove-pick");
        let grid = commands.spawn((Node { width: px(1200), flex_wrap: FlexWrap::Wrap, column_gap: px(8), row_gap: px(8), ..default() }, Pickable::IGNORE, ChildOf(col))).id();
        for (i, (id, up)) in pl.deck.iter().enumerate() {
            shelf_button(&mut commands, grid, l10n.card_name(id, *up), price, pl.gold >= price, theme::DIM, &fonts, RunAction::RemoveCard(i as u16));
        }
        commands.spawn((menu_button(l10n.t("coop-back"), &fonts), observe(|_: On<Activate>, mut pick: ResMut<UpgradePick>| pick.0 = false), ChildOf(col)));
        return;
    }
    let col = column(&mut commands, "shop-cards");
    for (i, slot) in s.cards.iter().enumerate() {
        let Some((id, price)) = slot else { continue };
        let Some(def) = db.0.cards.get(id) else { continue };
        let label = format!("{}  ·  {}", l10n.card_name(id, false), l10n.card_text(id, def, &def.view(false, Form::Human)));
        let edge = if def.hero.is_empty() { theme::DIM } else { theme::hero_color(&def.hero) };
        shelf_button(&mut commands, col, label, *price, pl.gold >= *price, edge, &fonts, RunAction::BuyCard(i as u8));
    }
    let col = column(&mut commands, "shop-wares");
    for (i, slot) in s.relics.iter().enumerate() {
        let Some((id, price)) = slot else { continue };
        let label = format!("{}  ·  {}", l10n.t(&format!("relic-{id}")), l10n.t(&format!("relic-{id}-desc")));
        shelf_button(&mut commands, col, label, *price, pl.gold >= *price, theme::CANDLE, &fonts, RunAction::BuyRelic(i as u8));
    }
    let room = pl.potions.len() < POTION_SLOTS;
    for (i, slot) in s.potions.iter().enumerate() {
        let Some((id, price)) = slot else { continue };
        let label = format!("{}  ·  {}", l10n.t(&format!("potion-{id}")), l10n.t(&format!("potion-{id}-desc")));
        shelf_button(&mut commands, col, label, *price, room && pl.gold >= *price, theme::ROTLIGHT, &fonts, RunAction::BuyPotion(i as u8));
    }
    let price = 75 + 25 * pl.removals as i32;
    commands.spawn((
        menu_button(l10n.n("shop-remove", price), &fonts),
        observe(|_: On<Activate>, mut pick: ResMut<UpgradePick>| pick.0 = true),
        ChildOf(col),
    ));
    commands.spawn((
        menu_button(l10n.t("shop-leave"), &fonts),
        observe(|_: On<Activate>, mut out: ResMut<RunOutbox>| out.0.push(RunAction::LeaveShop)),
        ChildOf(col),
    ));
}

// ---- omens ----

fn describe_fx(l10n: &L10n, o: &EventOption) -> String {
    let mut parts: Vec<String> = Vec::new();
    if o.price > 0 {
        parts.push(l10n.n("fx-run-pay", o.price));
    }
    for fx in &o.fx {
        parts.push(match fx {
            RunFx::Gold(n) if *n >= 0 => l10n.n("fx-run-gold", *n),
            RunFx::Gold(n) => l10n.n("fx-run-lose-gold", -*n),
            RunFx::Hp(n) if *n >= 0 => l10n.n("fx-run-heal", *n),
            RunFx::Hp(n) => l10n.n("fx-run-hurt", -*n),
            RunFx::HpPct(n) if *n >= 0 => l10n.n("fx-run-heal-pct", *n),
            RunFx::HpPct(n) => l10n.n("fx-run-hurt-pct", -*n),
            RunFx::MaxHp(n) => l10n.n("fx-run-maxhp", *n),
            RunFx::Card(id) => l10n.tf("fx-run-card", &[("c", l10n.card_name(id, false).into())]),
            RunFx::RandomCard(r) => l10n.t(&format!("fx-run-random-{r:?}")),
            RunFx::Relic => l10n.t("fx-run-relic"),
            RunFx::Potion => l10n.t("fx-run-potion"),
            RunFx::RemoveRandomCard => l10n.t("fx-run-remove"),
            RunFx::UpgradeRandomCard => l10n.t("fx-run-upgrade"),
        });
    }
    parts.join(" ")
}

fn draw_omen(
    mut commands: Commands,
    mut status: ResMut<NetStatus>,
    net: Option<Res<Net>>,
    db: Res<Db>,
    old: Query<Entity, With<ScreenRoot>>,
    canvas: Single<Entity, With<Canvas>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
) {
    let Some(net) = net else { return };
    let (Some(run), Some(me)) = (net.node.state.as_ref(), net.node.my_slot()) else { return };
    let Stage::Omen { id, picks } = &run.stage else { return };
    let Some(def) = db.0.events.get(id) else { return };
    let Some(root) = redraw(&mut commands, &mut status, &old, AppState::Omen, *canvas, Color::srgb(0.06, 0.06, 0.08)) else { return };
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
    commands.spawn((theme::text(l10n.t(&format!("omen-{id}")), &fonts.title, 56.0, theme::BONE), ChildOf(column)));
    commands.spawn((
        theme::text(l10n.t(&format!("omen-{id}-text")), &fonts.body, 26.0, theme::BONE),
        Node { max_width: px(1100), margin: UiRect::bottom(px(14)), ..default() },
        TextLayout::justify(Justify::Center),
        ChildOf(column),
    ));
    let vote = def.kind == EventKind::Vote;
    if vote {
        commands.spawn((theme::text(l10n.t("omen-vote-hint"), &fonts.body, 20.0, theme::DIM), ChildOf(column)));
    }
    let mine = picks[me as usize];
    if mine.is_some() && !vote {
        waiting_line(&mut commands, root, &fonts, &l10n);
        return;
    }
    for (i, o) in def.options.iter().enumerate() {
        let can = pl.gold >= o.price;
        let voters = picks.iter().filter(|v| **v == Some(i as u8)).count();
        let mut label = format!("{}  —  {}", l10n.t(&format!("omen-{id}-{}", o.id)), describe_fx(&l10n, o));
        if vote && voters > 0 {
            label.push_str(&format!("   [{voters}]"));
        }
        let base = if mine == Some(i as u8) { theme::SWAMP } else if can { theme::PEAT } else { theme::SOOT };
        commands.spawn((
            Button,
            Hovered::default(),
            Btn(base),
            Node { width: px(1000), padding: UiRect::axes(px(18), px(12)), border: UiRect::all(px(2)), ..default() },
            BackgroundColor(base),
            BorderColor::all(Color::srgb(0.42, 0.33, 0.22)),
            children![theme::text(label, &fonts.body, 22.0, if can { theme::BONE } else { theme::DIM })],
            observe(move |_: On<Activate>, mut out: ResMut<RunOutbox>| out.0.push(RunAction::Choose(i as u8))),
            ChildOf(column),
        ));
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
        if let Some(pl) = me.and_then(|m| run.players.get(m as usize)).filter(|_| !net.bot_me) {
            *opened = profile.record_run(&pl.hero, run.oberegi(), won, run.circle);
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
