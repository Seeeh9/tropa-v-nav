//! «Тропа в Навь» — Bevy client. Rules live in `nav_core`; this crate only draws and sends actions.

mod art;
mod audio;
mod battle;
mod between;
mod book;
#[cfg(feature = "dev")]
mod deck_fx;
mod dev;
#[cfg(feature = "dev")]
mod lan;
mod l10n;
mod lobby;
mod map_party;
mod menu;
mod net;
mod profile;
mod run_ui;
mod session;
mod steam;
mod story;
mod theme;

use bevy::prelude::*;
use bevy::window::{WindowPosition, WindowResolution};
use l10n::{L10n, Lang};

/// The whole UI is laid out on a fixed canvas and scaled to the window.
pub const STAGE_W: f32 = 1920.0;
pub const STAGE_H: f32 = 1080.0;

#[derive(States, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum AppState {
    #[default]
    Boot,
    NoSteam,
    Menu,
    Settings,
    /// The last stockade: spend village currency between runs.
    Village,
    /// The graveyard's chronicle: enemies met and runs past.
    Chronicle,
    /// Create or join a party.
    Coop,
    /// Seats, heroes, ready.
    Lobby,
    /// The path map: vote for the next node.
    Map,
    Battle,
    Reward,
    /// The fire.
    Rest,
    /// The pedlar.
    Shop,
    Omen,
    RunEnd,
}

/// Parent of every screen: a 1920x1080 node centred in the window.
#[derive(Component)]
pub struct Stage;

#[derive(Resource, serde::Deserialize, Clone)]
pub struct Config {
    /// 480 is Valve's Spacewar test app; swap for the real id once the game has one.
    pub steam_app_id: u32,
    #[serde(default)]
    pub lang: Option<String>,
}

fn load_config(assets: &std::path::Path) -> Config {
    std::fs::read_to_string(assets.join("config.ron"))
        .ok()
        .and_then(|s| ron::from_str(&s).ok())
        .unwrap_or(Config { steam_app_id: 480, lang: None })
}

fn main() {
    let assets = art::assets_dir();
    let config = load_config(&assets);
    let steam = steam::init(config.steam_app_id);
    let lang = match config.lang.as_deref() {
        Some("en") => Lang::En,
        Some("ru") => Lang::Ru,
        Some(_) => Lang::Old,
        None => steam.as_ref().ok().map(steam::ui_lang).unwrap_or(Lang::Old),
    };
    // offline dev builds share one local profile
    let account = steam.as_ref().ok().map(|c| c.user().steam_id().raw()).unwrap_or(0);
    let content = match nav_core::Content::load_dir(&assets.join("data")) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("content failed to load:\n{e}");
            std::process::exit(1);
        }
    };

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(test_window(Window {
                    title: "Тропа в Навь".into(),
                    resolution: WindowResolution::new(1600, 900),
                    ..default()
                })),
                ..default()
            })
            .set(AssetPlugin { file_path: assets.to_string_lossy().into_owned(), ..default() }),
    )
    .insert_resource(ClearColor(theme::SOOT))
    .insert_resource(L10n::load(&assets, lang))
    .insert_resource(art::Art::new(assets.clone()))
    .insert_resource(session::Db(content))
    .insert_resource(config)
    .insert_resource(profile::Profile::load(account))
    .init_state::<AppState>()
    .add_systems(Startup, (theme::load_fonts, setup_stage))
    .add_systems(PostStartup, menu::apply_settings)
    .add_systems(Update, fit_stage)
    .add_plugins((menu::plugin, battle::plugin, net::plugin, lobby::plugin, run_ui::plugin, audio::plugin, story::plugin, book::plugin, deck_fx::plugin, map_party::plugin, between::plugin));

    #[cfg(feature = "dev")]
    app.add_plugins((dev::plugin, lan::plugin));

    // Without Steam there is no game: only the "start Steam" screen. The `dev` build may run offline.
    let offline_ok = cfg!(feature = "dev");
    match steam {
        Ok(client) => {
            steam::install(&mut app, client);
            app.add_systems(PostStartup, |mut next: ResMut<NextState<AppState>>| next.set(AppState::Menu));
        }
        Err(e) if offline_ok => {
            warn!("Steam unavailable ({e}); running offline because this is a dev build");
            app.add_systems(PostStartup, |mut next: ResMut<NextState<AppState>>| next.set(AppState::Menu));
        }
        Err(e) => {
            error!("Steam unavailable: {e}");
            app.add_systems(PostStartup, |mut next: ResMut<NextState<AppState>>| next.set(AppState::NoSteam));
        }
    }
    app.run();
}

fn setup_stage(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            overflow: Overflow::clip(),
            ..default()
        },
        Pickable::IGNORE,
        children![(
            Stage,
            Node { width: px(STAGE_W), height: px(STAGE_H), flex_shrink: 0.0, ..default() },
            Pickable::IGNORE,
        )],
    ));
}

fn fit_stage(window: Single<&Window>, mut scale: ResMut<UiScale>) {
    let s = (window.width() / STAGE_W).min(window.height() / STAGE_H).max(0.1);
    if (scale.0 - s).abs() > 1e-4 {
        scale.0 = s;
    }
}

/// Cursor position in stage coordinates.
pub fn stage_cursor(window: &Window, scale: f32) -> Option<Vec2> {
    Some(stage_point(window, scale, window.cursor_position()?))
}

/// A point in the window (logical pixels) in stage coordinates.
pub fn stage_point(window: &Window, scale: f32, at: Vec2) -> Vec2 {
    let origin = Vec2::new(window.width() - STAGE_W * scale, window.height() - STAGE_H * scale) / 2.0;
    (at - origin) / scale
}

/// A dev screenshot run (`TROPA_SHOT`) opens its window out of the way: unfocused, beneath every
/// other window and off the screen, so a game being played meanwhile is never covered or
/// minimised. `TROPA_SHOW=1` shows it as usual.
fn test_window(window: Window) -> Window {
    let hidden = cfg!(feature = "dev") && std::env::var("TROPA_SHOT").is_ok() && std::env::var("TROPA_SHOW").is_err();
    if !hidden {
        return window;
    }
    Window {
        focused: false,
        window_level: bevy::window::WindowLevel::AlwaysOnBottom,
        position: WindowPosition::At(IVec2::new(-6000, -6000)),
        skip_taskbar: true,
        ..window
    }
}
