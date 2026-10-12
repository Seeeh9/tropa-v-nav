//! Development-only helpers, compiled in with `--features dev`.
//!
//!   TROPA_SCREEN=coop|village|settings|lobby|run|host  open the party menu, the village, settings, an offline lobby, a run in progress, or a real Steam lobby
//!   TROPA_BOTS=<0..3>           bot companions for `lobby` and `run`
//!   TROPA_HERO=<id>             the local hero for `run`
//!   TROPA_BOT_ME=1              the local seat is played by the autopilot too
//!   TROPA_FORCE=shop|omen|rest  open `run` straight at that stage
//!   TROPA_SEED=<n>              fixed seed for that run
//!   TROPA_SHOT=<file.png>       save a screenshot after TROPA_SHOT_AT seconds (default 3) and quit

use crate::session::Db;
use crate::AppState;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Menu), open_screen).add_systems(Update, shot);
}

fn env<T: std::str::FromStr>(name: &str) -> Option<T> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}

fn open_screen(
    mut commands: Commands,
    db: Res<Db>,
    steam: Option<Res<crate::steam::Steam>>,
    inbox: Option<Res<crate::net::Inbox>>,
    mut status: ResMut<crate::net::NetStatus>,
    mut next: ResMut<NextState<AppState>>,
    mut done: Local<bool>,
) {
    let Ok(screen) = std::env::var("TROPA_SCREEN") else { return };
    if *done {
        return;
    }
    *done = true;
    let bots = env::<usize>("TROPA_BOTS").unwrap_or(0).min(3);
    match screen.as_str() {
        "coop" => next.set(AppState::Coop),
        "village" => next.set(AppState::Village),
        "settings" => next.set(AppState::Settings),
        "lobby" => {
            crate::net::start_offline(&mut commands, &db.0, "dev", bots);
            next.set(AppState::Lobby);
        }
        // straight into a run over the real protocol; the stage router opens the right screen
        "run" => {
            let seed = env("TROPA_SEED").unwrap_or(7);
            let bot_me = std::env::var("TROPA_BOT_ME").is_ok();
            let force = match std::env::var("TROPA_FORCE").as_deref() {
                Ok("shop") => Some(nav_core::NodeKind::Shop),
                Ok("omen") => Some(nav_core::NodeKind::Event),
                Ok("rest") => Some(nav_core::NodeKind::Rest),
                _ => None,
            };
            crate::net::start_offline_run(&mut commands, &db.0, bots, seed, std::env::var("TROPA_HERO").ok(), bot_me, force);
        }
        "host" => {
            if let (Some(steam), Some(inbox)) = (steam, inbox) {
                crate::net::create_lobby(&steam, &mut status, &inbox);
            }
        }
        _ => {}
    }
}

fn shot(mut commands: Commands, time: Res<Time>, mut taken: Local<Option<f32>>, mut exit: MessageWriter<AppExit>) {
    let Ok(path) = std::env::var("TROPA_SHOT") else { return };
    let at = env::<f32>("TROPA_SHOT_AT").unwrap_or(3.0);
    let now = time.elapsed_secs();
    match *taken {
        None if now >= at => {
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
            *taken = Some(now);
        }
        // give the capture a moment to reach the disk
        Some(t) if now - t > 1.0 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
