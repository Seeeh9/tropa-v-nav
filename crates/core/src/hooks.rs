//! Named one-off mechanics that don't fit the effect primitives.

use crate::combat::{Ctx, Exec};
use crate::data::*;
use crate::state::*;

pub(crate) fn run(ex: &mut Exec, ctx: &mut Ctx, name: &str) {
    match name {
        // Vorozheya's "Transfer": every debuff on the heroes moves onto the targeted enemy
        "transfer" => {
            let Some(target) = ctx.target else { return };
            let heroes: Vec<u8> = ex.s.standing_heroes().map(|h| h.slot).collect();
            for slot in heroes {
                let debuffs: Vec<(StatusKind, i32)> =
                    ex.s.heroes[slot as usize].statuses.iter().filter(|(k, _)| k.debuff()).map(|(k, v)| (*k, *v)).collect();
                for (st, n) in debuffs {
                    ex.add_status(UnitRef::Hero(slot), st, -n);
                    ex.add_status(target, st, n);
                }
            }
        }
        // Volkhv's "Wheel of Kolyada": X times, one random rune on every hero fires
        "kolyada" => {
            for _ in 0..ctx.x {
                let heroes: Vec<u8> = ex.s.standing_heroes().map(|h| h.slot).collect();
                for slot in heroes {
                    let runes = ex.s.heroes[slot as usize].runes.clone();
                    if let Some(rune) = ex.s.rng.pick(&runes).copied() {
                        ex.fire_rune(slot, rune);
                    }
                }
            }
        }
        // Skomorokh's "Fairground": enemies swap who they are aiming at
        "balagan" => {
            let idx: Vec<usize> =
                ex.s.enemies.iter().enumerate().filter(|(_, e)| e.alive && e.intent.as_ref().is_some_and(|i| i.target.is_some())).map(|(i, _)| i).collect();
            let mut targets: Vec<Option<u8>> = idx.iter().map(|i| ex.s.enemies[*i].intent.as_ref().and_then(|x| x.target)).collect();
            ex.s.rng.shuffle(&mut targets);
            for (i, t) in idx.into_iter().zip(targets) {
                let e = &mut ex.s.enemies[i];
                if let Some(intent) = &mut e.intent {
                    intent.target = t;
                    let (enemy, mv) = (e.uid, intent.mv);
                    ex.ev.push(Event::Intent { enemy, mv, target: t });
                }
            }
        }
        // Skomorokh's "Switcheroo": the targeted enemy's next blow hits another enemy
        "misdirect" => {
            if let Some(UnitRef::Enemy(uid)) = ctx.target {
                if let Some(e) = ex.s.enemies.iter_mut().find(|e| e.uid == uid) {
                    e.flags.insert("misdirect".into(), 1);
                }
            }
        }
        // Znakharka's "Herbalist": a random common herb comes to hand
        "random_herb" => {
            if let UnitRef::Hero(me) = ctx.src {
                let herbs = ["herb_wormwood", "herb_sleep", "herb_weep"];
                let pick = herbs[ex.s.rng.below(herbs.len() as u32) as usize];
                if ex.c.cards.contains_key(pick) {
                    ex.add_card(me, pick, Pile::Hand);
                }
            }
        }
        _ => debug_assert!(false, "unknown hook `{name}`"),
    }
}
