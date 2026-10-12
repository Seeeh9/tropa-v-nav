//! Mutable combat state. Everything here is hashed for desync checks,
//! so no floats and no unordered maps.

use crate::data::*;
use crate::rng::Rng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const HAND_LIMIT: usize = 10;
pub const BASE_ENERGY: i32 = 3;
pub const BASE_DRAW: i32 = 5;
pub const NAV_MARK: &str = "nav_mark";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum UnitRef {
    Hero(u8),
    Enemy(u32),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    pub uid: u32,
    pub def: Id,
    pub upgraded: bool,
    /// An Echo copy: costs nothing and is gone once played.
    #[serde(default)]
    pub free: bool,
}

pub type Statuses = BTreeMap<StatusKind, i32>;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hero {
    pub slot: u8,
    pub def: Id,
    pub mechanic: Mechanic,
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub energy: i32,
    pub statuses: Statuses,
    pub draw: Vec<Card>,
    pub hand: Vec<Card>,
    pub discard: Vec<Card>,
    pub exhaust: Vec<Card>,
    pub relics: Vec<Id>,
    pub powers: Vec<Passive>,
    pub ready: bool,
    pub downed: bool,
    /// Falls over the whole run, carried in and out of combat.
    pub falls: u32,
    /// Curses earned this combat, to be added to the run deck afterwards.
    pub new_curses: u32,
    /// Bogatyr slot soaking this hero's overflow damage until the round closes.
    pub guard_by: Option<u8>,
    /// Absorbed damage not yet converted into Valor.
    pub valor_rem: i32,
    pub runes: Vec<RuneKind>,
    pub form: Form,
    pub moon: i32,
    pub mask: Option<MaskKind>,
    /// Herbs waiting to be brewed.
    pub cauldron: Vec<Id>,
    pub gold_lost: i32,
    pub flags: BTreeMap<String, i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Intent {
    pub mv: usize,
    pub target: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Enemy {
    pub uid: u32,
    pub def: Id,
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub statuses: Statuses,
    pub alive: bool,
    pub fled: bool,
    /// Moves taken so far; indexes `opening` then cycles `pattern`.
    pub step: usize,
    pub intent: Option<Intent>,
    /// Gold taken per hero slot, handed back if this enemy dies.
    pub stolen: BTreeMap<u8, i32>,
    pub flags: BTreeMap<String, i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Phase {
    Heroes,
    Won,
    Lost,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Played {
    pub player: u8,
    pub card: Id,
    pub upgraded: bool,
    pub kind: CardType,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Combat {
    pub round: u32,
    pub phase: Phase,
    pub heroes: Vec<Hero>,
    pub enemies: Vec<Enemy>,
    pub rng: Rng,
    pub next_uid: u32,
    /// Cards played this round by anyone, oldest first.
    pub log: Vec<Played>,
    /// Energy for the "Raise" action; Circle 9 bumps it to 3.
    pub raise_cost: i32,
    pub hp_scale: i32,
}

/// What a hero brings into a fight from the run.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HeroSetup {
    pub def: Id,
    pub hp: i32,
    pub max_hp: i32,
    /// `(card id, upgraded)`
    pub deck: Vec<(Id, bool)>,
    pub relics: Vec<Id>,
    pub falls: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CombatAction {
    Play { card: u32, target: Option<UnitRef> },
    Ready(bool),
    /// Spend energy and discard a card to raise a downed ally.
    Raise { ally: u8, discard: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionError {
    NotHeroPhase,
    NoSuchPlayer,
    Downed,
    NoSuchCard,
    Unplayable,
    NotEnoughEnergy,
    BadTarget,
    NotDowned,
}

/// Things that happened, in order, for the renderer and the log. Never read back by rules.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    RoundStart(u32),
    Intent { enemy: u32, mv: usize, target: Option<u8> },
    Drawn { player: u8, count: u32 },
    Shuffled { player: u8 },
    CardPlayed { player: u8, card: Id, upgraded: bool, target: Option<UnitRef> },
    /// `hp` and `block` are the target's values after the hit.
    Damage { src: Option<UnitRef>, dst: UnitRef, blocked: i32, lost: i32, attack: bool, hp: i32, block: i32 },
    Redirected { from: u8, to: u8, amount: i32 },
    Block { unit: UnitRef, amount: i32, total: i32 },
    Status { unit: UnitRef, status: StatusKind, delta: i32, total: i32 },
    Heal { unit: UnitRef, amount: i32, hp: i32 },
    Energy { player: u8, delta: i32 },
    Downed { player: u8 },
    Raised { player: u8, by: Option<u8>, hp: i32 },
    CardAdded { player: u8, card: Id, pile: Pile },
    Exhausted { player: u8, card: Id },
    EnemyMove { enemy: u32, mv: usize },
    EnemySkipped { enemy: u32, fear: bool },
    EnemyDied { enemy: u32 },
    EnemyFled { enemy: u32 },
    Summoned { enemy: u32, def: Id },
    Ready { player: u8, ready: bool },
    EnemyPhase,
    RuneCarved { player: u8, rune: RuneKind },
    RuneFired { player: u8, rune: RuneKind },
    HerbAdded { player: u8, herb: Id },
    Brewed { player: u8, recipe: Id },
    Shifted { player: u8, form: Form },
    MaskChanged { player: u8, mask: MaskKind },
    Taunt { player: u8 },
    GoldStolen { player: u8, amount: i32 },
    Ended { won: bool },
}

impl Combat {
    pub fn hero(&self, slot: u8) -> Option<&Hero> {
        self.heroes.get(slot as usize)
    }
    pub fn enemy(&self, uid: u32) -> Option<&Enemy> {
        self.enemies.iter().find(|e| e.uid == uid)
    }
    pub fn living_enemies(&self) -> impl Iterator<Item = &Enemy> {
        self.enemies.iter().filter(|e| e.alive)
    }
    pub fn standing_heroes(&self) -> impl Iterator<Item = &Hero> {
        self.heroes.iter().filter(|h| !h.downed)
    }
    pub fn status(&self, u: UnitRef, s: StatusKind) -> i32 {
        let map = match u {
            UnitRef::Hero(i) => self.hero(i).map(|h| &h.statuses),
            UnitRef::Enemy(i) => self.enemy(i).map(|e| &e.statuses),
        };
        map.and_then(|m| m.get(&s).copied()).unwrap_or(0)
    }
    pub fn is_coop(&self) -> bool {
        self.heroes.len() > 1
    }

    /// Stable 64-bit FNV-1a digest of the whole state, compared across clients.
    pub fn hash64(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        struct Fnv(u64);
        impl Hasher for Fnv {
            fn finish(&self) -> u64 {
                self.0
            }
            fn write(&mut self, bytes: &[u8]) {
                for b in bytes {
                    self.0 ^= *b as u64;
                    self.0 = self.0.wrapping_mul(0x0000_0100_0000_01B3);
                }
            }
        }
        let mut h = Fnv(0xCBF2_9CE4_8422_2325);
        self.hash(&mut h);
        h.finish()
    }
}
