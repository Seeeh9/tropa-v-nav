//! The fight in progress as the client sees it: the authoritative state, a
//! display copy that lags behind while events are being animated, and the
//! queue between them.

use bevy::prelude::*;
use nav_core::*;
use nav_core::Event;
use std::collections::VecDeque;

#[derive(Resource)]
pub struct Db(pub Content);

/// What to fight next; consumed when the battle screen opens.
#[derive(Resource, Clone)]
pub struct PendingBattle {
    pub party: Vec<HeroSetup>,
    pub enemies: Vec<Id>,
    pub seed: u64,
    pub me: u8,
    /// Slots played by the autopilot on this machine.
    pub bots: Vec<u8>,
}

#[derive(Resource)]
pub struct Session {
    /// The real state. Hand, energy and legality checks read this.
    pub truth: Combat,
    /// What the unit widgets show; catches up with `truth` event by event.
    pub shown: Combat,
    pub me: u8,
    pub bots: Vec<u8>,
    pub queue: VecDeque<Event>,
    pub wait: f32,
    pub bot_wait: f32,
    pub error: Option<(String, f32)>,
}

/// Actions the local player wants to take this frame.
#[derive(Resource, Default)]
pub struct Outbox(pub Vec<CombatAction>);

/// Events released to the screen this frame, for floaters and animations.
#[derive(Resource, Default)]
pub struct Cues(pub Vec<Event>);

/// Animation speed multiplier (1x / 1.5x / 2x). Never touches game state.
#[derive(Resource)]
pub struct AnimSpeed(pub f32);

impl Session {
    pub fn new(db: &Content, p: &PendingBattle) -> Session {
        let (truth, events) = start(db, &p.party, &p.enemies, p.seed);
        Session {
            shown: truth.clone(),
            truth,
            me: p.me,
            bots: p.bots.clone(),
            // the opening events already happened in `shown`; replay only the cosmetic ones
            queue: events.into_iter().filter(|e| matches!(e, Event::RoundStart(_))).collect(),
            wait: 0.0,
            bot_wait: 0.6,
            error: None,
        }
    }

    /// A fight whose state came from the network.
    pub fn from_state(state: Combat, me: u8) -> Session {
        Session { shown: state.clone(), truth: state, me, bots: vec![], queue: VecDeque::new(), wait: 0.0, bot_wait: 0.6, error: None }
    }

    pub fn my_hero(&self) -> &Hero {
        &self.truth.heroes[self.me as usize]
    }

    pub fn idle(&self) -> bool {
        self.queue.is_empty()
    }

    fn act(&mut self, db: &Content, slot: u8, action: &CombatAction) -> Result<(), ActionError> {
        let events = apply(db, &mut self.truth, slot, action)?;
        self.queue.extend(events);
        Ok(())
    }
}

/// How long the screen lingers on an event before showing the next.
fn delay(e: &Event) -> f32 {
    match e {
        Event::CardPlayed { .. } => 0.28,
        Event::Damage { attack: true, .. } => 0.32,
        Event::Damage { .. } => 0.22,
        Event::EnemyMove { .. } => 0.35,
        Event::EnemySkipped { .. } => 0.4,
        Event::EnemyPhase => 0.35,
        Event::Block { .. } | Event::Heal { .. } => 0.14,
        Event::Downed { .. } | Event::Raised { .. } => 0.6,
        Event::EnemyDied { .. } => 0.25,
        Event::RuneFired { .. } | Event::Shifted { .. } => 0.25,
        Event::RoundStart(_) => 0.3,
        Event::Ended { .. } => 0.5,
        _ => 0.0,
    }
}

/// Folds one event into the display copy.
fn show(shown: &mut Combat, truth: &Combat, e: &Event) {
    let set = |shown: &mut Combat, u: UnitRef, hp: Option<i32>, block: Option<i32>| match u {
        UnitRef::Hero(i) => {
            if let Some(h) = shown.heroes.get_mut(i as usize) {
                hp.map(|v| h.hp = v);
                block.map(|v| h.block = v);
            }
        }
        UnitRef::Enemy(i) => {
            if let Some(en) = shown.enemies.iter_mut().find(|x| x.uid == i) {
                hp.map(|v| en.hp = v);
                block.map(|v| en.block = v);
            }
        }
    };
    match e {
        Event::Damage { dst, hp, block, .. } => set(shown, *dst, Some(*hp), Some(*block)),
        Event::Block { unit, total, .. } => set(shown, *unit, None, Some(*total)),
        Event::Heal { unit, hp, .. } => set(shown, *unit, Some(*hp), None),
        Event::Status { unit, status, total, .. } => {
            let map = match unit {
                UnitRef::Hero(i) => shown.heroes.get_mut(*i as usize).map(|h| &mut h.statuses),
                UnitRef::Enemy(i) => shown.enemies.iter_mut().find(|x| x.uid == *i).map(|x| &mut x.statuses),
            };
            if let Some(map) = map {
                if *total == 0 {
                    map.remove(status);
                } else {
                    map.insert(*status, *total);
                }
            }
        }
        Event::Downed { player } => {
            if let Some(h) = shown.heroes.get_mut(*player as usize) {
                h.downed = true;
                h.hp = 0;
                h.block = 0;
            }
        }
        Event::Raised { player, hp, .. } => {
            if let Some(h) = shown.heroes.get_mut(*player as usize) {
                h.downed = false;
                h.hp = *hp;
            }
        }
        Event::EnemyDied { enemy } | Event::EnemyFled { enemy } => {
            if let Some(en) = shown.enemies.iter_mut().find(|x| x.uid == *enemy) {
                en.alive = false;
                en.intent = None;
            }
        }
        Event::Summoned { enemy, .. } => {
            if shown.enemy(*enemy).is_none() {
                if let Some(en) = truth.enemy(*enemy) {
                    let mut en = en.clone();
                    en.hp = en.max_hp;
                    en.alive = true;
                    en.intent = None;
                    shown.enemies.push(en);
                }
            }
        }
        Event::Intent { enemy, mv, target } => {
            if let Some(en) = shown.enemies.iter_mut().find(|x| x.uid == *enemy) {
                en.intent = Some(Intent { mv: *mv, target: *target });
            }
        }
        Event::Taunt { player } => {
            for en in shown.enemies.iter_mut() {
                if let Some(i) = &mut en.intent {
                    if i.target.is_some() {
                        i.target = Some(*player);
                    }
                }
            }
        }
        Event::EnemyMove { enemy, .. } | Event::EnemySkipped { enemy, .. } => {
            if let Some(en) = shown.enemies.iter_mut().find(|x| x.uid == *enemy) {
                en.intent = None;
            }
        }
        Event::RoundStart(n) => shown.round = *n,
        Event::RuneCarved { player, rune } => {
            if let Some(h) = shown.heroes.get_mut(*player as usize) {
                h.runes.push(*rune);
            }
        }
        Event::Shifted { player, form } => {
            if let Some(h) = shown.heroes.get_mut(*player as usize) {
                h.form = *form;
            }
        }
        Event::MaskChanged { player, mask } => {
            if let Some(h) = shown.heroes.get_mut(*player as usize) {
                h.mask = Some(*mask);
            }
        }
        _ => {}
    }
}

pub fn submit(
    mut session: ResMut<Session>,
    mut outbox: ResMut<Outbox>,
    db: Res<Db>,
    l10n: Res<crate::l10n::L10n>,
    mut net: Option<ResMut<crate::net::Net>>,
) {
    let me = session.me;
    for action in outbox.0.drain(..) {
        // in a party the host decides; the result comes back as a protocol note
        if let Some(net) = net.as_mut() {
            net.node.submit(&db.0, action);
            continue;
        }
        if let Err(e) = session.act(&db.0, me, &action) {
            session.error = Some((l10n.t(&format!("error-{e:?}")), 1.6));
        }
    }
}

pub fn run_bots(mut session: ResMut<Session>, db: Res<Db>, time: Res<Time>) {
    if session.bots.is_empty() || session.truth.phase != Phase::Heroes || !session.idle() {
        return;
    }
    session.bot_wait -= time.delta_secs();
    if session.bot_wait > 0.0 {
        return;
    }
    session.bot_wait = 0.45;
    let next = session.bots.iter().copied().find(|b| session.truth.hero(*b).is_some_and(|h| !h.downed && !h.ready));
    if let Some(slot) = next {
        let action = bot::choose(&db.0, &session.truth, slot);
        let _ = session.act(&db.0, slot, &action);
    }
}

pub fn playback(mut session: ResMut<Session>, mut cues: ResMut<Cues>, time: Res<Time>, speed: Res<AnimSpeed>) {
    cues.0.clear();
    if let Some((_, ttl)) = &mut session.error {
        *ttl -= time.delta_secs();
        if *ttl <= 0.0 {
            session.error = None;
        }
    }
    if session.queue.is_empty() {
        return;
    }
    session.wait -= time.delta_secs() * speed.0;
    while session.wait <= 0.0 {
        let Some(e) = session.queue.pop_front() else { break };
        let s = &mut *session;
        show(&mut s.shown, &s.truth, &e);
        s.wait += delay(&e);
        cues.0.push(e);
    }
    if session.queue.is_empty() {
        // whatever the events didn't carry is corrected here
        session.shown = session.truth.clone();
        session.wait = 0.0;
    }
}
