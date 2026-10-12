//! Combat rules: the only code allowed to mutate a `Combat`.

use crate::data::*;
use crate::rng::Rng;
use crate::state::*;
use std::collections::BTreeMap;
use StatusKind as S;

/// Where an effect list is running from.
#[derive(Clone, Debug)]
pub(crate) struct Ctx {
    pub src: UnitRef,
    pub target: Option<UnitRef>,
    pub x: i32,
    pub consumed: i32,
    pub fatal: bool,
    /// Damage counts as an attack: Strength, Weak, Hex and friends apply.
    pub attack: bool,
    /// Comes from a card the source played (Fortitude, Beast block ban).
    pub card: bool,
    pub bled: bool,
}

impl Ctx {
    fn passive(src: UnitRef) -> Ctx {
        Ctx { src, target: None, x: 0, consumed: 0, fatal: false, attack: false, card: false, bled: false }
    }
}

pub struct Exec<'a> {
    pub s: &'a mut Combat,
    pub c: &'a Content,
    pub ev: Vec<Event>,
    depth: u8,
}

/// Starts a fight. `enemies` are definition ids, left to right.
pub fn start(c: &Content, heroes: &[HeroSetup], enemies: &[Id], seed: u64) -> (Combat, Vec<Event>) {
    let mut rng = Rng::new(seed);
    let mut next_uid = 1u32;
    let mut hs = Vec::new();
    for (slot, h) in heroes.iter().enumerate() {
        let mut draw: Vec<Card> = h
            .deck
            .iter()
            .map(|(def, upgraded)| {
                next_uid += 1;
                Card { uid: next_uid, def: def.clone(), upgraded: *upgraded, free: false }
            })
            .collect();
        rng.shuffle(&mut draw);
        hs.push(Hero {
            slot: slot as u8,
            def: h.def.clone(),
            mechanic: c.heroes.get(&h.def).map(|d| d.mechanic).unwrap_or(Mechanic::Valor),
            hp: h.hp,
            max_hp: h.max_hp,
            block: 0,
            energy: 0,
            statuses: BTreeMap::new(),
            draw,
            hand: Vec::new(),
            discard: Vec::new(),
            exhaust: Vec::new(),
            relics: h.relics.clone(),
            powers: Vec::new(),
            ready: false,
            downed: h.hp <= 0,
            falls: h.falls,
            new_curses: 0,
            guard_by: None,
            valor_rem: 0,
            runes: Vec::new(),
            form: Form::Human,
            moon: 0,
            mask: None,
            cauldron: Vec::new(),
            gold_lost: 0,
            flags: BTreeMap::new(),
        });
    }
    let mut s = Combat {
        round: 0,
        phase: Phase::Heroes,
        hp_scale: hp_scale(hs.len()),
        heroes: hs,
        enemies: Vec::new(),
        rng,
        next_uid,
        log: Vec::new(),
        raise_cost: 2,
    };
    let mut ex = Exec { s: &mut s, c, ev: Vec::new(), depth: 0 };
    for id in enemies {
        ex.spawn(id);
    }
    for slot in 0..ex.s.heroes.len() as u8 {
        ex.fire(slot, Trigger::CombatStart);
    }
    ex.start_round();
    let ev = ex.ev;
    (s, ev)
}

/// Applies one player action. On `Err` the state is untouched.
pub fn apply(c: &Content, s: &mut Combat, player: u8, action: &CombatAction) -> Result<Vec<Event>, ActionError> {
    if s.phase != Phase::Heroes {
        return Err(ActionError::NotHeroPhase);
    }
    let h = s.hero(player).ok_or(ActionError::NoSuchPlayer)?;
    if h.downed {
        return Err(ActionError::Downed);
    }
    let mut ex = Exec { s, c, ev: Vec::new(), depth: 0 };
    match action {
        CombatAction::Play { card, target } => ex.play(player, *card, *target)?,
        CombatAction::Ready(r) => {
            ex.s.heroes[player as usize].ready = *r;
            ex.ev.push(Event::Ready { player, ready: *r });
        }
        CombatAction::Raise { ally, discard } => ex.raise(player, *ally, *discard)?,
    }
    ex.check_end();
    ex.maybe_close();
    Ok(ex.ev)
}

/// Pure form of [`apply`] for callers that want `reduce(state, action) -> state`.
pub fn reduce(c: &Content, s: &Combat, player: u8, action: &CombatAction) -> Result<(Combat, Vec<Event>), ActionError> {
    let mut next = s.clone();
    let ev = apply(c, &mut next, player, action)?;
    Ok((next, ev))
}

impl<'a> Exec<'a> {
    fn hero(&mut self, slot: u8) -> &mut Hero {
        &mut self.s.heroes[slot as usize]
    }
    fn enemy_idx(&self, uid: u32) -> Option<usize> {
        self.s.enemies.iter().position(|e| e.uid == uid)
    }
    fn standing(&self) -> Vec<u8> {
        self.s.standing_heroes().map(|h| h.slot).collect()
    }
    fn living(&self) -> Vec<u32> {
        self.s.living_enemies().map(|e| e.uid).collect()
    }
    fn exists(&self, u: UnitRef) -> bool {
        match u {
            UnitRef::Hero(i) => self.s.hero(i).is_some_and(|h| !h.downed),
            UnitRef::Enemy(i) => self.s.enemy(i).is_some_and(|e| e.alive),
        }
    }

    pub(crate) fn spawn(&mut self, def: &Id) -> Option<u32> {
        let d = self.c.enemies.get(def)?;
        self.s.next_uid += 1;
        let uid = self.s.next_uid;
        let hp = (self.s.rng.range(d.hp.0, d.hp.1) * self.s.hp_scale + 500) / 1000;
        self.s.enemies.push(Enemy {
            uid,
            def: def.clone(),
            hp,
            max_hp: hp,
            block: 0,
            statuses: BTreeMap::new(),
            alive: true,
            fled: false,
            step: 0,
            intent: None,
            stolen: BTreeMap::new(),
            flags: BTreeMap::new(),
        });
        Some(uid)
    }

    // ---- statuses, block, healing ----

    pub(crate) fn add_status(&mut self, u: UnitRef, st: StatusKind, n: i32) {
        if n == 0 || (n > 0 && !self.exists(u)) {
            return;
        }
        let map = match u {
            UnitRef::Hero(i) => &mut self.s.heroes[i as usize].statuses,
            UnitRef::Enemy(i) => match self.s.enemies.iter_mut().find(|e| e.uid == i) {
                Some(e) => &mut e.statuses,
                None => return,
            },
        };
        let cur = map.get(&st).copied().unwrap_or(0);
        let mut total = cur + n;
        // Strength is the only status that can go below zero (it can be drained)
        if st != S::Strength {
            total = total.max(0);
        }
        if st == S::Fear {
            total = total.min(75);
        }
        if total == 0 {
            map.remove(&st);
        } else {
            map.insert(st, total);
        }
        if total != cur {
            self.ev.push(Event::Status { unit: u, status: st, delta: total - cur, total });
        }
    }

    fn clear_status(&mut self, u: UnitRef, st: StatusKind) -> i32 {
        let cur = self.s.status(u, st);
        self.add_status(u, st, -cur);
        cur
    }

    pub(crate) fn gain_block(&mut self, u: UnitRef, n: i32) {
        if n <= 0 || !self.exists(u) {
            return;
        }
        let total = match u {
            UnitRef::Hero(i) => {
                self.hero(i).block += n;
                self.hero(i).block
            }
            UnitRef::Enemy(i) => {
                let idx = self.enemy_idx(i).unwrap();
                self.s.enemies[idx].block += n;
                self.s.enemies[idx].block
            }
        };
        self.ev.push(Event::Block { unit: u, amount: n, total });
    }

    pub(crate) fn heal(&mut self, u: UnitRef, n: i32) {
        if n <= 0 || !self.exists(u) {
            return;
        }
        let (healed, hp) = match u {
            UnitRef::Hero(i) => {
                let h = self.hero(i);
                let v = n.min(h.max_hp - h.hp);
                h.hp += v;
                (v, h.hp)
            }
            UnitRef::Enemy(i) => {
                let idx = self.enemy_idx(i).unwrap();
                let e = &mut self.s.enemies[idx];
                let v = n.min(e.max_hp - e.hp);
                e.hp += v;
                (v, e.hp)
            }
        };
        if healed > 0 {
            self.ev.push(Event::Heal { unit: u, amount: healed, hp });
        }
    }

    // ---- damage ----

    /// Applies attack modifiers, then lands the hit. Returns true if the target died.
    pub(crate) fn strike(&mut self, src: Option<UnitRef>, dst: UnitRef, base: i32, attack: bool) -> bool {
        if !self.exists(dst) {
            return false;
        }
        let mut d = base;
        if attack {
            let mut beast = false;
            let mut mara = false;
            if let Some(s) = src {
                d += self.s.status(s, S::Strength) + self.s.status(s, S::TempStrength);
                if let UnitRef::Hero(i) = s {
                    let h = &self.s.heroes[i as usize];
                    beast = h.form == Form::Beast;
                    mara = h.mask == Some(MaskKind::Mara);
                    if h.mask == Some(MaskKind::Bear) {
                        d += 3;
                    }
                }
            }
            if self.s.status(dst, S::Hex) > 0 {
                d += 3;
                self.add_status(dst, S::Hex, -1);
            }
            d += self.s.status(dst, S::HuntMark);
            d = d.max(0);
            if beast {
                d = d * 3 / 2;
            }
            if src.is_some_and(|s| self.s.status(s, S::Weak) > 0) {
                d = d * 3 / 4;
            }
            if self.s.status(dst, S::Vulnerable) > 0 {
                d = d * 3 / 2;
            }
            if self.s.status(dst, S::Mist) > 0 {
                d /= 2;
                self.add_status(dst, S::Mist, -1);
            }
            if mara && matches!(dst, UnitRef::Enemy(_)) {
                self.add_status(dst, S::Fear, 25);
            }
        }
        self.hurt(src, dst, d.max(0), attack)
    }

    /// Lands already-computed damage on block and HP.
    pub(crate) fn hurt(&mut self, src: Option<UnitRef>, dst: UnitRef, d: i32, attack: bool) -> bool {
        let mut killed = false;
        match dst {
            UnitRef::Hero(i) => {
                if self.s.heroes[i as usize].downed {
                    return false;
                }
                let guard = self.s.heroes[i as usize]
                    .guard_by
                    .filter(|g| *g != i && !self.s.heroes[*g as usize].downed);
                match guard {
                    Some(g) => {
                        let h = self.hero(i);
                        let blocked = d.min(h.block);
                        h.block -= blocked;
                        let (hp, block) = (h.hp, h.block);
                        self.ev.push(Event::Damage { src, dst, blocked, lost: 0, attack, hp, block });
                        let over = d - blocked;
                        if over > 0 {
                            self.ev.push(Event::Redirected { from: i, to: g, amount: over });
                            self.hero_take(src, g, over, attack);
                        }
                    }
                    None => self.hero_take(src, i, d, attack),
                }
            }
            UnitRef::Enemy(uid) => {
                let Some(idx) = self.enemy_idx(uid) else { return false };
                let e = &mut self.s.enemies[idx];
                if !e.alive {
                    return false;
                }
                let blocked = d.min(e.block);
                e.block -= blocked;
                let lost = (d - blocked).min(e.hp);
                e.hp -= lost;
                let dead = e.hp <= 0;
                let (hp, block) = (e.hp, e.block);
                self.ev.push(Event::Damage { src, dst, blocked, lost, attack, hp, block });
                if dead {
                    self.kill(idx, src);
                    killed = true;
                }
            }
        }
        if attack {
            if let Some(s) = src {
                let thorns = self.s.status(dst, S::Thorns);
                if thorns > 0 && s != dst {
                    self.hurt(None, s, thorns, false);
                }
            }
        }
        killed
    }

    fn hero_take(&mut self, src: Option<UnitRef>, i: u8, d: i32, attack: bool) {
        let h = self.hero(i);
        let blocked = d.min(h.block);
        h.block -= blocked;
        let lost = (d - blocked).min(h.hp);
        h.hp -= lost;
        let fell = h.hp <= 0;
        let mut valor = 0;
        if h.mechanic == Mechanic::Valor {
            h.valor_rem += blocked + lost;
            valor = h.valor_rem / 5;
            h.valor_rem %= 5;
        }
        let (hp, block) = (h.hp, h.block);
        self.ev.push(Event::Damage { src, dst: UnitRef::Hero(i), blocked, lost, attack, hp, block });
        if valor > 0 && !fell {
            self.add_status(UnitRef::Hero(i), S::Valor, valor);
        }
        if lost > 0 {
            for other in self.standing() {
                if other != i {
                    self.fire(other, Trigger::AllyHpLoss);
                }
            }
        }
        if fell {
            self.down(i);
        }
    }

    fn lose_hp(&mut self, u: UnitRef, n: i32) {
        if n <= 0 || !self.exists(u) {
            return;
        }
        match u {
            UnitRef::Hero(i) => {
                let h = self.hero(i);
                let lost = n.min(h.hp);
                h.hp -= lost;
                let fell = h.hp <= 0;
                let (hp, block) = (h.hp, h.block);
                self.ev.push(Event::Damage { src: None, dst: u, blocked: 0, lost, attack: false, hp, block });
                if lost > 0 {
                    for other in self.standing() {
                        if other != i {
                            self.fire(other, Trigger::AllyHpLoss);
                        }
                    }
                }
                if fell {
                    self.down(i);
                }
            }
            UnitRef::Enemy(uid) => {
                let idx = self.enemy_idx(uid).unwrap();
                let e = &mut self.s.enemies[idx];
                let lost = n.min(e.hp);
                e.hp -= lost;
                let dead = e.hp <= 0;
                let (hp, block) = (e.hp, e.block);
                self.ev.push(Event::Damage { src: None, dst: u, blocked: 0, lost, attack: false, hp, block });
                if dead {
                    self.kill(idx, None);
                }
            }
        }
    }

    fn down(&mut self, i: u8) {
        let has_curse = self.c.cards.contains_key(NAV_MARK);
        let h = self.hero(i);
        h.hp = 0;
        h.downed = true;
        h.ready = false;
        h.block = 0;
        h.guard_by = None;
        h.falls += 1;
        let hand = std::mem::take(&mut h.hand);
        h.discard.extend(hand);
        let cursed = h.falls > 1 && has_curse;
        self.ev.push(Event::Downed { player: i });
        if cursed {
            self.hero(i).new_curses += 1;
            self.add_card(i, NAV_MARK, Pile::Discard);
        }
        for h in &mut self.s.heroes {
            if h.guard_by == Some(i) {
                h.guard_by = None;
            }
        }
    }

    fn revive(&mut self, i: u8, pct: i32, by: Option<u8>) {
        let h = self.hero(i);
        if !h.downed {
            return;
        }
        h.downed = false;
        h.hp = (h.max_hp * pct / 100).max(1);
        let hp = h.hp;
        self.ev.push(Event::Raised { player: i, by, hp });
        for s in self.standing() {
            self.fire(s, Trigger::AllyRaised);
        }
    }

    fn kill(&mut self, idx: usize, src: Option<UnitRef>) {
        let e = &mut self.s.enemies[idx];
        e.alive = false;
        e.hp = 0;
        e.intent = None;
        let uid = e.uid;
        let def = e.def.clone();
        // a dead thief drops what it took
        for (slot, gold) in std::mem::take(&mut e.stolen) {
            self.s.heroes[slot as usize].gold_lost -= gold;
        }
        self.ev.push(Event::EnemyDied { enemy: uid });
        if let Some(d) = self.c.enemies.get(&def) {
            if !d.on_death.is_empty() {
                let fx = d.on_death.clone();
                let mut ctx = Ctx::passive(UnitRef::Enemy(uid));
                self.run(&fx, &mut ctx);
            }
        }
        if let Some(UnitRef::Hero(i)) = src {
            // Devour: a kill in Beast form feeds the werewolf
            if self.s.heroes[i as usize].form == Form::Beast {
                self.heal(UnitRef::Hero(i), 3);
            }
            self.fire(i, Trigger::Kill);
        }
    }

    // ---- cards and piles ----

    pub(crate) fn draw(&mut self, slot: u8, n: i32) {
        let mut count = 0;
        for _ in 0..n.max(0) {
            if self.s.heroes[slot as usize].hand.len() >= HAND_LIMIT {
                break;
            }
            if self.s.heroes[slot as usize].draw.is_empty() {
                if self.s.heroes[slot as usize].discard.is_empty() {
                    break;
                }
                let mut pile = std::mem::take(&mut self.s.heroes[slot as usize].discard);
                self.s.rng.shuffle(&mut pile);
                self.s.heroes[slot as usize].draw = pile;
                self.ev.push(Event::Shuffled { player: slot });
            }
            let h = self.hero(slot);
            let card = h.draw.pop().unwrap();
            h.hand.push(card);
            count += 1;
        }
        if count > 0 {
            self.ev.push(Event::Drawn { player: slot, count });
        }
    }

    pub(crate) fn add_card(&mut self, slot: u8, def: &str, pile: Pile) {
        self.s.next_uid += 1;
        let card = Card { uid: self.s.next_uid, def: def.to_string(), upgraded: false, free: false };
        let h = self.hero(slot);
        let pile = if pile == Pile::Hand && h.hand.len() >= HAND_LIMIT { Pile::Discard } else { pile };
        match pile {
            Pile::Hand => h.hand.push(card),
            Pile::Discard => h.discard.push(card),
            Pile::Draw => {
                // shuffled in at a random depth
                let at = self.s.rng.below(self.s.heroes[slot as usize].draw.len() as u32 + 1) as usize;
                self.s.heroes[slot as usize].draw.insert(at, card);
            }
        }
        self.ev.push(Event::CardAdded { player: slot, card: def.to_string(), pile });
    }

    fn play(&mut self, p: u8, uid: u32, target: Option<UnitRef>) -> Result<(), ActionError> {
        let h = &self.s.heroes[p as usize];
        let pos = h.hand.iter().position(|c| c.uid == uid).ok_or(ActionError::NoSuchCard)?;
        let card = h.hand[pos].clone();
        let def = self.c.cards.get(&card.def).ok_or(ActionError::NoSuchCard)?;
        let view = def.view(card.upgraded, h.form);
        if view.has(Keyword::Unplayable) {
            return Err(ActionError::Unplayable);
        }
        let carves = view.effects.iter().any(|e| matches!(e, Effect::Rune(..)));
        let free_rune = carves && h.flags.get("free_rune").copied().unwrap_or(0) > 0;
        let cost = if card.free || free_rune {
            0
        } else if view.x_cost {
            h.energy
        } else {
            view.cost
        };
        if h.energy < cost {
            return Err(ActionError::NotEnoughEnergy);
        }
        let target = match (view.target, target) {
            (CardTarget::None, _) => None,
            (CardTarget::Enemy, Some(t @ UnitRef::Enemy(_))) if self.exists(t) => Some(t),
            (CardTarget::Ally, Some(t @ UnitRef::Hero(j))) if j != p && self.exists(t) => Some(t),
            // alone, an ally card still plays for its base effect
            (CardTarget::Ally, None) if self.standing().len() == 1 => None,
            (CardTarget::AnyHero, Some(t @ UnitRef::Hero(_))) if self.exists(t) => Some(t),
            (CardTarget::Downed, Some(t @ UnitRef::Hero(j))) if self.s.hero(j).is_some_and(|a| a.downed) => Some(t),
            _ => return Err(ActionError::BadTarget),
        };

        let h = self.hero(p);
        h.hand.remove(pos);
        h.energy -= cost;
        if free_rune {
            *h.flags.entry("free_rune".into()).or_insert(1) -= 1;
        }
        self.ev.push(Event::CardPlayed { player: p, card: card.def.clone(), upgraded: card.upgraded, target });
        let mut ctx = Ctx {
            src: UnitRef::Hero(p),
            target,
            x: cost,
            consumed: 0,
            fatal: false,
            attack: view.kind == CardType::Attack,
            card: true,
            bled: false,
        };
        self.run(&view.effects, &mut ctx);

        let h = self.hero(p);
        if view.kind == CardType::Power {
            h.powers.extend(view.passives.iter().cloned());
        } else if view.has(Keyword::Exhaust) || card.free {
            h.exhaust.push(card.clone());
            self.ev.push(Event::Exhausted { player: p, card: card.def.clone() });
        } else {
            h.discard.push(card.clone());
        }
        self.s.log.push(Played { player: p, card: card.def.clone(), upgraded: card.upgraded, kind: view.kind });

        if self.s.heroes[p as usize].downed {
            return Ok(());
        }
        self.fire(p, Trigger::CardPlayed);
        if view.kind == CardType::Attack {
            self.fire(p, Trigger::AttackPlayed);
            let h = self.hero(p);
            if h.mechanic == Mechanic::Moon && h.form == Form::Human {
                h.moon += 1;
                if h.moon >= 5 {
                    self.shift(p);
                }
            }
        }
        if self.s.heroes[p as usize].mask == Some(MaskKind::Goat) {
            let mut others = self.standing();
            others.retain(|s| *s != p);
            let to = self.s.rng.pick(&others).copied().unwrap_or(p);
            self.gain_block(UnitRef::Hero(to), 2);
        }
        Ok(())
    }

    fn raise(&mut self, p: u8, ally: u8, discard: u32) -> Result<(), ActionError> {
        let cost = self.s.raise_cost;
        let h = &self.s.heroes[p as usize];
        if h.energy < cost {
            return Err(ActionError::NotEnoughEnergy);
        }
        let pos = h.hand.iter().position(|c| c.uid == discard).ok_or(ActionError::NoSuchCard)?;
        if ally == p || !self.s.hero(ally).is_some_and(|a| a.downed) {
            return Err(ActionError::NotDowned);
        }
        let h = self.hero(p);
        h.energy -= cost;
        let card = h.hand.remove(pos);
        h.discard.push(card);
        self.revive(ally, 25, Some(p));
        Ok(())
    }

    fn shift(&mut self, p: u8) {
        if self.s.status(UnitRef::Hero(p), S::Chained) > 0 {
            return;
        }
        let h = self.hero(p);
        h.form = if h.form == Form::Human { Form::Beast } else { Form::Human };
        h.moon = 0;
        let form = h.form;
        self.ev.push(Event::Shifted { player: p, form });
        self.fire(p, Trigger::Shifted);
    }

    // ---- passives ----

    pub(crate) fn fire(&mut self, slot: u8, on: Trigger) {
        if self.depth >= 6 {
            return;
        }
        let h = &self.s.heroes[slot as usize];
        if h.downed {
            return;
        }
        let mut lists: Vec<Vec<Effect>> = Vec::new();
        for r in &h.relics {
            if let Some(def) = self.c.relics.get(r) {
                lists.extend(def.passives.iter().filter(|p| p.on == on).map(|p| p.effects.clone()));
            }
        }
        lists.extend(h.powers.iter().filter(|p| p.on == on).map(|p| p.effects.clone()));
        self.depth += 1;
        for fx in lists {
            let mut ctx = Ctx::passive(UnitRef::Hero(slot));
            self.run(&fx, &mut ctx);
        }
        self.depth -= 1;
    }

    fn fire_runes(&mut self, slot: u8) {
        // the thunder-oak staff: the first rune its bearer carved fires twice
        let twice = self.s.heroes[slot as usize].flags.contains_key("first_rune_twice");
        for (i, rune) in self.s.heroes[slot as usize].runes.clone().into_iter().enumerate() {
            self.fire_rune(slot, rune);
            if i == 0 && twice {
                self.fire_rune(slot, rune);
            }
        }
    }

    pub(crate) fn fire_rune(&mut self, slot: u8, rune: RuneKind) {
        let me = UnitRef::Hero(slot);
        {
            if self.s.heroes[slot as usize].downed {
                return;
            }
            self.ev.push(Event::RuneFired { player: slot, rune });
            match rune {
                RuneKind::Thunder => {
                    let foes = self.living();
                    // Storm: thunder strikes every enemy instead of one
                    if self.s.heroes.iter().any(|h| !h.downed && h.flags.contains_key("storm")) {
                        for t in foes {
                            self.hurt(Some(me), UnitRef::Enemy(t), 6, false);
                        }
                    } else if let Some(t) = self.s.rng.pick(&foes).copied() {
                        self.hurt(Some(me), UnitRef::Enemy(t), 6, false);
                    }
                }
                RuneKind::Frost => self.gain_block(me, 4),
                RuneKind::Mist => self.add_status(me, S::Mist, 1),
                RuneKind::Flame => {
                    for t in self.living() {
                        self.hurt(Some(me), UnitRef::Enemy(t), 3, false);
                    }
                }
                RuneKind::Zhiva => self.heal(me, 2),
            }
        }
    }

    // ---- effect interpreter ----

    fn targets(&mut self, ctx: &Ctx, t: Tgt) -> Vec<UnitRef> {
        let hero_side = matches!(ctx.src, UnitRef::Hero(_));
        let heroes = |me: &Self| me.standing().into_iter().map(UnitRef::Hero).collect::<Vec<_>>();
        let enemies = |me: &Self| me.living().into_iter().map(UnitRef::Enemy).collect::<Vec<_>>();
        match t {
            Tgt::Me => vec![ctx.src],
            Tgt::Target => ctx.target.into_iter().collect(),
            Tgt::AllFoes => {
                if hero_side {
                    enemies(self)
                } else {
                    heroes(self)
                }
            }
            Tgt::RandomFoe => {
                let all = if hero_side { enemies(self) } else { heroes(self) };
                self.s.rng.pick(&all).copied().into_iter().collect()
            }
            Tgt::AllFriends => {
                if hero_side {
                    heroes(self)
                } else {
                    enemies(self)
                }
            }
            Tgt::OtherFriends => {
                let mut all = if hero_side { heroes(self) } else { enemies(self) };
                all.retain(|u| *u != ctx.src);
                all
            }
        }
    }

    fn amount(&mut self, ctx: &Ctx, a: &Amount) -> i32 {
        match a {
            Amount::N(n) => *n,
            Amount::X(m) => ctx.x * m,
            Amount::Stat(st, who, m) => {
                let us = self.targets(ctx, *who);
                us.iter().map(|u| self.s.status(*u, *st)).sum::<i32>() * m
            }
            Amount::Consumed(m) => ctx.consumed * m,
            Amount::PlayedThisRound(m) => self.s.log.len() as i32 * m,
            Amount::Curses(who, m) => {
                let us = self.targets(ctx, *who);
                us.iter().map(|u| self.s.status(*u, S::Rot) + self.s.status(*u, S::Hex) + self.s.status(*u, S::Weak)).sum::<i32>() * m
            }
            Amount::Runes(kind, m) => {
                let n: usize = self.s.heroes.iter().map(|h| h.runes.iter().filter(|r| *r == kind).count()).sum();
                n as i32 * m
            }
        }
    }

    fn cond(&mut self, ctx: &Ctx, c: &Cond) -> bool {
        let src_hero = match ctx.src {
            UnitRef::Hero(i) => self.s.hero(i),
            _ => None,
        };
        match c {
            Cond::Has(st, who, min) => {
                let us = self.targets(ctx, *who);
                !us.is_empty() && us.iter().all(|u| self.s.status(*u, *st) >= *min)
            }
            Cond::Link(kind) => self.s.log.iter().any(|p| UnitRef::Hero(p.player) != ctx.src && p.kind == *kind),
            Cond::Fatal => ctx.fatal,
            Cond::Form(f) => src_hero.is_some_and(|h| h.form == *f),
            Cond::Mask(m) => src_hero.is_some_and(|h| h.mask == Some(*m)),
            Cond::Coop => self.s.is_coop(),
            Cond::Not(inner) => !self.cond(ctx, inner),
        }
    }

    fn bleed(&mut self, ctx: &mut Ctx) {
        if ctx.attack && !ctx.bled {
            ctx.bled = true;
            let b = self.s.status(ctx.src, S::Bleed);
            if b > 0 {
                self.hurt(None, ctx.src, b, false);
            }
        }
    }

    pub(crate) fn run(&mut self, effects: &[Effect], ctx: &mut Ctx) {
        for e in effects {
            match e {
                Effect::Damage(a, t) | Effect::Hits(a, _, t) => {
                    let times = if let Effect::Hits(_, k, _) = e { *k } else { 1 };
                    self.bleed(ctx);
                    let n = self.amount(ctx, a);
                    ctx.fatal = false;
                    for _ in 0..times {
                        for u in self.targets(ctx, *t) {
                            if self.strike(Some(ctx.src), u, n, ctx.attack) {
                                ctx.fatal = true;
                            }
                        }
                    }
                }
                Effect::Block(a, t) | Effect::Guard(a, t) => {
                    let guard = matches!(e, Effect::Guard(..));
                    let mut n = self.amount(ctx, a);
                    if ctx.card {
                        n += self.s.status(ctx.src, S::Fortitude);
                    }
                    let beast = matches!(ctx.src, UnitRef::Hero(i) if self.s.heroes[i as usize].form == Form::Beast);
                    for u in self.targets(ctx, *t) {
                        // the Beast can't shield itself with its own cards
                        if ctx.card && beast && u == ctx.src {
                            continue;
                        }
                        self.gain_block(u, n);
                        if let (true, UnitRef::Hero(to), UnitRef::Hero(by)) = (guard, u, ctx.src) {
                            if to != by {
                                self.hero(to).guard_by = Some(by);
                            }
                        }
                    }
                }
                Effect::Apply(st, a, t) => {
                    let n = self.amount(ctx, a);
                    for u in self.targets(ctx, *t) {
                        self.add_status(u, *st, n);
                    }
                }
                Effect::Remove(st, t) => {
                    for u in self.targets(ctx, *t) {
                        self.clear_status(u, *st);
                    }
                }
                Effect::Consume(st, t) => {
                    ctx.consumed = 0;
                    for u in self.targets(ctx, *t) {
                        ctx.consumed += self.clear_status(u, *st);
                    }
                }
                Effect::Cleanse(t) => {
                    for u in self.targets(ctx, *t) {
                        let all: Vec<StatusKind> = match u {
                            UnitRef::Hero(i) => self.s.heroes[i as usize].statuses.keys().copied().collect(),
                            UnitRef::Enemy(i) => self.s.enemy(i).map(|e| e.statuses.keys().copied().collect()).unwrap_or_default(),
                        };
                        for st in all.into_iter().filter(|s| s.debuff()) {
                            self.clear_status(u, st);
                        }
                    }
                }
                Effect::Draw(a) => {
                    let n = self.amount(ctx, a);
                    if let UnitRef::Hero(i) = ctx.src {
                        self.draw(i, n);
                    }
                }
                Effect::Energy(a, t) => {
                    let n = self.amount(ctx, a);
                    for u in self.targets(ctx, *t) {
                        if let UnitRef::Hero(i) = u {
                            let h = self.hero(i);
                            let delta = n.max(-h.energy);
                            h.energy += delta;
                            self.ev.push(Event::Energy { player: i, delta });
                        }
                    }
                }
                Effect::Heal(a, t) => {
                    let n = self.amount(ctx, a);
                    for u in self.targets(ctx, *t) {
                        self.heal(u, n);
                    }
                }
                Effect::LoseHp(a, t) => {
                    let n = self.amount(ctx, a);
                    for u in self.targets(ctx, *t) {
                        self.lose_hp(u, n);
                    }
                }
                Effect::Revive(pct, t) => {
                    // downed heroes are invisible to `targets`, so resolve by hand
                    let who: Vec<u8> = match t {
                        Tgt::Target => match ctx.target {
                            Some(UnitRef::Hero(i)) => vec![i],
                            _ => vec![],
                        },
                        _ => self.s.heroes.iter().filter(|h| h.downed).map(|h| h.slot).collect(),
                    };
                    let by = if let UnitRef::Hero(i) = ctx.src { Some(i) } else { None };
                    for i in who {
                        self.revive(i, *pct, by);
                    }
                }
                Effect::AddCard { card, pile, to, count } => {
                    for u in self.targets(ctx, *to) {
                        if let UnitRef::Hero(i) = u {
                            for _ in 0..*count {
                                self.add_card(i, card, *pile);
                            }
                        }
                    }
                }
                Effect::Taunt => {
                    if let UnitRef::Hero(i) = ctx.src {
                        for e in self.s.enemies.iter_mut().filter(|e| e.alive) {
                            if let Some(intent) = &mut e.intent {
                                if intent.target.is_some() {
                                    intent.target = Some(i);
                                }
                            }
                        }
                        self.ev.push(Event::Taunt { player: i });
                    }
                }
                Effect::If(c, then, otherwise) => {
                    let branch = if self.cond(ctx, c) { then } else { otherwise };
                    self.run(branch, ctx);
                }
                Effect::Repeat(a, body) => {
                    for _ in 0..self.amount(ctx, a).clamp(0, 50) {
                        self.run(body, ctx);
                    }
                }
                Effect::Rune(kind, t) => {
                    for u in self.targets(ctx, *t) {
                        if let UnitRef::Hero(i) = u {
                            let cap = if u == ctx.src { 5 } else { 3 };
                            let h = self.hero(i);
                            if h.runes.len() < cap {
                                h.runes.push(*kind);
                                self.ev.push(Event::RuneCarved { player: i, rune: *kind });
                            }
                        }
                    }
                }
                Effect::Invoke => {
                    for i in self.standing() {
                        self.fire_runes(i);
                    }
                }
                Effect::Shift => {
                    if let UnitRef::Hero(i) = ctx.src {
                        self.shift(i);
                    }
                }
                Effect::Mask(m) => {
                    if let UnitRef::Hero(i) = ctx.src {
                        if self.s.heroes[i as usize].mask != Some(*m) {
                            self.hero(i).mask = Some(*m);
                            self.ev.push(Event::MaskChanged { player: i, mask: *m });
                            self.fire(i, Trigger::MaskChanged);
                        }
                    }
                }
                Effect::Summon(id, n) => {
                    for _ in 0..*n {
                        if let Some(uid) = self.spawn(id) {
                            self.ev.push(Event::Summoned { enemy: uid, def: id.clone() });
                        }
                    }
                }
                Effect::StealGold(n) => {
                    if let (UnitRef::Enemy(uid), Some(UnitRef::Hero(i))) = (ctx.src, ctx.target) {
                        self.hero(i).gold_lost += n;
                        if let Some(idx) = self.enemy_idx(uid) {
                            *self.s.enemies[idx].stolen.entry(i).or_insert(0) += n;
                        }
                        self.ev.push(Event::GoldStolen { player: i, amount: *n });
                    }
                }
                Effect::Flee => {
                    if let UnitRef::Enemy(uid) = ctx.src {
                        if let Some(idx) = self.enemy_idx(uid) {
                            let e = &mut self.s.enemies[idx];
                            e.alive = false;
                            e.fled = true;
                            e.intent = None;
                            self.ev.push(Event::EnemyFled { enemy: uid });
                        }
                    }
                }
                Effect::ConsumeCurses(t) => {
                    ctx.consumed = 0;
                    for u in self.targets(ctx, *t) {
                        for st in [S::Rot, S::Hex, S::Weak] {
                            ctx.consumed += self.clear_status(u, st);
                        }
                    }
                }
                Effect::Flag(name, n) => {
                    if let UnitRef::Hero(i) = ctx.src {
                        *self.hero(i).flags.entry(name.clone()).or_insert(0) += n;
                    }
                }
                Effect::Once(name, body) | Effect::OncePerTurn(name, body) => {
                    if let UnitRef::Hero(i) = ctx.src {
                        // per-round keys live under `turn_`, which the round start wipes
                        let key = if matches!(e, Effect::OncePerTurn(..)) { format!("turn_{name}") } else { format!("once_{name}") };
                        if !self.s.heroes[i as usize].flags.contains_key(&key) {
                            self.hero(i).flags.insert(key, 1);
                            self.run(body, ctx);
                        }
                    }
                }
                Effect::MaxHp(a, t) => {
                    let n = self.amount(ctx, a);
                    for u in self.targets(ctx, *t) {
                        if let UnitRef::Hero(i) = u {
                            let h = self.hero(i);
                            h.max_hp = (h.max_hp + n).max(1);
                            h.hp = (h.hp + n.max(0)).min(h.max_hp);
                        }
                    }
                }
                Effect::Echo(t) => {
                    let UnitRef::Hero(me) = ctx.src else { continue };
                    // an ally's last card; alone, your own
                    let last = self
                        .s
                        .log
                        .iter()
                        .rev()
                        .find(|p| p.player != me)
                        .or_else(|| self.s.log.iter().rev().find(|p| p.player == me))
                        .map(|p| (p.card.clone(), p.upgraded));
                    let Some((def, upgraded)) = last else { continue };
                    for u in self.targets(ctx, *t) {
                        if let UnitRef::Hero(i) = u {
                            self.s.next_uid += 1;
                            let card = Card { uid: self.s.next_uid, def: def.clone(), upgraded, free: true };
                            let h = self.hero(i);
                            if h.hand.len() < HAND_LIMIT {
                                h.hand.push(card);
                                self.ev.push(Event::CardAdded { player: i, card: def.clone(), pile: Pile::Hand });
                            }
                        }
                    }
                }
                Effect::Herb(herb) => {
                    if let UnitRef::Hero(i) = ctx.src {
                        let h = self.hero(i);
                        let room = if h.flags.contains_key("big_cauldron") { 4 } else { 3 };
                        if h.cauldron.len() < room {
                            h.cauldron.push(herb.clone());
                            self.ev.push(Event::HerbAdded { player: i, herb: herb.clone() });
                        }
                    }
                }
                Effect::Brew => self.brew(ctx),
                Effect::Provoke => {
                    if let (UnitRef::Hero(me), Some(UnitRef::Enemy(uid))) = (ctx.src, ctx.target) {
                        if let Some(idx) = self.enemy_idx(uid) {
                            if let Some(intent) = &mut self.s.enemies[idx].intent {
                                if intent.target.is_some() {
                                    intent.target = Some(me);
                                    let mv = intent.mv;
                                    self.ev.push(Event::Intent { enemy: uid, mv, target: Some(me) });
                                }
                            }
                        }
                    }
                }
                Effect::Hook(name) => crate::hooks::run(self, ctx, name),
            }
        }
    }

    /// Resolves the cauldron: the first recipe whose herbs match is brewed.
    fn brew(&mut self, ctx: &mut Ctx) {
        let UnitRef::Hero(me) = ctx.src else { return };
        let herbs = std::mem::take(&mut self.hero(me).cauldron);
        if herbs.is_empty() {
            return;
        }
        // fern flower stands in for any herb and doubles the brew
        let ferns = herbs.iter().filter(|h| *h == "fern").count();
        let plain: Vec<&Id> = herbs.iter().filter(|h| *h != "fern").collect();
        let found = self.c.recipes.iter().find(|r| {
            if r.herbs.len() != herbs.len() {
                return false;
            }
            let mut have = plain.clone();
            let mut wild = ferns;
            let mut stars = 0;
            for need in &r.herbs {
                if need == "*" {
                    stars += 1;
                } else if let Some(i) = have.iter().position(|h| *h == need) {
                    have.remove(i);
                } else if wild > 0 {
                    wild -= 1;
                } else {
                    return false;
                }
            }
            have.len() + wild == stars
        });
        let Some(recipe) = found else { return };
        let mut effects = recipe.effects.clone();
        let id = recipe.id.clone();
        // "Brew for all": single targets widen to the whole side
        let wide = self.s.heroes[me as usize].flags.get("brew_all").copied().unwrap_or(0) > 0;
        if wide {
            *self.hero(me).flags.entry("brew_all".into()).or_insert(1) -= 1;
            fn widen(list: &mut [Effect]) {
                let w = |t: &mut Tgt| {
                    *t = match *t {
                        Tgt::Target | Tgt::RandomFoe => Tgt::AllFoes,
                        Tgt::Me => Tgt::AllFriends,
                        other => other,
                    }
                };
                for e in list {
                    match e {
                        Effect::Damage(_, t) | Effect::Hits(_, _, t) | Effect::Block(_, t) | Effect::Apply(_, _, t) | Effect::Heal(_, t) => w(t),
                        Effect::If(_, a, b) => {
                            widen(a);
                            widen(b);
                        }
                        _ => {}
                    }
                }
            }
            widen(&mut effects);
        }
        self.ev.push(Event::Brewed { player: me, recipe: id });
        // brews are potions, not weapon swings
        let was = ctx.attack;
        ctx.attack = false;
        for _ in 0..if ferns > 0 { 2 } else { 1 } {
            self.run(&effects, ctx);
        }
        ctx.attack = was;
    }

    // ---- round flow ----

    fn start_round(&mut self) {
        self.s.round += 1;
        self.s.log.clear();
        self.ev.push(Event::RoundStart(self.s.round));

        let standing = self.standing();
        for idx in 0..self.s.enemies.len() {
            if !self.s.enemies[idx].alive {
                continue;
            }
            let Some(d) = self.c.enemies.get(&self.s.enemies[idx].def) else { continue };
            let step = self.s.enemies[idx].step;
            let mv = if step < d.opening.len() {
                d.opening[step]
            } else {
                d.pattern[(step - d.opening.len()) % d.pattern.len()]
            };
            let target = if d.moves[mv].targeted { self.s.rng.pick(&standing).copied() } else { None };
            let e = &mut self.s.enemies[idx];
            e.intent = Some(Intent { mv, target });
            let enemy = e.uid;
            self.ev.push(Event::Intent { enemy, mv, target });
        }

        for slot in standing {
            let round = self.s.round;
            let h = self.hero(slot);
            if round > 1 {
                h.block = 0;
            }
            h.energy = BASE_ENERGY;
            h.ready = false;
            h.flags.retain(|k, _| !k.starts_with("turn_"));
            let hungry = h.form == Form::Beast && !h.flags.contains_key("no_hunger");
            if hungry {
                self.lose_hp(UnitRef::Hero(slot), 2);
            }
            if self.s.heroes[slot as usize].downed {
                continue;
            }
            self.draw(slot, BASE_DRAW);
            self.fire(slot, Trigger::TurnStart);
        }
        self.check_end();
    }

    fn maybe_close(&mut self) {
        if self.s.phase == Phase::Heroes && self.s.standing_heroes().all(|h| h.ready) {
            self.end_round();
        }
    }

    fn end_round(&mut self) {
        // end of the heroes' turn, in slot order
        for slot in self.standing() {
            self.fire(slot, Trigger::TurnEnd);
            self.fire_runes(slot);
            if self.s.heroes[slot as usize].downed {
                continue;
            }
            let hand = std::mem::take(&mut self.hero(slot).hand);
            let mut keep = Vec::new();
            for card in hand {
                let Some(def) = self.c.cards.get(&card.def) else { continue };
                let view = def.view(card.upgraded, self.s.heroes[slot as usize].form);
                if !def.in_hand_end.is_empty() {
                    let fx = def.in_hand_end.clone();
                    let mut ctx = Ctx::passive(UnitRef::Hero(slot));
                    self.run(&fx, &mut ctx);
                }
                let h = self.hero(slot);
                if view.has(Keyword::Ethereal) {
                    self.ev.push(Event::Exhausted { player: slot, card: card.def.clone() });
                    self.hero(slot).exhaust.push(card);
                } else if view.has(Keyword::Retain) && !h.downed {
                    keep.push(card);
                } else {
                    h.discard.push(card);
                }
            }
            self.hero(slot).hand = keep;
            self.rot_tick(UnitRef::Hero(slot));
        }
        self.check_end();
        if self.s.phase != Phase::Heroes {
            return;
        }

        self.ev.push(Event::EnemyPhase);
        let order: Vec<u32> = self.s.enemies.iter().map(|e| e.uid).collect();
        for uid in order {
            let Some(idx) = self.enemy_idx(uid) else { continue };
            if !self.s.enemies[idx].alive {
                continue;
            }
            self.s.enemies[idx].block = 0;
            let Some(intent) = self.s.enemies[idx].intent.clone() else { continue };
            self.s.enemies[idx].step += 1;
            let me = UnitRef::Enemy(uid);
            let Some(mv) = self.c.enemies.get(&self.s.enemies[idx].def).and_then(|d| d.moves.get(intent.mv)).cloned() else {
                continue;
            };
            let attacks = matches!(mv.intent, IntentKind::Attack | IntentKind::AttackAll);
            let fear = self.s.status(me, S::Fear);
            if self.s.status(me, S::Stun) > 0 {
                self.add_status(me, S::Stun, -1);
                self.ev.push(Event::EnemySkipped { enemy: uid, fear: false });
            } else if attacks && fear > 0 && self.s.rng.chance(fear) {
                self.clear_status(me, S::Fear);
                self.ev.push(Event::EnemySkipped { enemy: uid, fear: true });
            } else {
                let mut target = intent.target;
                if target.is_some_and(|t| self.s.heroes[t as usize].downed) {
                    let standing = self.standing();
                    target = self.s.rng.pick(&standing).copied();
                }
                self.ev.push(Event::EnemyMove { enemy: uid, mv: intent.mv });
                // Switcheroo: the blow meant for a hero lands on another enemy
                let mut aim = target.map(UnitRef::Hero);
                if self.s.enemies[idx].flags.remove("misdirect").is_some() && aim.is_some() {
                    let others: Vec<u32> = self.living().into_iter().filter(|u| *u != uid).collect();
                    aim = self.s.rng.pick(&others).copied().map(UnitRef::Enemy);
                    if aim.is_none() {
                        self.rot_tick(me);
                        continue;
                    }
                }
                let mut ctx = Ctx {
                    src: me,
                    target: aim,
                    x: 0,
                    consumed: 0,
                    fatal: false,
                    attack: true,
                    card: false,
                    bled: false,
                };
                self.run(&mv.effects, &mut ctx);
            }
            self.rot_tick(me);
            if self.s.standing_heroes().next().is_none() {
                break;
            }
        }

        let units: Vec<UnitRef> = (0..self.s.heroes.len() as u8)
            .map(UnitRef::Hero)
            .chain(self.s.enemies.iter().map(|e| UnitRef::Enemy(e.uid)))
            .collect();
        for u in units {
            let all: Vec<(StatusKind, i32)> = match u {
                UnitRef::Hero(i) => self.s.heroes[i as usize].statuses.iter().map(|(k, v)| (*k, *v)).collect(),
                UnitRef::Enemy(i) => self.s.enemy(i).map(|e| e.statuses.iter().map(|(k, v)| (*k, *v)).collect()).unwrap_or_default(),
            };
            for (st, n) in all {
                if st.timed() {
                    self.add_status(u, st, -1);
                } else if st.round_only() {
                    self.add_status(u, st, -n);
                }
            }
        }
        for h in &mut self.s.heroes {
            h.guard_by = None;
        }
        self.check_end();
        if self.s.phase == Phase::Heroes {
            self.start_round();
        }
    }

    /// Rot: X damage, then one stack fewer.
    fn rot_tick(&mut self, u: UnitRef) {
        let rot = self.s.status(u, S::Rot);
        if rot > 0 && self.exists(u) {
            // Pestilence: Rot on enemies bites twice and does not fade
            let mor = matches!(u, UnitRef::Enemy(_)) && self.s.heroes.iter().any(|h| !h.downed && h.flags.contains_key("mor"));
            self.hurt(None, u, rot, false);
            if mor {
                self.hurt(None, u, rot, false);
            } else {
                self.add_status(u, S::Rot, -1);
            }
        }
    }

    fn check_end(&mut self) {
        if self.s.phase != Phase::Heroes {
            return;
        }
        if self.s.standing_heroes().next().is_none() {
            self.s.phase = Phase::Lost;
            self.ev.push(Event::Ended { won: false });
        } else if self.s.living_enemies().next().is_none() {
            self.s.phase = Phase::Won;
            let has_curse = self.c.cards.contains_key(NAV_MARK);
            for h in &mut self.s.heroes {
                if h.downed {
                    // still on the edge when the fight ends: up at 1 HP, marked
                    h.downed = false;
                    h.hp = 1;
                    if has_curse {
                        h.new_curses += 1;
                    }
                }
            }
            self.ev.push(Event::Ended { won: true });
        }
    }
}
