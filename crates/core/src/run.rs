//! A whole run: the path map, votes, fights, rewards and rest. Like combat,
//! it is a deterministic reducer, so every machine in a party holds the same run.

use crate::combat;
use crate::data::*;
use crate::rng::Rng;
use crate::state::*;
use serde::{Deserialize, Serialize};

pub const FLOORS: usize = 15;
pub const MAP_WIDTH: u8 = 7;
pub const POTION_SLOTS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum NodeKind {
    Fight,
    Elite,
    Event,
    Rest,
    Shop,
    Chest,
    Boss,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MapNode {
    pub kind: NodeKind,
    /// Column 0..MAP_WIDTH, for drawing.
    pub col: u8,
    /// Indices into the next floor this node leads to.
    pub next: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Map {
    pub floors: Vec<Vec<MapNode>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Player {
    pub hero: Id,
    pub hp: i32,
    pub max_hp: i32,
    /// Gold is personal: every player has their own purse.
    pub gold: i32,
    /// `(card id, upgraded)`
    pub deck: Vec<(Id, bool)>,
    pub relics: Vec<Id>,
    pub potions: Vec<Id>,
    pub falls: u32,
    /// Cards removed at the shop so far; each next removal costs more.
    pub removals: u32,
}

/// What one player is offered after a fight or at a chest.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Reward {
    pub gold: i32,
    pub cards: Vec<Id>,
    pub relic: Option<Id>,
    pub done: bool,
}

/// One player's shelves at the pedlar: `(id, price)`; a sold item becomes `None`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Stock {
    pub cards: Vec<Option<(Id, i32)>>,
    pub relics: Vec<Option<(Id, i32)>>,
    pub potions: Vec<Option<(Id, i32)>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stage {
    /// Choosing the next node; `votes[player]` indexes into [`Run::choices`].
    Map { votes: Vec<Option<u8>> },
    Combat,
    Reward { rewards: Vec<Reward> },
    Rest { done: Vec<bool> },
    /// The pedlar: every player has their own shelves.
    Shop { stock: Vec<Stock>, done: Vec<bool> },
    /// An omen. `picks[player]` is the chosen option (a vote, for vote omens).
    Omen { id: Id, picks: Vec<Option<u8>> },
    /// A boss trophy: the party votes for one of `offers`, and every player gets it.
    Trophy { offers: Vec<Id>, picks: Vec<Option<u8>> },
    Victory,
    Defeat,
}

/// What the party's village and hero levels have opened. It widens variety,
/// never power: nothing here makes a hero stronger at the start of a run.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Unlocks {
    /// Smithy level 0..=2: common relics, then uncommon, then rare.
    pub relic_tier: u8,
    /// Herbalist's hut level 0..=2: ten potions per level.
    pub potion_tier: u8,
    /// Tavern level 0..=2: twenty omens, then fifteen more per level.
    pub omen_tier: u8,
    /// Hero level of each player, in slot order: uncommon cards from 2, rare from 4.
    pub hero_levels: Vec<u32>,
    /// Which starting relic each player took, in slot order: 0 is the hero's own,
    /// 1 and 2 are the shrine's alternatives.
    #[serde(default)]
    pub starters: Vec<u8>,
}

impl Unlocks {
    /// Everything open: tests, the simulator and dev shortcuts.
    pub fn all(players: usize) -> Unlocks {
        Unlocks { relic_tier: 2, potion_tier: 2, omen_tier: 2, hero_levels: vec![10; players], starters: vec![] }
    }

    fn relic_ok(&self, rarity: Rarity) -> bool {
        match rarity {
            Rarity::Rare => self.relic_tier >= 2,
            Rarity::Uncommon => self.relic_tier >= 1,
            _ => true,
        }
    }

    fn card_ok(&self, player: usize, rarity: Rarity) -> bool {
        let level = self.hero_levels.get(player).copied().unwrap_or(1);
        match rarity {
            Rarity::Rare => level >= 4,
            Rarity::Uncommon => level >= 2,
            _ => true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Run {
    pub seed: u64,
    pub rng: Rng,
    /// Circle of Nav, 0..=10.
    pub circle: u8,
    pub unlocks: Unlocks,
    pub act: u8,
    pub map: Map,
    /// `(floor, node)` the party stands on; `None` before the first step of an act.
    pub at: Option<(u8, u8)>,
    pub players: Vec<Player>,
    pub stage: Stage,
    pub combat: Option<Combat>,
    /// What is being fought, for rewards.
    pub fighting: Option<NodeKind>,
    /// Tallies for the village currency at the end.
    pub floors_cleared: u32,
    pub elites_slain: u32,
    pub bosses_slain: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RunAction {
    /// Put your token on a next node (index into [`Run::choices`]).
    Vote(u8),
    Combat(CombatAction),
    /// Take one of the offered cards, or `None` to skip.
    TakeCard(Option<u8>),
    /// At the fire: sleep.
    Sleep,
    /// At the fire: upgrade the card at this deck index.
    Upgrade(u16),
    /// Hand some of your gold to an ally (shop and fire only).
    GiveGold { to: u8, amount: i32 },
    BuyCard(u8),
    BuyRelic(u8),
    BuyPotion(u8),
    /// Pay the pedlar to take a card out of your deck for good.
    RemoveCard(u16),
    LeaveShop,
    /// Choose (or vote for) an option of the omen.
    Choose(u8),
    /// Throw away a potion to free the slot.
    DropPotion(u8),
}

/// At a schism the party must part: when every other player has gone the same way,
/// that way is closed to the last one. `None` alone, or while the others differ or still think.
pub fn schism_closed(picks: &[Option<u8>], p: usize) -> Option<u8> {
    let mut others = picks.iter().enumerate().filter(|(i, _)| *i != p).map(|(_, v)| *v);
    let first = others.next()??;
    others.all(|v| v == Some(first)).then_some(first)
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RunError {
    Combat(ActionError),
    WrongStage,
    NoSuchPlayer,
    BadChoice,
    AlreadyDone,
    NotEnoughGold,
    /// A schism: the others all went that way, so this player must take another.
    MustSplit,
    SoldOut,
    NoRoom,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunEvent {
    Combat(Event),
    Voted { player: u8, choice: u8 },
    /// A tied vote was settled by lot among `tied`.
    Lot { tied: Vec<u8>, chosen: u8 },
    Moved { floor: u8, node: u8, kind: NodeKind },
    CombatStarted,
    Rewards,
    CardTaken { player: u8, card: Option<Id> },
    Rested { player: u8, healed: i32 },
    Upgraded { player: u8, card: Id },
    Gold { player: u8, delta: i32 },
    RelicGained { player: u8, relic: Id },
    PotionGained { player: u8, potion: Id },
    CardGained { player: u8, card: Id },
    CardRemoved { player: u8, card: Id },
    Hp { player: u8, delta: i32 },
    OmenResolved { option: u8 },
    ActCleared { act: u8 },
    Ended { won: bool },
}

/// Rules that change with the Circle of Nav; effects stack as the circle rises.
#[derive(Clone, Copy, Debug)]
pub struct Circle(pub u8);

impl Circle {
    pub fn elite_weight(self) -> u32 {
        if self.0 >= 1 { 12 } else { 8 }
    }
    pub fn enemy_dmg(self, kind: NodeKind) -> i32 {
        match kind {
            NodeKind::Boss if self.0 >= 4 => 115,
            NodeKind::Fight if self.0 >= 2 => 110,
            _ => 100,
        }
    }
    pub fn rest_percent(self) -> i32 {
        if self.0 >= 5 { 20 } else { 30 }
    }
    pub fn starts_with_toska(self) -> bool {
        self.0 >= 6
    }
    pub fn gold_percent(self) -> i32 {
        if self.0 >= 7 { 75 } else { 100 }
    }
    pub fn enemy_hp(self) -> i32 {
        if self.0 >= 8 { 110 } else { 100 }
    }
    pub fn koschei_shadow(self) -> bool {
        self.0 >= 10
    }
    pub fn raise_cost(self) -> i32 {
        if self.0 >= 9 { 3 } else { 2 }
    }
}

pub fn generate_map(rng: &mut Rng, circle: Circle) -> Map {
    // six walkers leave the bottom and drift up; where they tread there are nodes
    let mut cols: Vec<Vec<u8>> = vec![Vec::new(); FLOORS];
    let mut edges: Vec<Vec<(u8, u8)>> = vec![Vec::new(); FLOORS];
    for _walker in 0..6 {
        let mut col = rng.below(MAP_WIDTH as u32) as u8;
        for floor in 0..FLOORS {
            if !cols[floor].contains(&col) {
                cols[floor].push(col);
            }
            if floor + 1 < FLOORS {
                let step = rng.range(-1, 1);
                let mut to = (col as i32 + step).clamp(0, MAP_WIDTH as i32 - 1) as u8;
                // paths may merge and split but never cross each other
                let crosses = edges[floor].iter().any(|(a, b)| (*a < col && *b > to) || (*a > col && *b < to));
                if crosses {
                    to = col;
                }
                if !edges[floor].contains(&(col, to)) {
                    edges[floor].push((col, to));
                }
                col = to;
            }
        }
    }
    for c in &mut cols {
        c.sort();
    }
    let mut floors: Vec<Vec<MapNode>> = Vec::new();
    for floor in 0..FLOORS {
        let nodes = cols[floor]
            .iter()
            .map(|col| {
                let mut next: Vec<u8> = edges[floor]
                    .iter()
                    .filter(|(a, _)| a == col)
                    .filter_map(|(_, b)| cols.get(floor + 1).and_then(|n| n.iter().position(|x| x == b)).map(|i| i as u8))
                    .collect();
                next.sort();
                next.dedup();
                MapNode { kind: NodeKind::Fight, col: *col, next }
            })
            .collect();
        floors.push(nodes);
    }
    // kinds: fixed floors first, the rest by share (design doc, section 5)
    for floor in 0..FLOORS {
        for i in 0..floors[floor].len() {
            let kind = match floor {
                0 => NodeKind::Fight,
                8 => NodeKind::Chest,
                14 => NodeKind::Rest,
                _ => {
                    let parents: Vec<NodeKind> =
                        if floor == 0 { vec![] } else { floors[floor - 1].iter().filter(|p| p.next.contains(&(i as u8))).map(|p| p.kind).collect() };
                    let mut pick = NodeKind::Fight;
                    for _ in 0..8 {
                        let mut bag: Vec<(NodeKind, u32)> = vec![(NodeKind::Fight, 45), (NodeKind::Event, 22)];
                        // no fire right before the guaranteed one
                        if floor != 13 {
                            bag.push((NodeKind::Rest, 12));
                        }
                        if floor >= 5 {
                            bag.push((NodeKind::Elite, circle.elite_weight()));
                        }
                        if floor >= 2 {
                            bag.push((NodeKind::Shop, 6));
                        }
                        let total: u32 = bag.iter().map(|b| b.1).sum();
                        let mut roll = rng.below(total);
                        for (k, w) in bag {
                            if roll < w {
                                pick = k;
                                break;
                            }
                            roll -= w;
                        }
                        // two fires, shops or elites in a row make a dull path
                        let special = matches!(pick, NodeKind::Rest | NodeKind::Shop | NodeKind::Elite);
                        if !(special && parents.contains(&pick)) {
                            break;
                        }
                        pick = NodeKind::Fight;
                    }
                    pick
                }
            };
            floors[floor][i].kind = kind;
        }
    }
    Map { floors }
}

impl Run {
    /// A run with everything unlocked.
    pub fn new(c: &Content, heroes: &[Id], seed: u64, circle: u8) -> Option<Run> {
        Run::new_with(c, heroes, seed, circle, Unlocks::all(heroes.len()))
    }

    pub fn new_with(c: &Content, heroes: &[Id], seed: u64, circle: u8, unlocks: Unlocks) -> Option<Run> {
        let mut rng = Rng::new(seed);
        let circle_rules = Circle(circle);
        let mut players = Vec::new();
        for (slot, h) in heroes.iter().enumerate() {
            let d = c.heroes.get(h)?;
            let relic = match unlocks.starters.get(slot).copied().unwrap_or(0) {
                0 => &d.relic,
                k => d.alt_relics.get(k as usize - 1).filter(|id| c.relics.contains_key(*id)).unwrap_or(&d.relic),
            };
            let mut deck: Vec<(Id, bool)> = d.deck.iter().map(|id| (id.clone(), false)).collect();
            if circle_rules.starts_with_toska() && c.cards.contains_key("toska") {
                deck.push(("toska".into(), false));
            }
            players.push(Player { hero: h.clone(), hp: d.max_hp, max_hp: d.max_hp, gold: 99, deck, relics: vec![relic.clone()], potions: vec![], falls: 0, removals: 0 });
        }
        let map = generate_map(&mut rng, circle_rules);
        let n = players.len();
        Some(Run {
            seed,
            rng,
            circle,
            unlocks,
            act: 1,
            map,
            at: None,
            players,
            stage: Stage::Map { votes: vec![None; n] },
            combat: None,
            fighting: None,
            floors_cleared: 0,
            elites_slain: 0,
            bosses_slain: 0,
        })
    }

    pub fn rules(&self) -> Circle {
        Circle(self.circle)
    }

    /// Nodes the party may step to next: `(node index on that floor, kind)`.
    /// Empty when the only way on is the boss.
    pub fn choices(&self) -> Vec<(u8, NodeKind)> {
        // the last stretch has no map: only Koschei waits
        if self.map.floors.is_empty() {
            return vec![(0, NodeKind::Boss)];
        }
        match self.at {
            None => self.map.floors[0].iter().enumerate().map(|(i, n)| (i as u8, n.kind)).collect(),
            Some((floor, node)) => {
                let next = floor as usize + 1;
                if next >= self.map.floors.len() {
                    return vec![(0, NodeKind::Boss)];
                }
                self.map.floors[floor as usize][node as usize].next.iter().map(|i| (*i, self.map.floors[next][*i as usize].kind)).collect()
            }
        }
    }

    /// Village currency earned so far (design doc, section 8).
    pub fn oberegi(&self) -> u32 {
        let won = if self.stage == Stage::Victory { 100 } else { 0 };
        self.floors_cleared + self.elites_slain * 10 + self.bosses_slain * 30 + won
    }

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

    /// Applies one player's action. On `Err` the run is untouched.
    pub fn apply(&mut self, c: &Content, player: u8, action: &RunAction) -> Result<Vec<RunEvent>, RunError> {
        let p = player as usize;
        if p >= self.players.len() {
            return Err(RunError::NoSuchPlayer);
        }
        let mut ev = Vec::new();
        match (action, &mut self.stage) {
            (RunAction::Vote(choice), Stage::Map { votes }) => {
                let options = {
                    // borrow dance: choices() needs &self
                    let at = self.at;
                    let map = &self.map;
                    match at {
                        _ if map.floors.is_empty() => 1,
                        None => map.floors[0].len(),
                        Some((f, n)) if (f as usize + 1) < map.floors.len() => map.floors[f as usize][n as usize].next.len(),
                        Some(_) => 1,
                    }
                };
                if *choice as usize >= options {
                    return Err(RunError::BadChoice);
                }
                votes[p] = Some(*choice);
                ev.push(RunEvent::Voted { player, choice: *choice });
                if votes.iter().all(|v| v.is_some()) {
                    let cast: Vec<u8> = votes.iter().flatten().copied().collect();
                    let mut counts = vec![0u32; options];
                    for v in &cast {
                        counts[*v as usize] += 1;
                    }
                    let best = counts.iter().copied().max().unwrap_or(0);
                    let tied: Vec<u8> = (0..options as u8).filter(|i| counts[*i as usize] == best).collect();
                    // a tie goes to the lot, never to the host
                    let winner = if tied.len() == 1 {
                        tied[0]
                    } else {
                        let chosen = *self.rng.pick(&tied).unwrap_or(&tied[0]);
                        ev.push(RunEvent::Lot { tied: tied.clone(), chosen });
                        chosen
                    };
                    self.travel(c, winner, &mut ev);
                }
            }
            (RunAction::Combat(a), Stage::Combat) => {
                let combat = self.combat.as_mut().ok_or(RunError::WrongStage)?;
                let events = combat::apply(c, combat, player, a).map_err(RunError::Combat)?;
                ev.extend(events.into_iter().map(RunEvent::Combat));
                let phase = combat.phase;
                match phase {
                    Phase::Won => self.after_fight(c, &mut ev),
                    Phase::Lost => {
                        self.stage = Stage::Defeat;
                        ev.push(RunEvent::Ended { won: false });
                    }
                    Phase::Heroes => {}
                }
            }
            (RunAction::TakeCard(pick), Stage::Reward { rewards }) => {
                let r = &mut rewards[p];
                if r.done {
                    return Err(RunError::AlreadyDone);
                }
                let card = match pick {
                    Some(i) => Some(r.cards.get(*i as usize).ok_or(RunError::BadChoice)?.clone()),
                    None => None,
                };
                r.done = true;
                if let Some(card) = &card {
                    self.players[p].deck.push((card.clone(), false));
                }
                ev.push(RunEvent::CardTaken { player, card });
                if rewards.iter().all(|r| r.done) {
                    self.to_map(c, &mut ev);
                }
            }
            (RunAction::Sleep, Stage::Rest { done }) | (RunAction::Upgrade(_), Stage::Rest { done }) => {
                if done[p] {
                    return Err(RunError::AlreadyDone);
                }
                let pl = &mut self.players[p];
                match action {
                    RunAction::Sleep => {
                        let pct = Circle(self.circle).rest_percent();
                        let healed = (pl.max_hp * pct / 100).min(pl.max_hp - pl.hp);
                        pl.hp += healed;
                        ev.push(RunEvent::Rested { player, healed });
                    }
                    RunAction::Upgrade(i) => {
                        let slot = pl.deck.get_mut(*i as usize).ok_or(RunError::BadChoice)?;
                        let upgradable = c.cards.get(&slot.0).is_some_and(|d| d.up.is_some());
                        if slot.1 || !upgradable {
                            return Err(RunError::BadChoice);
                        }
                        slot.1 = true;
                        ev.push(RunEvent::Upgraded { player, card: slot.0.clone() });
                    }
                    _ => unreachable!(),
                }
                done[p] = true;
                if done.iter().all(|d| *d) {
                    self.to_map(c, &mut ev);
                }
            }
            (RunAction::GiveGold { to, amount }, Stage::Rest { .. } | Stage::Shop { .. }) => {
                let to = *to as usize;
                if to >= self.players.len() || to == p || *amount <= 0 {
                    return Err(RunError::BadChoice);
                }
                if self.players[p].gold < *amount {
                    return Err(RunError::NotEnoughGold);
                }
                self.players[p].gold -= amount;
                self.players[to].gold += amount;
                ev.push(RunEvent::Gold { player, delta: -amount });
                ev.push(RunEvent::Gold { player: to as u8, delta: *amount });
            }
            (RunAction::BuyCard(i) | RunAction::BuyRelic(i) | RunAction::BuyPotion(i), Stage::Shop { stock, done }) => {
                if done[p] {
                    return Err(RunError::AlreadyDone);
                }
                let shelf = match action {
                    RunAction::BuyCard(_) => &mut stock[p].cards,
                    RunAction::BuyRelic(_) => &mut stock[p].relics,
                    _ => &mut stock[p].potions,
                };
                let slot = shelf.get_mut(*i as usize).ok_or(RunError::BadChoice)?;
                let (id, price) = slot.clone().ok_or(RunError::SoldOut)?;
                let pl = &mut self.players[p];
                if pl.gold < price {
                    return Err(RunError::NotEnoughGold);
                }
                if matches!(action, RunAction::BuyPotion(_)) && pl.potions.len() >= POTION_SLOTS {
                    return Err(RunError::NoRoom);
                }
                pl.gold -= price;
                *slot = None;
                ev.push(RunEvent::Gold { player, delta: -price });
                match action {
                    RunAction::BuyCard(_) => {
                        pl.deck.push((id.clone(), false));
                        ev.push(RunEvent::CardGained { player, card: id });
                    }
                    RunAction::BuyRelic(_) => {
                        pl.relics.push(id.clone());
                        ev.push(RunEvent::RelicGained { player, relic: id });
                    }
                    _ => {
                        pl.potions.push(id.clone());
                        ev.push(RunEvent::PotionGained { player, potion: id });
                    }
                }
            }
            (RunAction::RemoveCard(i), Stage::Shop { done, .. }) => {
                if done[p] {
                    return Err(RunError::AlreadyDone);
                }
                let pl = &mut self.players[p];
                // 75 gold, then 25 more each time
                let price = 75 + 25 * pl.removals as i32;
                if *i as usize >= pl.deck.len() {
                    return Err(RunError::BadChoice);
                }
                if pl.gold < price {
                    return Err(RunError::NotEnoughGold);
                }
                pl.gold -= price;
                pl.removals += 1;
                let (card, _) = pl.deck.remove(*i as usize);
                ev.push(RunEvent::Gold { player, delta: -price });
                ev.push(RunEvent::CardRemoved { player, card });
            }
            (RunAction::LeaveShop, Stage::Shop { done, .. }) => {
                done[p] = true;
                if done.iter().all(|d| *d) {
                    self.to_map(c, &mut ev);
                }
            }
            (RunAction::Choose(choice), Stage::Trophy { offers, picks }) => {
                if *choice as usize >= offers.len() {
                    return Err(RunError::BadChoice);
                }
                picks[p] = Some(*choice);
                ev.push(RunEvent::Voted { player, choice: *choice });
                if picks.iter().all(|v| v.is_some()) {
                    let cast: Vec<u8> = picks.iter().flatten().copied().collect();
                    let offers = offers.clone();
                    let mut counts = vec![0u32; offers.len()];
                    for v in &cast {
                        counts[*v as usize] += 1;
                    }
                    let best = counts.iter().copied().max().unwrap_or(0);
                    let tied: Vec<u8> = (0..offers.len() as u8).filter(|i| counts[*i as usize] == best).collect();
                    let winner = if tied.len() == 1 {
                        tied[0]
                    } else {
                        let chosen = *self.rng.pick(&tied).unwrap_or(&tied[0]);
                        ev.push(RunEvent::Lot { tied: tied.clone(), chosen });
                        chosen
                    };
                    ev.push(RunEvent::OmenResolved { option: winner });
                    let relic = offers[winner as usize].clone();
                    for q in 0..self.players.len() {
                        self.players[q].relics.push(relic.clone());
                        ev.push(RunEvent::RelicGained { player: q as u8, relic: relic.clone() });
                    }
                    self.stage = Stage::Map { votes: vec![None; self.players.len()] };
                }
            }
            (RunAction::Choose(choice), Stage::Omen { id, picks }) => {
                let def = c.events.get(id.as_str()).ok_or(RunError::BadChoice)?.clone();
                let option = def.options.get(*choice as usize).ok_or(RunError::BadChoice)?;
                if picks[p].is_some() && def.kind != EventKind::Vote {
                    return Err(RunError::AlreadyDone);
                }
                if def.kind == EventKind::Schism && schism_closed(picks, p) == Some(*choice) {
                    return Err(RunError::MustSplit);
                }
                if self.players[p].gold < option.price {
                    return Err(RunError::NotEnoughGold);
                }
                picks[p] = Some(*choice);
                let all = picks.iter().all(|v| v.is_some());
                let cast: Vec<u8> = picks.iter().flatten().copied().collect();
                match def.kind {
                    // one pays, all gain; those who refuse just wait to see if anyone will
                    EventKind::Sacrifice => {
                        self.apply_option(c, p, option, &mut ev);
                        if !option.all.is_empty() {
                            ev.push(RunEvent::OmenResolved { option: *choice });
                            let gift = EventOption { id: option.id.clone(), price: 0, fx: option.all.clone(), all: vec![] };
                            for q in 0..self.players.len() {
                                self.apply_option(c, q, &gift, &mut ev);
                            }
                            self.to_map(c, &mut ev);
                        } else if all {
                            self.to_map(c, &mut ev);
                        }
                    }
                    // everyone decides for themselves, at once
                    EventKind::Personal | EventKind::Schism => {
                        self.apply_option(c, p, option, &mut ev);
                        if all {
                            self.to_map(c, &mut ev);
                        }
                    }
                    // one choice for the party; a tie goes to the lot
                    EventKind::Vote => {
                        ev.push(RunEvent::Voted { player, choice: *choice });
                        if all {
                            let mut counts = vec![0u32; def.options.len()];
                            for v in &cast {
                                counts[*v as usize] += 1;
                            }
                            let best = counts.iter().copied().max().unwrap_or(0);
                            let tied: Vec<u8> = (0..def.options.len() as u8).filter(|i| counts[*i as usize] == best).collect();
                            let winner = if tied.len() == 1 {
                                tied[0]
                            } else {
                                let chosen = *self.rng.pick(&tied).unwrap_or(&tied[0]);
                                ev.push(RunEvent::Lot { tied: tied.clone(), chosen });
                                chosen
                            };
                            ev.push(RunEvent::OmenResolved { option: winner });
                            let option = &def.options[winner as usize];
                            for q in 0..self.players.len() {
                                self.apply_option(c, q, option, &mut ev);
                            }
                            self.to_map(c, &mut ev);
                        }
                    }
                }
            }
            (RunAction::DropPotion(i), _) => {
                // not mid-fight: there the potions live in the combat state
                if self.stage == Stage::Combat {
                    return Err(RunError::WrongStage);
                }
                let pl = &mut self.players[p];
                if *i as usize >= pl.potions.len() {
                    return Err(RunError::BadChoice);
                }
                pl.potions.remove(*i as usize);
            }
            _ => return Err(RunError::WrongStage),
        }
        Ok(ev)
    }

    /// Puts the party straight into a shop or an omen, wherever it stands.
    /// For tests and the dev build's screen shortcuts; never reachable by a player action.
    pub fn force_stage(&mut self, c: &Content, kind: NodeKind) {
        match kind {
            NodeKind::Shop => {
                let stock = (0..self.players.len()).map(|p| self.roll_stock(c, p)).collect();
                self.stage = Stage::Shop { stock, done: vec![false; self.players.len()] };
            }
            NodeKind::Event => {
                let pool: Vec<&EventDef> = c.events.values().collect();
                if let Some(e) = self.rng.pick(&pool) {
                    self.stage = Stage::Omen { id: e.id.clone(), picks: vec![None; self.players.len()] };
                }
            }
            NodeKind::Rest => self.stage = Stage::Rest { done: vec![false; self.players.len()] },
            _ => {}
        }
    }

    fn to_map(&mut self, c: &Content, ev: &mut Vec<RunEvent>) {
        // past the boss there is no map left in this act
        let boss_done = self.fighting == Some(NodeKind::Boss);
        self.fighting = None;
        self.combat = None;
        if boss_done {
            ev.push(RunEvent::ActCleared { act: self.act });
            // act 4 is the island of Buyan: Koschei himself
            if self.act >= 4 {
                self.stage = Stage::Victory;
                ev.push(RunEvent::Ended { won: true });
                return;
            }
            self.act += 1;
            self.at = None;
            self.map = if self.act >= 4 { Map { floors: vec![] } } else { generate_map(&mut self.rng, Circle(self.circle)) };
            // the boss leaves a trophy: three are offered, the party votes for one, everyone gets it
            let mut offers: Vec<Id> = c.relics.values().filter(|r| r.pool == RelicPool::Boss && !self.players.iter().any(|p| p.relics.contains(&r.id))).map(|r| r.id.clone()).collect();
            self.rng.shuffle(&mut offers);
            offers.truncate(3);
            if !offers.is_empty() {
                self.stage = Stage::Trophy { offers, picks: vec![None; self.players.len()] };
                return;
            }
        }
        self.stage = Stage::Map { votes: vec![None; self.players.len()] };
    }

    fn travel(&mut self, c: &Content, choice: u8, ev: &mut Vec<RunEvent>) {
        let options = self.choices();
        let (node, kind) = options[choice as usize];
        let floor = match self.at {
            None => 0,
            Some((f, _)) => f + 1,
        };
        if kind != NodeKind::Boss && !self.map.floors.is_empty() {
            self.at = Some((floor, node));
        }
        ev.push(RunEvent::Moved { floor, node, kind });
        match kind {
            NodeKind::Fight | NodeKind::Elite | NodeKind::Boss => self.start_fight(c, kind, ev),
            NodeKind::Rest => self.stage = Stage::Rest { done: vec![false; self.players.len()] },
            NodeKind::Chest => {
                self.floors_cleared += 1;
                let rewards = (0..self.players.len()).map(|p| Reward { gold: 0, cards: vec![], relic: self.roll_relic(c, p), done: false }).collect();
                self.grant(c, rewards, ev);
            }
            NodeKind::Shop => {
                self.floors_cleared += 1;
                let stock = (0..self.players.len()).map(|p| self.roll_stock(c, p)).collect();
                self.stage = Stage::Shop { stock, done: vec![false; self.players.len()] };
            }
            NodeKind::Event => {
                self.floors_cleared += 1;
                let act = self.act;
                let open = 20 + 15 * self.unlocks.omen_tier as usize;
                let pool: Vec<&EventDef> = c.events.values().take(open).filter(|e| e.act == 0 || e.act == act).collect();
                match self.rng.pick(&pool) {
                    Some(e) => self.stage = Stage::Omen { id: e.id.clone(), picks: vec![None; self.players.len()] },
                    // no omens written yet: the road is simply quiet
                    None => self.stage = Stage::Map { votes: vec![None; self.players.len()] },
                }
            }
        }
    }

    fn start_fight(&mut self, c: &Content, kind: NodeKind, ev: &mut Vec<RunEvent>) {
        let want = match kind {
            NodeKind::Elite => EncounterKind::Elite,
            NodeKind::Boss => EncounterKind::Boss,
            _ => EncounterKind::Normal,
        };
        let pick = |rng: &mut Rng, act: u8, kind: EncounterKind| -> Option<EncounterDef> {
            let pool: Vec<&EncounterDef> = c.encounters.values().filter(|e| e.act == act && e.kind == kind).collect();
            rng.pick(&pool).map(|e| (*e).clone())
        };
        // fall back so a thin content set never strands the party
        let enc = pick(&mut self.rng, self.act, want)
            .or_else(|| pick(&mut self.rng, self.act, EncounterKind::Normal))
            .or_else(|| pick(&mut self.rng, 1, EncounterKind::Normal));
        let Some(enc) = enc else {
            self.stage = Stage::Map { votes: vec![None; self.players.len()] };
            return;
        };
        let n = self.players.len();
        let mut enemies = if n == 1 && !enc.solo.is_empty() { enc.solo.clone() } else { enc.enemies.clone() };
        if n == 3 && self.rng.chance(50) {
            enemies.extend(enc.extra3.iter().cloned());
        }
        if n >= 4 {
            enemies.extend(enc.extra4.iter().cloned());
        }
        let party: Vec<HeroSetup> = self
            .players
            .iter()
            .map(|p| HeroSetup { def: p.hero.clone(), hp: p.hp, max_hp: p.max_hp, deck: p.deck.clone(), relics: p.relics.clone(), falls: p.falls, potions: p.potions.clone() })
            .collect();
        let rules = Circle(self.circle);
        let opts = CombatOpts { raise_cost: rules.raise_cost(), enemy_hp: rules.enemy_hp(), enemy_dmg: rules.enemy_dmg(kind), shadow: rules.koschei_shadow() };
        let seed = ((self.rng.next_u32() as u64) << 32) | self.rng.next_u32() as u64;
        let (combat, events) = combat::start_with(c, &party, &enemies, seed, opts);
        self.combat = Some(combat);
        self.fighting = Some(kind);
        self.stage = Stage::Combat;
        ev.push(RunEvent::CombatStarted);
        ev.extend(events.into_iter().map(RunEvent::Combat));
        // a fight can be over before anyone acts: a curse or a relic that bites at the start
        // may fell the last hero standing, and then nobody is left to make the move that ends it
        match self.combat.as_ref().map(|k| k.phase) {
            Some(Phase::Lost) => {
                self.stage = Stage::Defeat;
                ev.push(RunEvent::Ended { won: false });
            }
            Some(Phase::Won) => self.after_fight(c, ev),
            _ => {}
        }
    }

    /// Puts the party straight into a fight with exactly these enemies.
    /// For tests and the dev build's screen shortcuts; never reachable by a player action.
    pub fn force_fight(&mut self, c: &Content, enemies: &[Id]) {
        let party: Vec<HeroSetup> = self
            .players
            .iter()
            .map(|p| HeroSetup { def: p.hero.clone(), hp: p.hp, max_hp: p.max_hp, deck: p.deck.clone(), relics: p.relics.clone(), falls: p.falls, potions: p.potions.clone() })
            .collect();
        let seed = ((self.rng.next_u32() as u64) << 32) | self.rng.next_u32() as u64;
        let (combat, _) = combat::start_with(c, &party, enemies, seed, CombatOpts::default());
        self.combat = Some(combat);
        self.fighting = Some(NodeKind::Fight);
        self.stage = Stage::Combat;
    }

    fn after_fight(&mut self, c: &Content, ev: &mut Vec<RunEvent>) {
        // the finished fight stays readable until the party walks on
        let Some(combat) = self.combat.clone() else { return };
        let kind = self.fighting.unwrap_or(NodeKind::Fight);
        for (pl, h) in self.players.iter_mut().zip(&combat.heroes) {
            pl.hp = h.hp.max(1);
            pl.max_hp = h.max_hp;
            pl.falls = h.falls;
            pl.potions = h.potions.clone();
            pl.gold = (pl.gold - h.gold_lost).max(0);
            for _ in 0..h.new_curses {
                pl.deck.push((NAV_MARK.into(), false));
            }
        }
        self.floors_cleared += 1;
        match kind {
            NodeKind::Elite => self.elites_slain += 1,
            NodeKind::Boss => self.bosses_slain += 1,
            _ => {}
        }
        let gold_pct = Circle(self.circle).gold_percent();
        let mut rewards = Vec::new();
        for p in 0..self.players.len() {
            let base = match kind {
                NodeKind::Elite => self.rng.range(25, 35),
                NodeKind::Boss => self.rng.range(95, 105),
                _ => self.rng.range(10, 20),
            };
            let cards = self.roll_cards(c, p, kind);
            let relic = if kind == NodeKind::Elite { self.roll_relic(c, p) } else { None };
            rewards.push(Reward { gold: base * gold_pct / 100, cards, relic, done: false });
        }
        // four fights in ten leave a draught behind
        for p in 0..self.players.len() {
            if self.rng.chance(40) {
                self.give_potion(c, p, ev);
            }
        }
        self.grant(c, rewards, ev);
    }

    /// Hands out gold and relics at once and leaves the card choice to the players.
    fn grant(&mut self, c: &Content, mut rewards: Vec<Reward>, ev: &mut Vec<RunEvent>) {
        for (p, r) in rewards.iter_mut().enumerate() {
            if r.gold != 0 {
                self.players[p].gold += r.gold;
                ev.push(RunEvent::Gold { player: p as u8, delta: r.gold });
            }
            if let Some(relic) = &r.relic {
                self.players[p].relics.push(relic.clone());
                ev.push(RunEvent::RelicGained { player: p as u8, relic: relic.clone() });
            }
            // nothing to choose: this player is finished already
            if r.cards.is_empty() {
                r.done = true;
            }
        }
        ev.push(RunEvent::Rewards);
        if rewards.iter().all(|r| r.done) {
            self.stage = Stage::Reward { rewards };
            self.to_map(c, ev);
        } else {
            self.stage = Stage::Reward { rewards };
        }
    }

    /// Three different cards of the player's own hero; co-op cards only in a party.
    fn roll_cards(&mut self, c: &Content, p: usize, kind: NodeKind) -> Vec<Id> {
        let hero = &self.players[p].hero;
        let coop = self.players.len() > 1;
        let pool = |rarity: Rarity| -> Vec<&CardDef> {
            c.cards
                .values()
                .filter(|d| &d.hero == hero && d.rarity == rarity && (coop || !d.keywords.contains(&Keyword::Coop)))
                .filter(|d| !matches!(d.kind, CardType::Curse | CardType::Status))
                .filter(|d| self.unlocks.card_ok(p, d.rarity))
                .collect()
        };
        let (common, uncommon, rare) = (pool(Rarity::Common), pool(Rarity::Uncommon), pool(Rarity::Rare));
        // elites and bosses lean toward the better shelves
        let (w_unc, w_rare) = match kind {
            NodeKind::Boss => (40, 60),
            NodeKind::Elite => (45, 12),
            _ => (37, 3),
        };
        let mut out: Vec<Id> = Vec::new();
        for _ in 0..40 {
            if out.len() == 3 {
                break;
            }
            let roll = self.rng.below(100) as i32;
            let shelf = if roll < w_rare { &rare } else if roll < w_rare + w_unc { &uncommon } else { &common };
            // an empty shelf falls through to whatever exists
            let shelf = [shelf, &uncommon, &common, &rare].into_iter().find(|s| !s.is_empty());
            let Some(shelf) = shelf else { break };
            if let Some(card) = self.rng.pick(shelf) {
                if !out.contains(&card.id) {
                    out.push(card.id.clone());
                }
            }
        }
        out
    }

    fn apply_option(&mut self, c: &Content, p: usize, option: &EventOption, ev: &mut Vec<RunEvent>) {
        let player = p as u8;
        if option.price > 0 {
            self.players[p].gold = (self.players[p].gold - option.price).max(0);
            ev.push(RunEvent::Gold { player, delta: -option.price });
        }
        for fx in &option.fx {
            match fx {
                RunFx::Gold(n) => {
                    let pl = &mut self.players[p];
                    let delta = (*n).max(-pl.gold);
                    pl.gold += delta;
                    ev.push(RunEvent::Gold { player, delta });
                }
                RunFx::Hp(n) | RunFx::HpPct(n) => {
                    let pl = &mut self.players[p];
                    let n = if matches!(fx, RunFx::HpPct(_)) { pl.max_hp * n / 100 } else { *n };
                    // an omen may wound, but it never kills outright
                    let delta = n.clamp(1 - pl.hp, pl.max_hp - pl.hp);
                    pl.hp += delta;
                    ev.push(RunEvent::Hp { player, delta });
                }
                RunFx::MaxHp(n) => {
                    let pl = &mut self.players[p];
                    pl.max_hp = (pl.max_hp + n).max(1);
                    pl.hp = (pl.hp + (*n).max(0)).min(pl.max_hp).max(1);
                }
                RunFx::Card(card) => {
                    self.players[p].deck.push((card.clone(), false));
                    ev.push(RunEvent::CardGained { player, card: card.clone() });
                }
                RunFx::RandomCard(rarity) => {
                    let hero = self.players[p].hero.clone();
                    let coop = self.players.len() > 1;
                    let pool: Vec<&CardDef> =
                        c.cards.values().filter(|d| d.hero == hero && d.rarity == *rarity && (coop || !d.keywords.contains(&Keyword::Coop)) && self.unlocks.card_ok(p, d.rarity)).collect();
                    if let Some(card) = self.rng.pick(&pool).map(|d| d.id.clone()) {
                        self.players[p].deck.push((card.clone(), false));
                        ev.push(RunEvent::CardGained { player, card });
                    }
                }
                RunFx::Relic => {
                    if let Some(relic) = self.roll_relic(c, p) {
                        self.players[p].relics.push(relic.clone());
                        ev.push(RunEvent::RelicGained { player, relic });
                    }
                }
                RunFx::Potion => self.give_potion(c, p, ev),
                RunFx::RemoveRandomCard => {
                    let n = self.players[p].deck.len();
                    if n > 1 {
                        let i = self.rng.below(n as u32) as usize;
                        let (card, _) = self.players[p].deck.remove(i);
                        ev.push(RunEvent::CardRemoved { player, card });
                    }
                }
                RunFx::UpgradeRandomCard => {
                    let able: Vec<usize> = self.players[p].deck.iter().enumerate().filter(|(_, (id, up))| !up && c.cards.get(id).is_some_and(|d| d.up.is_some())).map(|(i, _)| i).collect();
                    if let Some(i) = self.rng.pick(&able).copied() {
                        self.players[p].deck[i].1 = true;
                        ev.push(RunEvent::Upgraded { player, card: self.players[p].deck[i].0.clone() });
                    }
                }
            }
        }
    }

    /// A random potion, if the player has a free slot.
    fn give_potion(&mut self, c: &Content, p: usize, ev: &mut Vec<RunEvent>) {
        if self.players[p].potions.len() >= POTION_SLOTS {
            return;
        }
        let open = 10 * (self.unlocks.potion_tier as usize + 1);
        let pool: Vec<&Id> = c.potions.keys().take(open).collect();
        if let Some(potion) = self.rng.pick(&pool).map(|id| (*id).clone()) {
            self.players[p].potions.push(potion.clone());
            ev.push(RunEvent::PotionGained { player: p as u8, potion });
        }
    }

    /// The pedlar's shelves for one player: 5 cards of their hero, 2 neutral, 3 relics, 3 potions.
    fn roll_stock(&mut self, c: &Content, p: usize) -> Stock {
        let hero = self.players[p].hero.clone();
        let coop = self.players.len() > 1;
        let price = |rng: &mut Rng, rarity: Rarity| match rarity {
            Rarity::Rare => rng.range(135, 165),
            Rarity::Uncommon => rng.range(68, 82),
            _ => rng.range(45, 55),
        };
        let mut stock = Stock::default();
        let mine: Vec<&CardDef> = c
            .cards
            .values()
            .filter(|d| d.hero == hero && !matches!(d.rarity, Rarity::Starter | Rarity::Special) && (coop || !d.keywords.contains(&Keyword::Coop)))
            .filter(|d| self.unlocks.card_ok(p, d.rarity))
            .collect();
        let neutral: Vec<&CardDef> = c.cards.values().filter(|d| d.hero.is_empty() && !matches!(d.kind, CardType::Curse | CardType::Status) && d.rarity != Rarity::Special).collect();
        for (pool, want) in [(&mine, 5), (&neutral, 2)] {
            let mut picked: Vec<&Id> = Vec::new();
            for _ in 0..40 {
                if picked.len() == want.min(pool.len()) {
                    break;
                }
                if let Some(d) = self.rng.pick(pool) {
                    if !picked.contains(&&d.id) {
                        picked.push(&d.id);
                        let cost = price(&mut self.rng, d.rarity);
                        stock.cards.push(Some((d.id.clone(), cost)));
                    }
                }
            }
        }
        let owned = self.players[p].relics.clone();
        let relics: Vec<&RelicDef> =
            c.relics.values().filter(|r| r.rarity != Rarity::Starter && r.pool != RelicPool::Boss && (r.hero.is_empty() || r.hero == hero) && !owned.contains(&r.id) && self.unlocks.relic_ok(r.rarity)).collect();
        // the first shelf is the pedlar's own: something found nowhere else
        let wares: Vec<&RelicDef> = relics.iter().copied().filter(|r| r.pool == RelicPool::Shop).collect();
        if let Some(r) = self.rng.pick(&wares) {
            let cost = price(&mut self.rng, r.rarity) + 80;
            stock.relics.push(Some((r.id.clone(), cost)));
        }
        for _ in 0..40 {
            if stock.relics.len() == 3.min(relics.len()) {
                break;
            }
            if let Some(r) = self.rng.pick(&relics) {
                if !stock.relics.iter().flatten().any(|(id, _)| *id == r.id) {
                    let cost = price(&mut self.rng, r.rarity) + 80;
                    stock.relics.push(Some((r.id.clone(), cost)));
                }
            }
        }
        let potions: Vec<&Id> = c.potions.keys().take(10 * (self.unlocks.potion_tier as usize + 1)).collect();
        for _ in 0..3.min(potions.len()) {
            if let Some(id) = self.rng.pick(&potions) {
                let cost = self.rng.range(45, 60);
                stock.potions.push(Some(((*id).clone(), cost)));
            }
        }
        stock
    }

    /// A relic this player doesn't own yet: common pool, or their hero's own.
    fn roll_relic(&mut self, c: &Content, p: usize) -> Option<Id> {
        let pl = &self.players[p];
        let pool: Vec<&RelicDef> = c
            .relics
            .values()
            .filter(|r| r.rarity != Rarity::Starter && r.pool == RelicPool::Any && (r.hero.is_empty() || r.hero == pl.hero))
            .filter(|r| !pl.relics.contains(&r.id) && self.unlocks.relic_ok(r.rarity))
            .collect();
        self.rng.pick(&pool).map(|r| r.id.clone())
    }
}

/// A sensible action for a seat nobody is steering.
pub fn run_bot(c: &Content, run: &Run, player: u8) -> Option<RunAction> {
    let p = player as usize;
    match &run.stage {
        Stage::Map { votes } => votes.get(p)?.is_none().then_some(RunAction::Vote(0)),
        Stage::Combat => {
            let combat = run.combat.as_ref()?;
            let h = combat.hero(player)?;
            (combat.phase == Phase::Heroes && !h.downed && !h.ready).then(|| RunAction::Combat(crate::bot::choose(c, combat, player)))
        }
        Stage::Reward { rewards } => {
            let r = rewards.get(p)?;
            (!r.done).then(|| RunAction::TakeCard(if r.cards.is_empty() { None } else { Some(0) }))
        }
        Stage::Rest { done } => {
            let pl = run.players.get(p)?;
            if *done.get(p)? {
                return None;
            }
            // hurt heroes sleep, healthy ones sharpen a card
            let upgrade = pl.deck.iter().position(|(id, up)| !up && c.cards.get(id).is_some_and(|d| d.up.is_some() && d.kind != CardType::Curse));
            Some(match upgrade {
                Some(i) if pl.hp * 10 >= pl.max_hp * 7 => RunAction::Upgrade(i as u16),
                _ => RunAction::Sleep,
            })
        }
        Stage::Shop { stock, done } => {
            if *done.get(p)? {
                return None;
            }
            // the autopilot buys the first thing it can afford, once, and moves on
            let pl = run.players.get(p)?;
            let afford = |shelf: &Vec<Option<(Id, i32)>>| shelf.iter().position(|s| s.as_ref().is_some_and(|(_, price)| *price <= pl.gold)).map(|i| i as u8);
            let s = stock.get(p)?;
            if let Some(i) = afford(&s.relics) {
                return Some(RunAction::BuyRelic(i));
            }
            if let Some(i) = afford(&s.cards) {
                return Some(RunAction::BuyCard(i));
            }
            Some(RunAction::LeaveShop)
        }
        Stage::Omen { id, picks } => {
            let def = c.events.get(id.as_str())?;
            if picks.get(p)?.is_some() {
                return None;
            }
            let pl = run.players.get(p)?;
            // the first option it can pay for (and, at a schism, that is still open to it)
            let closed = if def.kind == EventKind::Schism { schism_closed(picks, p) } else { None };
            let i = (0..def.options.len()).find(|i| def.options[*i].price <= pl.gold && closed != Some(*i as u8)).unwrap_or(def.options.len() - 1);
            Some(RunAction::Choose(i as u8))
        }
        Stage::Trophy { picks, .. } => picks.get(p)?.is_none().then_some(RunAction::Choose(0)),
        Stage::Victory | Stage::Defeat => None,
    }
}
