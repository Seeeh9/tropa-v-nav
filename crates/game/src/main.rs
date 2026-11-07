// a released game opens no console; its log goes to a file (see `logfile`)
#![cfg_attr(not(feature = "dev"), windows_subsystem = "windows")]
//! «Тропа в Навь» — Bevy client. Rules live in `nav_core`; this crate only draws and sends actions.

mod art;
mod audio;
mod battle;
mod between;
mod book;
mod card_flip;
mod deck_fx;
#[cfg(feature = "dev")]
mod dev;
mod lan;
mod logfile;
mod l10n;
mod lobby;
mod map_ink;
mod map_party;
mod menu;
mod net;
mod preload;
mod pause;
mod profile;
mod reconnect;
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
    // playing without Steam, everyone on this machine shares one local profile
    let account = steam.as_ref().ok().map(|c| c.user().steam_id().raw()).unwrap_or(0);
    let profile = profile::Profile::load(account);
    // what the player last chose in the game, else what the config says, else the language the game
    // was installed in, else Steam's
    let installed = std::fs::read_to_string(assets.join("install_lang.txt")).ok().and_then(|word| Lang::from_word(&word));
    let lang = Lang::from_saved(profile.settings.text_lang)
        .or_else(|| config.lang.as_deref().map(|word| Lang::from_word(word).unwrap_or(Lang::Old)))
        .or(installed)
        .or_else(|| steam.as_ref().ok().map(steam::ui_lang))
        .unwrap_or(Lang::Old);
    let content = match nav_core::Content::load_dir(&assets.join("data")) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("content failed to load:\n{e}");
            std::process::exit(1);
        }
    };

    let config_lang = lang;
    logfile::catch_panics();
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(bevy::log::LogPlugin { custom_layer: logfile::layer, ..default() })
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
    .insert_resource(profile)
    .init_state::<AppState>()
    .add_systems(Startup, (theme::load_fonts, setup_stage))
    .add_systems(PostStartup, menu::apply_settings)
    .add_systems(Update, fit_stage)
    .add_plugins((menu::plugin, battle::plugin, net::plugin, lobby::plugin, run_ui::plugin, audio::plugin, story::plugin, book::plugin, deck_fx::plugin, card_flip::plugin, map_party::plugin, map_ink::plugin, between::plugin, preload::plugin))
    .add_plugins((reconnect::plugin, pause::plugin));

    #[cfg(feature = "dev")]
    app.add_plugins(dev::plugin);
    app.add_plugins(lan::plugin).add_systems(Update, (logfile::note_screens, name_window));
    info!("Tropa v Nav {} ({}), language {:?}, Steam {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS, config_lang, steam.is_ok());

    // without Steam the game still runs: alone, or with friends over a local network
    match steam {
        Ok(client) => {
            steam::install(&mut app, client);
            app.add_systems(PostStartup, |mut next: ResMut<NextState<AppState>>| next.set(AppState::Menu));
        }
        Err(e) => {
            warn!("Steam unavailable ({e}); running without it");
            app.add_systems(PostStartup, |mut next: ResMut<NextState<AppState>>| next.set(AppState::Menu));
        }
    }
    app.run();
}

/// The window goes by the game's name in the language chosen.
fn name_window(l10n: Res<L10n>, mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>) {
    if l10n.is_changed() {
        for mut window in &mut windows {
            window.title = l10n.t("game-title");
        }
    }
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
    // TROPA_WINDOW=<w>x<h> shoots at another window size, to see the stage fit it
    let resolution = std::env::var("TROPA_WINDOW")
        .ok()
        .and_then(|v| v.split_once('x').and_then(|(w, h)| Some(WindowResolution::new(w.parse().ok()?, h.parse().ok()?))))
        .unwrap_or(window.resolution.clone());
    Window {
        resolution,
        focused: false,
        window_level: bevy::window::WindowLevel::AlwaysOnBottom,
        position: WindowPosition::At(IVec2::new(-6000, -6000)),
        skip_taskbar: true,
        ..window
    }
}
