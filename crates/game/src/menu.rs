//! Main menu and the "start Steam" wall.

use crate::art::Art;
use crate::battle::Btn;
use crate::l10n::L10n;
use crate::session::{Db, PendingBattle};
use crate::theme::{self, Fonts};
use crate::{AppState, Stage, STAGE_H, STAGE_W};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};
use nav_core::*;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Menu), enter_menu)
        .add_systems(OnEnter(AppState::NoSteam), enter_no_steam)
        .add_systems(Update, crate::battle::style_buttons.run_if(not(in_state(AppState::Battle))));
}

fn screen(state: AppState, stage: Entity) -> impl Bundle {
    (
        DespawnOnExit(state),
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
        ChildOf(stage),
    )
}

pub fn menu_button(label: String, fonts: &Fonts) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        Btn(theme::PEAT),
        Node {
            width: px(520),
            padding: UiRect::axes(px(30), px(16)),
            border: UiRect::all(px(3)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(theme::PEAT),
        BorderColor::all(Color::srgb(0.42, 0.33, 0.22)),
        children![theme::text(label, &fonts.title, 32.0, theme::BONE)],
    )
}

/// Picks a fight for a lone hero: any normal encounter of the first act.
pub fn solo_battle(db: &Content, hero: &str, seed: u64) -> Option<PendingBattle> {
    let mut rng = Rng::new(seed);
    let pool: Vec<&EncounterDef> = db.encounters.values().filter(|e| e.act == 1 && e.kind == EncounterKind::Normal).collect();
    let enc = rng.pick(&pool)?;
    Some(PendingBattle { party: vec![HeroSetup::fresh(db, hero)?], enemies: enc.enemies.clone(), seed, me: 0, bots: vec![] })
}

fn now_seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1)
}

fn enter_menu(
    mut commands: Commands,
    stage: Single<Entity, With<Stage>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    commands.spawn((
        DespawnOnExit(AppState::Menu),
        Node { position_type: PositionType::Absolute, width: px(STAGE_W), height: px(STAGE_H), ..default() },
        art.node(&assets, "bg/menu", theme::SOOT),
        Pickable::IGNORE,
        ChildOf(*stage),
    ));
    commands.spawn((
        screen(AppState::Menu, *stage),
        children![
            theme::text(l10n.t("game-title"), &fonts.title, 120.0, theme::BONE),
            (theme::text(l10n.t("game-subtitle"), &fonts.body, 30.0, theme::CANDLE), Node { margin: UiRect::bottom(px(40)), ..default() }),
            (
                menu_button(l10n.t("menu-solo"), &fonts),
                observe(|_: On<Activate>, mut commands: Commands, db: Res<Db>, mut next: ResMut<NextState<AppState>>| {
                    if let Some(b) = solo_battle(&db.0, "bogatyr", now_seed()) {
                        commands.insert_resource(b);
                        next.set(AppState::Battle);
                    }
                }),
            ),
            (
                menu_button(l10n.t("menu-coop"), &fonts),
                observe(|_: On<Activate>, mut next: ResMut<NextState<AppState>>| next.set(AppState::Coop)),
            ),
            (
                menu_button(l10n.t("menu-lang"), &fonts),
                observe(|_: On<Activate>, mut l10n: ResMut<L10n>, art: Res<Art>, mut next: ResMut<NextState<AppState>>| {
                    *l10n = L10n::load(art.root(), l10n.lang.other());
                    // re-enter the menu so every label is rebuilt in the new language
                    next.set(AppState::Menu);
                }),
            ),
            (
                menu_button(l10n.t("menu-quit"), &fonts),
                observe(|_: On<Activate>, mut exit: MessageWriter<AppExit>| {
                    exit.write(AppExit::Success);
                }),
            ),
        ],
    ));
}

fn enter_no_steam(mut commands: Commands, stage: Single<Entity, With<Stage>>, fonts: Res<Fonts>, l10n: Res<L10n>) {
    commands.spawn((
        screen(AppState::NoSteam, *stage),
        children![
            theme::text(l10n.t("steam-required-title"), &fonts.title, 96.0, theme::FRESH_BLOOD),
            (
                theme::text(l10n.t("steam-required-body"), &fonts.body, 30.0, theme::BONE),
                TextLayout::justify(Justify::Center),
                Node { max_width: px(1100), margin: UiRect::vertical(px(30)), ..default() },
            ),
            (
                menu_button(l10n.t("steam-required-quit"), &fonts),
                observe(|_: On<Activate>, mut exit: MessageWriter<AppExit>| {
                    exit.write(AppExit::Success);
                }),
            ),
        ],
    ));
}
