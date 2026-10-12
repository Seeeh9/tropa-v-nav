//! Steamworks glue. The overlay needs nothing from us beyond a successful
//! `SteamAPI_Init`: Steam injects it into the process and hooks the swapchain.

use crate::l10n::Lang;
use bevy::prelude::*;
use steamworks::Client;

#[derive(Resource, Clone)]
pub struct Steam {
    pub client: Client,
}

pub fn init(app_id: u32) -> Result<Client, String> {
    Client::init_app(app_id).map_err(|e| e.to_string())
}

pub fn ui_lang(client: &Client) -> Lang {
    match client.utils().ui_language().as_str() {
        "russian" | "ukrainian" => Lang::Ru,
        _ => Lang::En,
    }
}

pub fn install(app: &mut App, client: Client) {
    info!("Steam: signed in as {} ({})", client.friends().name(), client.user().steam_id().raw());
    app.insert_resource(Steam { client }).add_systems(PreUpdate, pump);
}

fn pump(steam: Res<Steam>, time: Res<Time>, mut reported: Local<bool>) {
    steam.client.run_callbacks();
    // the overlay hooks in a moment after the first frames are presented
    if !*reported && time.elapsed_secs() > 4.0 {
        *reported = true;
        info!("Steam overlay enabled: {}", steam.client.utils().is_overlay_enabled());
    }
}
