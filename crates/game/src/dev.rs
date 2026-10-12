//! Development-only helpers, compiled in with `--features dev`.
//!
//!   TROPA_AUTOSTART=<players>   skip the menu, fight at once; extra heroes are bots
//!   TROPA_SEED=<n>              fixed seed for that fight
//!   TROPA_SHOT=<file.png>       save a screenshot after TROPA_SHOT_AT seconds (default 3) and quit

use crate::session::{Db, PendingBattle};
use crate::AppState;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Menu), autostart).add_systems(Update, shot);
}

fn autostart(mut commands: Commands, db: Res<Db>, mut next: ResMut<NextState<AppState>>, mut done: Local<bool>) {
    let Some(players) = std::env::var("TROPA_AUTOSTART").ok().and_then(|v| v.parse::<u8>().ok()) else { return };
    if *done {
        return;
    }
    *done = true;
    let seed = std::env::var("TROPA_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(7);
    let Some(mut battle) = crate::menu::solo_battle(&db.0, "bogatyr", seed) else { return };
    let players = players.clamp(1, 4);
    let hero = battle.party[0].clone();
    battle.party = (0..players).map(|_| hero.clone()).collect();
    battle.bots = (1..players).collect();
    commands.insert_resource::<PendingBattle>(battle);
    next.set(AppState::Battle);
}

fn shot(mut commands: Commands, time: Res<Time>, mut taken: Local<Option<f32>>, mut exit: MessageWriter<AppExit>) {
    let Ok(path) = std::env::var("TROPA_SHOT") else { return };
    let at = std::env::var("TROPA_SHOT_AT").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(3.0);
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
