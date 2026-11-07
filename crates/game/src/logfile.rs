//! A log kept on disk in every build, to be sent along with a bug report: what the game did, in
//! words, and how it died if it did. One file a start in `logs` under the player's folder (see `platform::user_dir`), the newest few kept.

use bevy::log::tracing_subscriber::{self, Layer};
use bevy::log::BoxedLayer;
use bevy::prelude::*;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// How many starts' logs are kept.
const KEEP: usize = 15;

static FILE: OnceLock<PathBuf> = OnceLock::new();

/// Where the logs lie.
pub fn dir() -> PathBuf {
    crate::platform::user_dir().unwrap_or_else(|| std::env::temp_dir().join("TropaVNav")).join("logs")
}

fn open() -> Option<std::fs::File> {
    let dir = dir();
    std::fs::create_dir_all(&dir).ok()?;
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let path = dir.join(format!("tropa-{secs}.log"));
    let file = std::fs::File::create(&path).ok()?;
    let _ = FILE.set(path);
    // the oldest make room
    if let Ok(read) = std::fs::read_dir(&dir) {
        let mut old: Vec<PathBuf> = read.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "log")).collect();
        old.sort();
        for p in old.iter().rev().skip(KEEP) {
            let _ = std::fs::remove_file(p);
        }
    }
    Some(file)
}

/// The layer that writes the log, for `LogPlugin::custom_layer`.
pub fn layer(_: &mut App) -> Option<BoxedLayer> {
    let file = open()?;
    let filter = tracing_subscriber::EnvFilter::new("info,wgpu=error,naga=warn,tropa=debug");
    Some(tracing_subscriber::fmt::layer().with_ansi(false).with_writer(Mutex::new(file)).with_filter(filter).boxed())
}

/// A crash goes in the log too, since a released game has no console to die on.
pub fn catch_panics() {
    let plain = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(path) = FILE.get() {
            if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(path) {
                let _ = writeln!(f, "PANIC: {info}\n{}", std::backtrace::Backtrace::force_capture());
            }
        }
        plain(info);
    }));
}

/// Opens the folder of logs for the player, to send one.
pub fn show_folder() {
    let dir = dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Err(e) = std::process::Command::new("explorer").arg(&dir).spawn() {
        warn!("cannot open the folder of logs: {e}");
    }
}

/// Which screen the game went to, as the first thing to read in a report.
pub fn note_screens(mut changes: MessageReader<StateTransitionEvent<crate::AppState>>) {
    for c in changes.read() {
        info!("screen: {:?} -> {:?}", c.exited, c.entered);
    }
}
