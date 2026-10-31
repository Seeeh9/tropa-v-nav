//! Where figures stand on the backdrop of this fight: `assets/data/stands.ron`, a backdrop's own
//! open ground, marked on a copy of the picture. A backdrop not in the table keeps the usual ranks.

use crate::art::Art;
use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

#[derive(Deserialize, Clone, Default)]
struct Spots {
    heroes: Vec<(f32, f32)>,
    foes: Vec<(f32, f32)>,
}

static TABLE: OnceLock<HashMap<String, Spots>> = OnceLock::new();
/// This fight's places on the stage. Everything asking where a unit stands asks many times a
/// frame and from all over, so this is kept to hand rather than passed along.
static HERE: RwLock<Option<(Vec<Vec2>, Vec<Vec2>)>> = RwLock::new(None);

/// Lowest a foot may be on the stage: under it lies the hand.
const LOWEST: f32 = 730.0;

fn place(spots: &[(f32, f32)], lift: f32) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = spots.iter().map(|(x, y)| Vec2::new(*x, (y - lift).min(LOWEST))).collect();
    out.sort_by(|a, b| a.x.total_cmp(&b.x));
    out
}

/// A fight opens on this backdrop, lifted so.
pub(super) fn enter(art: &Art, backdrop: &str, lift: f32) {
    let table = TABLE.get_or_init(|| {
        let text = std::fs::read_to_string(art.root().join("data/stands.ron")).unwrap_or_default();
        ron::from_str(&text).unwrap_or_default()
    });
    let here = table.get(backdrop).map(|s| (place(&s.heroes, lift), place(&s.foes, lift)));
    if let Ok(mut h) = HERE.write() {
        *h = here;
    }
}

/// The first hero stands nearest the foes, the others further back.
fn hero_of(spots: &[Vec2], slot: u8) -> Option<Vec2> {
    spots.iter().rev().nth(slot as usize).copied()
}

/// `n` foes share the places evenly, left to right; more than there are places, the usual ranks.
fn foe_of(spots: &[Vec2], i: usize, n: usize) -> Option<Vec2> {
    if n > spots.len() || n == 0 {
        return None;
    }
    let at = ((i as f32 + 0.5) * spots.len() as f32 / n as f32 - 0.5).round() as usize;
    spots.get(at).copied()
}

pub(super) fn hero(slot: u8) -> Option<Vec2> {
    HERE.read().ok()?.as_ref().and_then(|h| hero_of(&h.0, slot))
}

pub(super) fn foe(i: usize, n: usize) -> Option<Vec2> {
    HERE.read().ok()?.as_ref().and_then(|h| foe_of(&h.1, i, n))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> Vec<Vec2> {
        place(&[(1500.0, 900.0), (1100.0, 800.0), (1700.0, 800.0), (1300.0, 800.0)], 150.0)
    }

    #[test]
    fn the_picture_is_lifted_and_the_hand_kept_clear() {
        let r = row();
        assert_eq!(r[0], Vec2::new(1100.0, 650.0));
        assert_eq!(r[2], Vec2::new(1500.0, LOWEST), "never down into the hand");
    }

    #[test]
    fn the_first_hero_stands_nearest_the_foes() {
        let r = row();
        assert_eq!(hero_of(&r, 0).unwrap().x, 1700.0);
        assert_eq!(hero_of(&r, 3).unwrap().x, 1100.0);
        assert!(hero_of(&r, 4).is_none());
    }

    #[test]
    fn foes_share_the_places_and_no_two_share_one() {
        let r = row();
        for n in 1..=4 {
            let mut xs: Vec<i32> = (0..n).map(|i| foe_of(&r, i, n).unwrap().x as i32).collect();
            xs.dedup();
            assert_eq!(xs.len(), n);
        }
        assert!(foe_of(&r, 0, 5).is_none(), "a crowd keeps the usual ranks");
    }
}
