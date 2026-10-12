//! What a player keeps between runs: village currency, unlocked Circles of Nav,
//! hero experience. Stored per Steam account under the user's app-data folder,
//! in a plain RON file so that Steam Cloud can pick it up later.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    /// Village currency (Обереги).
    pub oberegi: u32,
    /// Highest Circle of Nav the player may choose; a win on it opens the next.
    pub circle_unlocked: u8,
    pub hero_xp: BTreeMap<String, u32>,
    pub runs: u32,
    pub wins: u32,
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl Profile {
    /// Loads the profile for this account, or starts a fresh one.
    pub fn load(account: u64) -> Profile {
        let path = profile_dir().map(|d| d.join(format!("{account}.ron")));
        let mut profile: Profile = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| ron::from_str(&s).ok())
            .unwrap_or_default();
        profile.path = path;
        profile
    }

    pub fn save(&self) {
        let Some(path) = &self.path else { return };
        let Ok(text) = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // write beside and rename, so a crash mid-write can't leave half a file
        let tmp = path.with_extension("ron.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            if let Err(e) = std::fs::rename(&tmp, path) {
                warn!("profile not saved: {e}");
            }
        }
    }

    /// Hero level 1..=10 from experience; each level takes a little more.
    pub fn hero_level(&self, hero: &str) -> u32 {
        let xp = self.hero_xp.get(hero).copied().unwrap_or(0);
        let mut level = 1;
        let mut need = 40;
        let mut left = xp;
        while level < 10 && left >= need {
            left -= need;
            level += 1;
            need += 20;
        }
        level
    }

    /// Books a finished run. Returns the Circle this win has just opened, if any.
    pub fn record_run(&mut self, hero: &str, oberegi: u32, won: bool, circle: u8) -> Option<u8> {
        self.oberegi += oberegi;
        *self.hero_xp.entry(hero.to_string()).or_insert(0) += oberegi;
        self.runs += 1;
        let mut opened = None;
        if won {
            self.wins += 1;
            if circle >= self.circle_unlocked && self.circle_unlocked < 10 {
                self.circle_unlocked = circle + 1;
                opened = Some(self.circle_unlocked);
            }
        }
        self.save();
        opened
    }
}

impl Profile {
    fn run_path(&self) -> Option<PathBuf> {
        self.path.as_ref().map(|p| p.with_extension("run.ron"))
    }

    /// Keeps the solo run in progress, so closing the game doesn't end it.
    pub fn save_run(&self, run: &nav_core::Run) {
        let Some(path) = self.run_path() else { return };
        let Ok(text) = ron::to_string(run) else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension("ron.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }

    pub fn load_run(&self) -> Option<nav_core::Run> {
        let text = std::fs::read_to_string(self.run_path()?).ok()?;
        let run: nav_core::Run = ron::from_str(&text).ok()?;
        // a finished run is not something to continue
        (!matches!(run.stage, nav_core::Stage::Victory | nav_core::Stage::Defeat)).then_some(run)
    }

    pub fn clear_run(&self) {
        if let Some(path) = self.run_path() {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn profile_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("TropaVNav").join("profiles"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_win_opens_the_next_circle_only_from_the_highest() {
        let mut p = Profile::default();
        assert_eq!(p.record_run("bogatyr", 50, false, 0), None);
        assert_eq!(p.record_run("bogatyr", 150, true, 0), Some(1));
        assert_eq!(p.record_run("bogatyr", 150, true, 0), None, "an old circle opens nothing new");
        assert_eq!(p.record_run("volhv", 150, true, 1), Some(2));
        assert_eq!((p.oberegi, p.runs, p.wins, p.circle_unlocked), (500, 4, 3, 2));
    }

    #[test]
    fn levels_climb_and_stop_at_ten() {
        let mut p = Profile::default();
        assert_eq!(p.hero_level("bogatyr"), 1);
        p.hero_xp.insert("bogatyr".into(), 40);
        assert_eq!(p.hero_level("bogatyr"), 2);
        p.hero_xp.insert("bogatyr".into(), 100);
        assert_eq!(p.hero_level("bogatyr"), 3);
        p.hero_xp.insert("bogatyr".into(), 1_000_000);
        assert_eq!(p.hero_level("bogatyr"), 10);
    }
}
