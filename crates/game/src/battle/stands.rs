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
static HERE: RwLock<Option<Here>> = RwLock::new(None);

struct Here {
    heroes: Vec<Vec2>,
    /// Foes' places in the order they fill: the marked ones, then a second row between them.
    ranks: Vec<Vec2>,
    /// Which place each foe has: it keeps it until it falls, whoever is called in beside it.
    taken: HashMap<u32, usize>,
}

/// Lowest a foot may be on the stage: under it lies the hand.
const LOWEST: f32 = 730.0;
/// How far behind the marked places the second row stands.
const BACK_ROW: f32 = 90.0;

fn place(spots: &[(f32, f32)], lift: f32) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = spots.iter().map(|(x, y)| Vec2::new(*x, (y - lift).min(LOWEST))).collect();
    out.sort_by(|a, b| a.x.total_cmp(&b.x));
    out
}

/// `0..n` from the middle outwards.
fn middle_out(n: usize) -> impl Iterator<Item = usize> {
    (0..n).map(move |k| if k % 2 == 1 { n / 2 - k.div_ceil(2) } else { n / 2 + k / 2 })
}

/// The places in the order foes take them: a lone foe in the middle, those called in to either
/// side of it, and when the marked places run out, behind the gaps between them.
fn ranks_of(spots: &[Vec2]) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = middle_out(spots.len()).map(|i| spots[i]).collect();
    if spots.len() > 1 {
        out.extend(middle_out(spots.len() - 1).map(|i| (spots[i] + spots[i + 1]) / 2.0 - Vec2::new(0.0, BACK_ROW)));
    }
    out
}

/// A fight opens on this backdrop, lifted so.
pub(super) fn enter(art: &Art, backdrop: &str, lift: f32) {
    let table = TABLE.get_or_init(|| {
        let text = art.read_text("data/stands.ron").unwrap_or_default();
        ron::from_str(&text).unwrap_or_default()
    });
    let here = table.get(backdrop).map(|s| Here { heroes: place(&s.heroes, lift), ranks: ranks_of(&place(&s.foes, lift)), taken: HashMap::new() });
    if let Ok(mut h) = HERE.write() {
        *h = here;
    }
}

/// The first hero stands nearest the foes, the others further back.
fn hero_of(spots: &[Vec2], slot: u8) -> Option<Vec2> {
    spots.iter().rev().nth(slot as usize).copied()
}

/// Gives each living foe with no place yet the first one no living foe holds, in the fight's order.
fn seat(taken: &mut HashMap<u32, usize>, places: usize, foes: &[(u32, bool)]) {
    for (uid, _) in foes.iter().filter(|(_, alive)| *alive) {
        if taken.contains_key(uid) {
            continue;
        }
        let held = |r: usize| foes.iter().any(|(other, alive)| *alive && taken.get(other) == Some(&r));
        if let Some(free) = (0..places).find(|r| !held(*r)) {
            taken.insert(*uid, free);
        }
    }
}

pub(super) fn hero(slot: u8) -> Option<Vec2> {
    HERE.read().ok()?.as_ref().and_then(|h| hero_of(&h.heroes, slot))
}

/// Where this foe stands. `foes` lists the fight's foes and whether they live, asked for only when
/// someone new has come. None off the table, or with more foes than there are places.
pub(super) fn foe(uid: u32, foes: impl FnOnce() -> Vec<(u32, bool)>) -> Option<Vec2> {
    {
        let here = HERE.read().ok()?;
        let h = here.as_ref()?;
        if let Some(r) = h.taken.get(&uid) {
            return h.ranks.get(*r).copied();
        }
    }
    let mut here = HERE.write().ok()?;
    let h = here.as_mut()?;
    seat(&mut h.taken, h.ranks.len(), &foes());
    h.taken.get(&uid).and_then(|r| h.ranks.get(*r).copied())
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
    fn a_lone_foe_takes_the_middle_and_the_others_its_sides() {
        let r = row();
        let xs: Vec<f32> = ranks_of(&r).iter().take(4).map(|p| p.x).collect();
        assert_eq!(xs, vec![r[2].x, r[1].x, r[3].x, r[0].x]);
        let three = place(&[(100.0, 800.0), (200.0, 800.0), (300.0, 800.0)], 0.0);
        assert_eq!(ranks_of(&three)[0].x, 200.0);
    }

    #[test]
    fn a_crowd_stands_behind_the_gaps_and_no_two_share_a_place() {
        let r = row();
        let ranks = ranks_of(&r);
        assert_eq!(ranks.len(), 7);
        assert!(ranks[4].y < ranks[0].y, "the second row is further back");
        let mut xs: Vec<i32> = ranks.iter().map(|p| p.x as i32).collect();
        xs.sort();
        xs.dedup();
        assert_eq!(xs.len(), 7);
    }

    #[test]
    fn those_called_in_move_nobody() {
        let mut taken = HashMap::new();
        seat(&mut taken, 7, &[(1, true)]);
        assert_eq!(taken[&1], 0);
        seat(&mut taken, 7, &[(1, true), (2, true), (3, true)]);
        assert_eq!(taken[&1], 0, "the summoner stays where it stood");
        assert_eq!((taken[&2], taken[&3]), (1, 2));
    }

    #[test]
    fn the_fallen_leave_their_place_to_those_called_after() {
        let mut taken = HashMap::new();
        seat(&mut taken, 7, &[(1, true), (2, true)]);
        seat(&mut taken, 7, &[(1, true), (2, false), (3, true)]);
        assert_eq!(taken[&3], 1);
        assert_eq!(taken[&2], 1, "the fallen keeps it for its last moments");
    }

    #[test]
    fn more_foes_than_places_leaves_the_last_with_none() {
        let mut taken = HashMap::new();
        seat(&mut taken, 2, &[(1, true), (2, true), (3, true)]);
        assert!(!taken.contains_key(&3));
    }
}
