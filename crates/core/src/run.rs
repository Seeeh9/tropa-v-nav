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

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stage {
    /// Choosing the next node; `votes[player]` indexes into [`Run::choices`].
    Map { votes: Vec<Option<u8>> },
    Combat,
    Reward { rewards: Vec<Reward> },
    Rest { done: Vec<bool> },
    Victory,
    Defeat,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Run {
    pub seed: u64,
    pub rng: Rng,
    /// Circle of Nav, 0..=10.
    pub circle: u8,
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
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RunError {
    Combat(ActionError),
    WrongStage,
    NoSuchPlayer,
    BadChoice,
    AlreadyDone,
    NotEnoughGold,
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
    pub fn new(c: &Content, heroes: &[Id], seed: u64, circle: u8) -> Option<Run> {
        let mut rng = Rng::new(seed);
        let circle_rules = Circle(circle);
        let mut players = Vec::new();
        for h in heroes {
            let d = c.heroes.get(h)?;
            let mut deck: Vec<(Id, bool)> = d.deck.iter().map(|id| (id.clone(), false)).collect();
            if circle_rules.starts_with_toska() && c.cards.contains_key("toska") {
                deck.push(("toska".into(), false));
            }
            players.push(Player { hero: h.clone(), hp: d.max_hp, max_hp: d.max_hp, gold: 99, deck, relics: vec![d.relic.clone()], potions: vec![], falls: 0, removals: 0 });
        }
        let map = generate_map(&mut rng, circle_rules);
        let n = players.len();
        Some(Run {
            seed,
            rng,
            circle,
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
                    self.to_map(&mut ev);
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
                    self.to_map(&mut ev);
                }
            }
            (RunAction::GiveGold { to, amount }, Stage::Rest { .. }) => {
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
            _ => return Err(RunError::WrongStage),
        }
        Ok(ev)
    }

    fn to_map(&mut self, ev: &mut Vec<RunEvent>) {
        // past the boss there is no map left in this act
        let boss_done = self.fighting == Some(NodeKind::Boss);
        self.fighting = None;
        self.combat = None;
        if boss_done {
            ev.push(RunEvent::ActCleared { act: self.act });
            if self.act >= 3 {
                self.stage = Stage::Victory;
                ev.push(RunEvent::Ended { won: true });
                return;
            }
            self.act += 1;
            self.at = None;
            self.map = generate_map(&mut self.rng, Circle(self.circle));
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
        if kind != NodeKind::Boss {
            self.at = Some((floor, node));
        }
        ev.push(RunEvent::Moved { floor, node, kind });
        match kind {
            NodeKind::Fight | NodeKind::Elite | NodeKind::Boss => self.start_fight(c, kind, ev),
            NodeKind::Rest => self.stage = Stage::Rest { done: vec![false; self.players.len()] },
            NodeKind::Chest => {
                self.floors_cleared += 1;
                let rewards = (0..self.players.len()).map(|p| Reward { gold: 0, cards: vec![], relic: self.roll_relic(c, p), done: false }).collect();
                self.grant(rewards, ev);
            }
            // shops and omens arrive with their own content; until then the road is simply quiet
            NodeKind::Shop | NodeKind::Event => {
                self.floors_cleared += 1;
                self.stage = Stage::Map { votes: vec![None; self.players.len()] };
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
        let mut enemies = enc.enemies.clone();
        if n == 3 && self.rng.chance(50) {
            enemies.extend(enc.extra3.iter().cloned());
        }
        if n >= 4 {
            enemies.extend(enc.extra4.iter().cloned());
        }
        let party: Vec<HeroSetup> = self
            .players
            .iter()
            .map(|p| HeroSetup { def: p.hero.clone(), hp: p.hp, max_hp: p.max_hp, deck: p.deck.clone(), relics: p.relics.clone(), falls: p.falls })
            .collect();
        let rules = Circle(self.circle);
        let opts = CombatOpts { raise_cost: rules.raise_cost(), enemy_hp: rules.enemy_hp(), enemy_dmg: rules.enemy_dmg(kind) };
        let seed = ((self.rng.next_u32() as u64) << 32) | self.rng.next_u32() as u64;
        let (combat, events) = combat::start_with(c, &party, &enemies, seed, opts);
        self.combat = Some(combat);
        self.fighting = Some(kind);
        self.stage = Stage::Combat;
        ev.push(RunEvent::CombatStarted);
        ev.extend(events.into_iter().map(RunEvent::Combat));
    }

    fn after_fight(&mut self, c: &Content, ev: &mut Vec<RunEvent>) {
        // the finished fight stays readable until the party walks on
        let Some(combat) = self.combat.clone() else { return };
        let kind = self.fighting.unwrap_or(NodeKind::Fight);
        for (pl, h) in self.players.iter_mut().zip(&combat.heroes) {
            pl.hp = h.hp.max(1);
            pl.max_hp = h.max_hp;
            pl.falls = h.falls;
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
        self.grant(rewards, ev);
    }

    /// Hands out gold and relics at once and leaves the card choice to the players.
    fn grant(&mut self, mut rewards: Vec<Reward>, ev: &mut Vec<RunEvent>) {
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
            self.to_map(ev);
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

    /// A relic this player doesn't own yet: common pool, or their hero's own.
    fn roll_relic(&mut self, c: &Content, p: usize) -> Option<Id> {
        let pl = &self.players[p];
        let pool: Vec<&RelicDef> = c
            .relics
            .values()
            .filter(|r| r.rarity != Rarity::Starter && (r.hero.is_empty() || r.hero == pl.hero))
            .filter(|r| !pl.relics.contains(&r.id))
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
        Stage::Victory | Stage::Defeat => None,
    }
}
