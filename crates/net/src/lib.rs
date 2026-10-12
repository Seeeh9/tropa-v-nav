//! Co-op protocol, independent of any transport.
//!
//! The host is the single source of truth. A client sends the action it wants;
//! the host validates it against its own state, applies it, and broadcasts it
//! with a running number. Every machine applies the same numbered actions to
//! the same deterministic rules, so they all hold the same run. Whenever the
//! run moves on (a new round, a new stage, a new floor) the host sends a hash;
//! a client that disagrees asks for a full snapshot.
//!
//! A [`Node`] is one machine's view. It never touches the network: it takes
//! incoming messages through [`Node::handle`] and leaves outgoing ones in
//! [`Node::take_out`]. Steam, or the in-memory [`Hub`], moves them.

use nav_core::*;
use serde::{Deserialize, Serialize};

pub type Peer = u64;
pub const MAX_PLAYERS: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyPlayer {
    pub peer: Peer,
    pub name: String,
    pub hero: Option<Id>,
    /// The player's level with that hero; it decides which of their cards may turn up.
    #[serde(default)]
    pub level: u32,
    pub ready: bool,
    pub connected: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lobby {
    /// Slot order: index here is the hero slot in combat.
    pub players: Vec<LobbyPlayer>,
    pub host: Peer,
    /// Circle of Nav, 0..=10.
    pub circle: u8,
    /// Optional turn timer in seconds (off by default).
    pub turn_timer: Option<u32>,
    pub started: bool,
}

impl Lobby {
    pub fn slot_of(&self, peer: Peer) -> Option<u8> {
        self.players.iter().position(|p| p.peer == peer).map(|i| i as u8)
    }
    /// Everyone picked a distinct hero and pressed ready.
    pub fn can_start(&self) -> bool {
        !self.players.is_empty() && self.players.iter().all(|p| p.ready && p.hero.is_some())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StartInfo {
    pub seed: u64,
    /// Heroes in slot order.
    pub heroes: Vec<Id>,
    pub circle: u8,
    pub unlocks: Unlocks,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Msg {
    /// Client -> host on joining or rejoining.
    Hello { name: String },
    /// Host -> everyone whenever the lobby changes.
    Lobby(Lobby),
    /// A hero and the player's level with them.
    PickHero(Option<Id>, u32),
    SetReady(bool),
    /// Host -> everyone: the run begins from this exact setup.
    Start(StartInfo),
    /// Client -> host: "I want to do this". `seq` lets the client match a refusal.
    Want { seq: u32, action: RunAction },
    /// Host -> everyone: action number `n` in the shared log.
    Did { n: u32, player: u8, action: RunAction },
    Refused { seq: u32, error: RunError },
    /// Host -> everyone whenever the run moves on.
    Hash { n: u32, hash: u64 },
    /// Client -> host: my hash differs, or I just rejoined.
    Resync,
    /// Host -> one client: the whole state after `n` actions.
    Snapshot { n: u32, lobby: Lobby, state: Box<Run> },
    /// Quick signal without chat, pinned to a unit.
    Ping { kind: u8, at: UnitRef },
}

impl Msg {
    pub fn encode(&self) -> Vec<u8> {
        ron::to_string(self).expect("messages always serialise").into_bytes()
    }
    pub fn decode(bytes: &[u8]) -> Option<Msg> {
        ron::from_str(std::str::from_utf8(bytes).ok()?).ok()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum To {
    One(Peer),
    /// Everyone in the lobby except the sender.
    All,
}

/// Things the UI should react to.
#[derive(Clone, Debug, PartialEq)]
pub enum Note {
    LobbyChanged,
    Started,
    /// An action was applied; these are its rule events.
    Applied { player: u8, events: Vec<RunEvent> },
    Refused(RunError),
    /// State was replaced wholesale; redraw without animation.
    Resynced,
    Ping { from: u8, kind: u8, at: UnitRef },
    HostChanged,
}

pub struct Node {
    pub me: Peer,
    pub name: String,
    pub lobby: Lobby,
    pub state: Option<Run>,
    /// Actions applied so far.
    pub n: u32,
    seq: u32,
    /// Wants sent to the host and not yet answered, resent if the host changes.
    pending: Vec<(u32, RunAction)>,
    last_mark: u64,
    out: Vec<(To, Msg)>,
    notes: Vec<Note>,
}

impl Node {
    /// Opens a lobby as its host.
    pub fn host(me: Peer, name: &str) -> Node {
        let mut node = Node::blank(me, name);
        node.lobby.host = me;
        node.lobby.players.push(LobbyPlayer { peer: me, name: name.into(), hero: None, level: 1, ready: false, connected: true });
        node
    }

    /// Reopens a saved solo run as its host.
    pub fn resume(me: Peer, name: &str, hero: Id, run: Run) -> Node {
        let mut node = Node::host(me, name);
        node.lobby.players[0].hero = Some(hero);
        node.lobby.players[0].ready = true;
        node.lobby.circle = run.circle;
        node.lobby.started = true;
        node.last_mark = mark(&run);
        node.state = Some(run);
        node.notes.push(Note::Resynced);
        node
    }

    /// Joins the lobby hosted by `host`.
    pub fn join(me: Peer, name: &str, host: Peer) -> Node {
        let mut node = Node::blank(me, name);
        node.lobby.host = host;
        node.out.push((To::One(host), Msg::Hello { name: name.into() }));
        node
    }

    fn blank(me: Peer, name: &str) -> Node {
        Node {
            me,
            name: name.into(),
            lobby: Lobby::default(),
            state: None,
            n: 0,
            seq: 0,
            pending: Vec::new(),
            last_mark: 0,
            out: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn is_host(&self) -> bool {
        self.lobby.host == self.me
    }
    pub fn my_slot(&self) -> Option<u8> {
        self.lobby.slot_of(self.me)
    }
    pub fn take_out(&mut self) -> Vec<(To, Msg)> {
        std::mem::take(&mut self.out)
    }
    pub fn take_notes(&mut self) -> Vec<Note> {
        std::mem::take(&mut self.notes)
    }

    fn broadcast_lobby(&mut self) {
        self.out.push((To::All, Msg::Lobby(self.lobby.clone())));
        self.notes.push(Note::LobbyChanged);
    }

    // ---- lobby, local side ----

    pub fn pick_hero(&mut self, hero: Option<Id>, level: u32) {
        if self.is_host() {
            self.host_pick(self.me, hero, level);
        } else {
            self.out.push((To::One(self.lobby.host), Msg::PickHero(hero, level)));
        }
    }

    pub fn set_ready(&mut self, ready: bool) {
        if self.is_host() {
            self.host_ready(self.me, ready);
        } else {
            self.out.push((To::One(self.lobby.host), Msg::SetReady(ready)));
        }
    }

    fn host_pick(&mut self, peer: Peer, hero: Option<Id>, level: u32) {
        if self.lobby.started {
            return;
        }
        // heroes are unique in a party: the colour is the player's identity
        let taken = hero.as_ref().is_some_and(|h| self.lobby.players.iter().any(|p| p.peer != peer && p.hero.as_ref() == Some(h)));
        if taken {
            return;
        }
        if let Some(p) = self.lobby.players.iter_mut().find(|p| p.peer == peer) {
            p.hero = hero;
            p.level = level.clamp(1, 10);
            p.ready = false;
            self.broadcast_lobby();
        }
    }

    fn host_ready(&mut self, peer: Peer, ready: bool) {
        if self.lobby.started {
            return;
        }
        if let Some(p) = self.lobby.players.iter_mut().find(|p| p.peer == peer) {
            p.ready = ready && p.hero.is_some();
            self.broadcast_lobby();
        }
    }

    /// Host only: choose the Circle of Nav for the run about to start.
    pub fn set_circle(&mut self, circle: u8) {
        if self.is_host() && !self.lobby.started && self.lobby.circle != circle.min(10) {
            self.lobby.circle = circle.min(10);
            self.broadcast_lobby();
        }
    }

    /// Host only: the optional turn timer in seconds (`None` is off, the default).
    pub fn set_turn_timer(&mut self, seconds: Option<u32>) {
        if self.is_host() && self.lobby.turn_timer != seconds {
            self.lobby.turn_timer = seconds;
            self.broadcast_lobby();
        }
    }

    /// Host only: begin the run. Returns false if the lobby isn't ready.
    /// `village` is the host's: its smithy, hut and tavern decide the shared pools.
    pub fn start(&mut self, c: &Content, seed: u64, village: (u8, u8, u8)) -> bool {
        if !self.is_host() || !self.lobby.can_start() || self.lobby.started {
            return false;
        }
        let heroes: Option<Vec<Id>> = self.lobby.players.iter().map(|p| p.hero.clone().filter(|h| c.heroes.contains_key(h))).collect();
        let Some(heroes) = heroes else { return false };
        self.lobby.started = true;
        let unlocks = Unlocks { relic_tier: village.0, potion_tier: village.1, omen_tier: village.2, hero_levels: self.lobby.players.iter().map(|p| p.level.max(1)).collect() };
        let info = StartInfo { seed, heroes, circle: self.lobby.circle, unlocks };
        self.out.push((To::All, Msg::Lobby(self.lobby.clone())));
        self.out.push((To::All, Msg::Start(info.clone())));
        self.begin(c, &info);
        true
    }

    fn begin(&mut self, c: &Content, info: &StartInfo) {
        let Some(state) = Run::new_with(c, &info.heroes, info.seed, info.circle, info.unlocks.clone()) else { return };
        self.last_mark = mark(&state);
        self.state = Some(state);
        self.n = 0;
        self.pending.clear();
        self.lobby.started = true;
        self.notes.push(Note::Started);
    }

    // ---- actions ----

    /// The local player wants to act.
    pub fn submit(&mut self, c: &Content, action: RunAction) {
        let Some(slot) = self.my_slot() else { return };
        if self.is_host() {
            if let Err(e) = self.host_apply(c, slot, action) {
                self.notes.push(Note::Refused(e));
            }
        } else {
            self.seq += 1;
            self.pending.push((self.seq, action.clone()));
            self.out.push((To::One(self.lobby.host), Msg::Want { seq: self.seq, action }));
        }
    }

    /// Host: validate, apply, broadcast.
    fn host_apply(&mut self, c: &Content, slot: u8, action: RunAction) -> Result<(), RunError> {
        let state = self.state.as_mut().ok_or(RunError::WrongStage)?;
        let events = state.apply(c, slot, &action)?;
        self.n += 1;
        self.out.push((To::All, Msg::Did { n: self.n, player: slot, action }));
        self.notes.push(Note::Applied { player: slot, events });
        let state = self.state.as_ref().unwrap();
        if mark(state) != self.last_mark {
            self.last_mark = mark(state);
            self.out.push((To::All, Msg::Hash { n: self.n, hash: state.hash64() }));
        }
        Ok(())
    }

    /// Host: play one autopilot move for a disconnected player, if any is due.
    /// Call it on a timer; returns whether a move was made.
    pub fn autopilot(&mut self, c: &Content) -> bool {
        if !self.is_host() {
            return false;
        }
        let Some(state) = &self.state else { return false };
        let due = self.lobby.players.iter().enumerate().find_map(|(i, p)| if p.connected { None } else { run_bot(c, state, i as u8).map(|a| (i as u8, a)) });
        let Some((slot, action)) = due else { return false };
        self.host_apply(c, slot, action).is_ok()
    }

    pub fn ping(&mut self, kind: u8, at: UnitRef) {
        if let Some(from) = self.my_slot() {
            self.out.push((To::All, Msg::Ping { kind, at }));
            self.notes.push(Note::Ping { from, kind, at });
        }
    }

    // ---- membership ----

    /// The transport saw this peer leave the lobby.
    pub fn peer_left(&mut self, peer: Peer) {
        if self.lobby.started {
            if let Some(p) = self.lobby.players.iter_mut().find(|p| p.peer == peer) {
                p.connected = false;
            }
        } else {
            self.lobby.players.retain(|p| p.peer != peer);
        }
        if self.is_host() {
            self.broadcast_lobby();
        } else {
            self.notes.push(Note::LobbyChanged);
        }
    }

    /// The transport says the lobby has a new owner (the old host left).
    pub fn host_changed(&mut self, c: &Content, new_host: Peer) {
        if self.lobby.host == new_host {
            return;
        }
        self.lobby.host = new_host;
        self.notes.push(Note::HostChanged);
        if self.is_host() {
            // everyone holds the full state, so the new host simply carries on
            self.broadcast_lobby();
            // what this machine asked the old host for, it now decides itself
            if let Some(slot) = self.my_slot() {
                for (_, action) in std::mem::take(&mut self.pending) {
                    let _ = self.host_apply(c, slot, action);
                }
            }
            if let Some(state) = &self.state {
                self.out.push((To::All, Msg::Hash { n: self.n, hash: state.hash64() }));
            }
        } else {
            // whatever the old host never answered goes to the new one
            for (seq, action) in self.pending.clone() {
                self.out.push((To::One(new_host), Msg::Want { seq, action }));
            }
        }
    }

    // ---- incoming ----

    pub fn handle(&mut self, c: &Content, from: Peer, msg: Msg) {
        let host = self.is_host();
        match msg {
            Msg::Hello { name } if host => {
                if let Some(p) = self.lobby.players.iter_mut().find(|p| p.peer == from) {
                    // a returning player takes the wheel back from the autopilot
                    p.connected = true;
                    p.name = name;
                } else if !self.lobby.started && self.lobby.players.len() < MAX_PLAYERS {
                    self.lobby.players.push(LobbyPlayer { peer: from, name, hero: None, level: 1, ready: false, connected: true });
                } else {
                    return;
                }
                self.broadcast_lobby();
                if let Some(state) = &self.state {
                    self.out.push((To::One(from), Msg::Snapshot { n: self.n, lobby: self.lobby.clone(), state: Box::new(state.clone()) }));
                }
            }
            Msg::PickHero(hero, level) if host => self.host_pick(from, hero, level),
            Msg::SetReady(ready) if host => self.host_ready(from, ready),
            Msg::Want { seq, action } if host => {
                let Some(slot) = self.lobby.slot_of(from) else { return };
                if let Err(error) = self.host_apply(c, slot, action) {
                    self.out.push((To::One(from), Msg::Refused { seq, error }));
                }
            }
            Msg::Resync if host => {
                if let Some(state) = &self.state {
                    self.out.push((To::One(from), Msg::Snapshot { n: self.n, lobby: self.lobby.clone(), state: Box::new(state.clone()) }));
                }
            }
            Msg::Ping { kind, at } => {
                if let Some(slot) = self.lobby.slot_of(from) {
                    self.notes.push(Note::Ping { from: slot, kind, at });
                }
            }

            // everything below is only believed when it comes from the host
            _ if from != self.lobby.host => {}
            Msg::Lobby(lobby) => {
                self.lobby = lobby;
                self.notes.push(Note::LobbyChanged);
            }
            Msg::Start(info) => self.begin(c, &info),
            Msg::Did { n, player, action } => {
                if n != self.n + 1 {
                    // a gap or a repeat: our log no longer lines up with the host's
                    if n > self.n {
                        self.out.push((To::One(self.lobby.host), Msg::Resync));
                    }
                    return;
                }
                let Some(state) = self.state.as_mut() else { return };
                match state.apply(c, player, &action) {
                    Ok(events) => {
                        self.n = n;
                        if Some(player) == self.lobby.slot_of(self.me) {
                            // confirmed in order, so the oldest pending want is this one
                            if let Some(i) = self.pending.iter().position(|(_, a)| *a == action) {
                                self.pending.remove(i);
                            }
                        }
                        self.notes.push(Note::Applied { player, events });
                    }
                    // the host accepted what our state refuses: we have drifted
                    Err(_) => self.out.push((To::One(self.lobby.host), Msg::Resync)),
                }
            }
            Msg::Refused { seq, error } => {
                self.pending.retain(|(s, _)| *s != seq);
                self.notes.push(Note::Refused(error));
            }
            Msg::Hash { n, hash } => {
                if n == self.n && self.state.as_ref().is_some_and(|s| s.hash64() != hash) {
                    self.out.push((To::One(self.lobby.host), Msg::Resync));
                }
            }
            Msg::Snapshot { n, lobby, state } => {
                self.lobby = lobby;
                self.state = Some(*state);
                self.n = n;
                self.pending.clear();
                self.notes.push(Note::Resynced);
            }
            _ => {}
        }
    }
}

/// A cheap fingerprint of "where the run is": it changes on a new round, stage or floor.
fn mark(run: &Run) -> u64 {
    let stage = match &run.stage {
        Stage::Map { .. } => 1,
        Stage::Combat => 2,
        Stage::Reward { .. } => 3,
        Stage::Rest { .. } => 4,
        Stage::Shop { .. } => 7,
        Stage::Omen { .. } => 8,
        Stage::Trophy { .. } => 9,
        Stage::Victory => 5,
        Stage::Defeat => 6,
    };
    let round = run.combat.as_ref().map(|c| c.round as u64).unwrap_or(0);
    let at = run.at.map(|(f, n)| f as u64 * 16 + n as u64 + 1).unwrap_or(0);
    stage | round << 8 | at << 40 | (run.act as u64) << 56
}

/// In-memory transport: several nodes in one process, for tests and for the
/// dev build's "four clients in one window" mode.
#[derive(Default)]
pub struct Hub {
    pub nodes: Vec<Node>,
    /// Peers whose messages are currently dropped in both directions.
    pub offline: Vec<Peer>,
}

impl Hub {
    pub fn node(&mut self, peer: Peer) -> &mut Node {
        self.nodes.iter_mut().find(|n| n.me == peer).expect("known peer")
    }

    /// Delivers messages until every outbox is empty.
    pub fn pump(&mut self, c: &Content) {
        for _ in 0..10_000 {
            let mut mail: Vec<(Peer, Peer, Msg)> = Vec::new();
            let peers: Vec<Peer> = self.nodes.iter().map(|n| n.me).collect();
            for node in &mut self.nodes {
                let from = node.me;
                for (to, msg) in node.take_out() {
                    match to {
                        To::One(p) => mail.push((from, p, msg)),
                        To::All => mail.extend(peers.iter().filter(|p| **p != from).map(|p| (from, *p, msg.clone()))),
                    }
                }
            }
            if mail.is_empty() {
                return;
            }
            for (from, to, msg) in mail {
                if self.offline.contains(&from) || self.offline.contains(&to) {
                    continue;
                }
                // through bytes, exactly as the real transport does it
                let Some(msg) = Msg::decode(&msg.encode()) else { panic!("message does not round-trip") };
                if let Some(node) = self.nodes.iter_mut().find(|n| n.me == to) {
                    node.handle(c, from, msg);
                }
            }
        }
        panic!("message storm: nodes never went quiet");
    }
}
