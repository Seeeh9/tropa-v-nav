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

/// The kinds a packed build may have turned a file into: a picture into WebP, a sound into Ogg or
/// FLAC. The packer (`tools/pack_assets.py`) never leaves two kinds of one file side by side.
fn kinds(ext: &str) -> &'static [&'static str] {
    match ext {
        "png" | "webp" => &["png", "webp"],
        "wav" | "ogg" | "flac" => &["wav", "ogg", "flac"],
        "ttf" => &["ttf"],
        "ron" => &["ron"],
        _ => &[],
    }
}

/// The same lookup, for the few places that load a fixed file and hold no `Res<Art>`.
static SHARED: std::sync::OnceLock<Art> = std::sync::OnceLock::new();

/// `Art::path` for a fixed file, as this build holds it.
pub fn path(rel: &str) -> String {
    SHARED.get().map_or_else(|| rel.to_string(), |art| art.path(rel))
}

#[derive(Resource)]
pub struct Art {
    root: PathBuf,
    /// Every file of a packed build, from its `index.txt`; `None` in development, where the folder
    /// itself is looked at.
    index: Option<std::collections::HashSet<String>>,
}

impl Art {
    pub fn new(root: PathBuf) -> Self {
        let index = crate::platform::read_asset(&root, "index.txt")
            .map(|bytes| String::from_utf8_lossy(&bytes).lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect());
        let art = Art { root, index };
        let _ = SHARED.set(Art { root: art.root.clone(), index: art.index.clone() });
        art
    }

    /// Whether the game's file `rel` is there, exactly so named.
    pub fn has(&self, rel: &str) -> bool {
        match &self.index {
            Some(index) => index.contains(rel),
            None => !crate::platform::PACKED_ONLY && self.root.join(rel).is_file(),
        }
    }

    /// The file `rel` as this build holds it: `cards/strike.png` may ship as `cards/strike.webp`.
    pub fn file(&self, rel: &str) -> Option<String> {
        if self.has(rel) {
            return Some(rel.to_string());
        }
        let (stem, ext) = rel.rsplit_once('.')?;
        kinds(ext).iter().filter(|k| **k != ext).map(|k| format!("{stem}.{k}")).find(|alt| self.has(alt))
    }

    /// `file`, or `rel` itself when there is no such file (Bevy then says what is missing).
    pub fn path(&self, rel: &str) -> String {
        self.file(rel).unwrap_or_else(|| rel.to_string())
    }

    /// The bytes of one of the game's files.
    pub fn read(&self, rel: &str) -> Option<Vec<u8>> {
        // a file put there after packing (the installer's `install_lang.txt`) is in no index
        let found = self.file(rel).or_else(|| (!crate::platform::PACKED_ONLY).then(|| rel.to_string()))?;
        crate::platform::read_asset(&self.root, &found)
    }

    /// The text of one of the game's files.
    pub fn read_text(&self, rel: &str) -> Option<String> {
        self.read(rel).and_then(|b| String::from_utf8(b).ok())
    }

    /// The names of the files directly in `dir` that end in `.ext`, sorted.
    pub fn list(&self, dir: &str, ext: &str) -> Vec<String> {
        let dir = dir.trim_end_matches('/');
        let suffix = format!(".{ext}");
        let mut out: Vec<String> = match &self.index {
            Some(index) => index
                .iter()
                .filter_map(|f| f.strip_prefix(dir)?.strip_prefix('/'))
                .filter(|name| !name.contains('/') && name.ends_with(&suffix))
                .map(String::from)
                .collect(),
            None => std::fs::read_dir(self.root.join(dir))
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|name| name.ends_with(&suffix))
                .collect(),
        };
        out.sort();
        out
    }

    /// Asset path for `stem` (no extension), or `None` when neither variant exists.
    pub fn find(&self, stem: &str, ext: &str, generated: &str) -> Option<String> {
        [format!("{stem}-{generated}.{ext}"), format!("{stem}.{ext}")].into_iter().find_map(|rel| self.file(&rel))
    }

    /// Width over height of the picture for `stem`, read from the file's header.
    pub fn aspect(&self, stem: &str) -> Option<f32> {
        let rel = self.find(stem, "png", "gemini")?;
        let (w, h) = image_size(&self.head(&rel)?)?;
        (h > 0).then(|| w as f32 / h as f32)
    }

    /// The first bytes of a file, enough for its header.
    fn head(&self, rel: &str) -> Option<Vec<u8>> {
        if self.index.is_none() || !crate::platform::PACKED_ONLY {
            use std::io::Read;
            let mut head = vec![0u8; 32];
            let n = std::fs::File::open(self.root.join(rel)).ok()?.read(&mut head).ok()?;
            head.truncate(n);
            return Some(head);
        }
        self.read(rel)
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

/// Width and height from a PNG or WebP header.
fn image_size(head: &[u8]) -> Option<(u32, u32)> {
    let be = |i: usize| u32::from_be_bytes([head[i], head[i + 1], head[i + 2], head[i + 3]]);
    let le24 = |i: usize| u32::from(head[i]) | u32::from(head[i + 1]) << 8 | u32::from(head[i + 2]) << 16;
    if head.len() >= 24 && head.starts_with(b"\x89PNG") {
        return Some((be(16), be(20)));
    }
    if head.len() < 30 || &head[0..4] != b"RIFF" || &head[8..12] != b"WEBP" {
        return None;
    }
    match &head[12..16] {
        // lossy: the frame's 14-bit sizes
        b"VP8 " => Some((u32::from(u16::from_le_bytes([head[26], head[27]]) & 0x3fff), u32::from(u16::from_le_bytes([head[28], head[29]]) & 0x3fff))),
        // lossless: 14 bits each, less one, after the signature byte
        b"VP8L" => {
            let b = u32::from_le_bytes([head[21], head[22], head[23], head[24]]);
            Some(((b & 0x3fff) + 1, ((b >> 14) & 0x3fff) + 1))
        }
        // extended (alpha and the rest): the canvas, 24 bits each, less one
        b"VP8X" => Some((le24(24) + 1, le24(27) + 1)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn png_and_webp_headers_give_the_size() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend(37u32.to_be_bytes());
        png.extend(21u32.to_be_bytes());
        assert_eq!(super::image_size(&png), Some((37, 21)));
        let riff = |chunk: &[u8], body: &[u8]| [b"RIFF\0\0\0\0WEBP".as_slice(), chunk, b"\0\0\0\0", body, &[0u8; 8]].concat();
        let bits: u32 = 36 | 20 << 14;
        let lossless = riff(b"VP8L", &[[0x2f].as_slice(), &bits.to_le_bytes()].concat());
        assert_eq!(super::image_size(&lossless), Some((37, 21)));
        let extended = riff(b"VP8X", &[0x10, 0, 0, 0, 36, 0, 0, 20, 0, 0]);
        assert_eq!(super::image_size(&extended), Some((37, 21)));
        let lossy = riff(b"VP8 ", &[0, 0, 0, 0x9d, 0x01, 0x2a, 37, 0, 21, 0]);
        assert_eq!(super::image_size(&lossy), Some((37, 21)));
    }
}
