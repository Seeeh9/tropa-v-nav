//! Main menu and the "start Steam" wall.

use crate::art::Art;
use crate::battle::Btn;
use crate::l10n::L10n;
use crate::session::Db;
use crate::theme::{self, Fonts};
use crate::{AppState, Stage, STAGE_H, STAGE_W};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};

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

fn enter_menu(
    mut commands: Commands,
    profile: Res<crate::profile::Profile>,
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
        DespawnOnExit(AppState::Menu),
        theme::text(
            l10n.tf("menu-profile", &[("oberegi", (profile.oberegi as i32).into()), ("wins", (profile.wins as i32).into()), ("runs", (profile.runs as i32).into()), ("circle", (profile.circle_unlocked as i32).into())]),
            &fonts.body,
            22.0,
            theme::DIM,
        ),
        Node { position_type: PositionType::Absolute, left: px(24), bottom: px(20), ..default() },
        ChildOf(*stage),
    ));
    // a solo run left unfinished can be picked up again
    if let Some(run) = profile.load_run() {
        let label = l10n.tf("menu-continue", &[("hero", l10n.t(&format!("hero-{}", run.players[0].hero)).into()), ("act", (run.act as i32).into())]);
        commands.spawn((
            DespawnOnExit(AppState::Menu),
            menu_button(label, &fonts),
            observe(|_: On<Activate>, mut commands: Commands, profile: Res<crate::profile::Profile>, steam: Option<Res<crate::steam::Steam>>, l10n: Res<L10n>| {
                if let Some(run) = profile.load_run() {
                    let name = steam.map(|s| s.client.friends().name()).unwrap_or_else(|| l10n.t("lobby-wanderer"));
                    // the stage router opens whichever screen the run was left on
                    crate::net::resume_solo(&mut commands, &name, run);
                }
            }),
            ChildOf(*stage),
        ));
    }
    commands.spawn((
        screen(AppState::Menu, *stage),
        children![
            theme::text(l10n.t("game-title"), &fonts.title, 120.0, theme::BONE),
            (theme::text(l10n.t("game-subtitle"), &fonts.body, 30.0, theme::CANDLE), Node { margin: UiRect::bottom(px(40)), ..default() }),
            (
                menu_button(l10n.t("menu-solo"), &fonts),
                observe(|_: On<Activate>, mut commands: Commands, db: Res<Db>, steam: Option<Res<crate::steam::Steam>>, l10n: Res<L10n>, mut next: ResMut<NextState<AppState>>| {
                    // alone is just a party of one: same lobby, same rules, no Steam traffic
                    let name = steam.map(|s| s.client.friends().name()).unwrap_or_else(|| l10n.t("lobby-wanderer"));
                    crate::net::start_offline(&mut commands, &db.0, &name, 0);
                    next.set(AppState::Lobby);
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
