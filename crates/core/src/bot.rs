//! Autopilot for disconnected players and the balance simulator:
//! block when something is swinging at you, otherwise hit.

use crate::data::*;
use crate::state::*;

/// Damage one enemy's shown intent will deal per hit, and how many hits, before the target's block.
pub fn intent_damage(c: &Content, s: &Combat, e: &Enemy) -> Option<(i32, i32)> {
    let intent = e.intent.as_ref()?;
    let mv = c.enemies.get(&e.def)?.moves.get(intent.mv)?;
    let me = UnitRef::Enemy(e.uid);
    mv.effects.iter().find_map(|fx| {
        let (a, hits) = match fx {
            Effect::Damage(a, _) => (a, 1),
            Effect::Hits(a, k, _) => (a, *k),
            _ => return None,
        };
        let Amount::N(base) = a else { return None };
        let mut d = base + s.status(me, StatusKind::Strength) + s.status(me, StatusKind::TempStrength);
        if s.status(me, StatusKind::Weak) > 0 {
            d = d * 3 / 4;
        }
        Some((d.max(0), hits))
    })
}

/// Total damage aimed at a hero this round.
pub fn incoming(c: &Content, s: &Combat, slot: u8) -> i32 {
    s.living_enemies()
        .filter_map(|e| {
            let intent = e.intent.as_ref()?;
            let mv = c.enemies.get(&e.def)?.moves.get(intent.mv)?;
            let hits_me = intent.target == Some(slot) || (intent.target.is_none() && mv.intent == IntentKind::AttackAll);
            if !hits_me {
                return None;
            }
            intent_damage(c, s, e).map(|(d, k)| d * k)
        })
        .sum()
}

fn blocks_self(view: &CardView) -> bool {
    view.effects.iter().any(|e| matches!(e, Effect::Block(_, Tgt::Me | Tgt::AllFriends)))
}

pub fn choose(c: &Content, s: &Combat, slot: u8) -> CombatAction {
    let Some(h) = s.hero(slot) else { return CombatAction::Ready(true) };
    let playable: Vec<(&Card, CardView)> = h
        .hand
        .iter()
        .filter_map(|card| {
            let v = c.cards.get(&card.def)?.view(card.upgraded, h.form);
            let cost = if v.x_cost { 1 } else { v.cost };
            // paying in blood is for players who can count; the autopilot never does
            let bleeds = v.effects.iter().any(|e| matches!(e, Effect::LoseHp(_, Tgt::Me)));
            let affordable = !bleeds;
            (!v.has(Keyword::Unplayable) && cost <= h.energy && affordable && !matches!(v.kind, CardType::Curse | CardType::Status)).then_some((card, v))
        })
        .collect();

    // a fallen friend comes first: the run ends when everyone is down
    if let Some(ally) = s.heroes.iter().find(|a| a.downed) {
        if let Some((card, _)) = playable.iter().find(|(_, v)| v.target == CardTarget::Downed) {
            return CombatAction::Play { card: card.uid, target: Some(UnitRef::Hero(ally.slot)) };
        }
        if h.energy >= s.raise_cost {
            if let Some(card) = h.hand.first() {
                return CombatAction::Raise { ally: ally.slot, discard: card.uid };
            }
        }
    }

    let weakest = s.living_enemies().min_by_key(|e| e.hp + e.block).map(|e| UnitRef::Enemy(e.uid));
    let other = s.standing_heroes().find(|a| a.slot != slot).map(|a| UnitRef::Hero(a.slot));
    let target_for = |v: &CardView| -> Option<Option<UnitRef>> {
        match v.target {
            CardTarget::None => Some(None),
            CardTarget::Enemy => weakest.map(Some),
            CardTarget::Ally => Some(other),
            CardTarget::AnyHero => Some(Some(other.unwrap_or(UnitRef::Hero(slot)))),
            CardTarget::Downed => None,
        }
    };

    let threatened = incoming(c, s, slot) > h.block;
    let pick = |want: &dyn Fn(&CardView) -> bool| {
        playable.iter().filter(|(_, v)| want(v)).find_map(|(card, v)| {
            target_for(v).map(|target| CombatAction::Play { card: card.uid, target })
        })
    };
    if threatened {
        if let Some(a) = pick(&blocks_self) {
            return a;
        }
    }
    if let Some(a) = pick(&|v| v.kind == CardType::Power) {
        return a;
    }
    if let Some(a) = pick(&|v| v.kind == CardType::Attack) {
        return a;
    }
    if let Some(a) = pick(&|v| !blocks_self(v) || threatened) {
        return a;
    }
    if let Some(a) = pick(&|_| true) {
        return a;
    }
    CombatAction::Ready(true)
}
