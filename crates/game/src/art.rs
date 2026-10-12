//! Asset lookup. A generated `name-gemini.png` (or `-flow` for audio) always wins
//! over the procedural `name.png`, so dropping a file in swaps the art with no code change.

use bevy::prelude::*;
use std::path::{Path, PathBuf};

/// The `assets` folder: next to the exe when shipped, at the workspace root in development.
pub fn assets_dir() -> PathBuf {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("assets"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("assets"));
    }
    candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"));
    candidates
        .into_iter()
        .find(|p| p.join("data").is_dir())
        // not `canonicalize`: its verbatim-path prefix confuses asset paths on Windows
        .map(|p| std::path::absolute(&p).unwrap_or(p))
        .unwrap_or_else(|| PathBuf::from("assets"))
}

#[derive(Resource)]
pub struct Art {
    root: PathBuf,
}

impl Art {
    pub fn new(root: PathBuf) -> Self {
        Art { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Asset path for `stem` (no extension), or `None` when neither variant exists.
    pub fn find(&self, stem: &str, ext: &str, generated: &str) -> Option<String> {
        [format!("{stem}-{generated}.{ext}"), format!("{stem}.{ext}")]
            .into_iter()
            .find(|rel| self.root.join(rel).is_file())
    }

    pub fn image(&self, assets: &AssetServer, stem: &str) -> Option<Handle<Image>> {
        self.find(stem, "png", "gemini").map(|p| assets.load(p))
    }

    /// An image node for `stem`, or a flat colour block when the art is missing.
    pub fn node(&self, assets: &AssetServer, stem: &str, fallback: Color) -> (ImageNode, BackgroundColor) {
        match self.image(assets, stem) {
            Some(h) => (ImageNode::new(h), BackgroundColor(Color::NONE)),
            None => (ImageNode::solid_color(fallback), BackgroundColor(Color::NONE)),
        }
    }
}
