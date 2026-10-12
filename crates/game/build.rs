//! Puts `steam_api64.dll` next to the game's exe. The Steamworks crate builds
//! it into its own output folder, where a double-clicked exe would not find it.

use std::path::{Path, PathBuf};

fn main() {
    // OUT_DIR is target/<profile>/build/tropa-<hash>/out
    let Some(out) = std::env::var_os("OUT_DIR").map(PathBuf::from) else { return };
    let Some(profile_dir) = out.ancestors().nth(3).map(Path::to_path_buf) else { return };
    let build_dir = profile_dir.join("build");
    let Ok(entries) = std::fs::read_dir(&build_dir) else { return };
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with("steamworks-sys-") {
            continue;
        }
        for name in ["steam_api64.dll", "steam_api.dll", "libsteam_api.so", "libsteam_api.dylib"] {
            let from = entry.path().join("out").join(name);
            if from.is_file() {
                let to = profile_dir.join(name);
                // only when missing or stale, so an unchanged build stays quiet
                let stale = std::fs::metadata(&to).map(|m| m.len()).ok() != std::fs::metadata(&from).map(|m| m.len()).ok();
                if stale {
                    if let Err(e) = std::fs::copy(&from, &to) {
                        println!("cargo:warning=could not copy {name} next to the exe: {e}");
                    }
                }
            }
        }
    }
}
