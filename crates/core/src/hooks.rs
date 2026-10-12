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
        // Koschei, every turn: he knits back together while deathless; once the needle is
        // broken he drains the weakest hero and his last breath counts down to the party's end
        "koschei_turn" => {
            let UnitRef::Enemy(uid) = ctx.src else { return };
            let Some(idx) = ex.s.enemies.iter().position(|e| e.uid == uid) else { return };
            if ex.s.enemies[idx].flags.contains_key("immortal") {
                let heal = ex.s.enemies[idx].max_hp / 10;
                ex.heal(ctx.src, heal);
                // his patience wears thin: every second turn he hits harder, so a party
                // that only shelters behind its shields cannot wait him out forever
                let wrath = ex.s.enemies[idx].flags.get("wrath").copied().unwrap_or(0) + 1;
                ex.s.enemies[idx].flags.insert("wrath".into(), wrath);
                if wrath % 2 == 0 {
                    ex.add_status(ctx.src, StatusKind::Strength, 1);
                }
                return;
            }
            let weakest = ex.s.standing_heroes().min_by_key(|h| h.hp).map(|h| h.slot);
            if let Some(slot) = weakest {
                ex.hurt(Some(ctx.src), UnitRef::Hero(slot), 10, false);
                ex.heal(ctx.src, 10);
            }
            let left = ex.s.enemies[idx].flags.get("last_breath").copied().unwrap_or(0) - 1;
            ex.s.enemies[idx].flags.insert("last_breath".into(), left);
            if left <= 0 {
                let all: Vec<u8> = ex.s.standing_heroes().map(|h| h.slot).collect();
                for slot in all {
                    let hp = ex.s.heroes[slot as usize].hp + ex.s.heroes[slot as usize].block;
                    ex.hurt(Some(ctx.src), UnitRef::Hero(slot), hp, false);
                }
            }
        }
        // the egg is cracked: the needle falls into a random hero's hand and Koschei rises
        "drop_needle" => {
            let heroes: Vec<u8> = ex.s.standing_heroes().map(|h| h.slot).collect();
            if let Some(slot) = ex.s.rng.pick(&heroes).copied() {
                ex.add_card(slot, "koschei_needle", Pile::Hand);
            }
            for e in ex.s.enemies.iter_mut().filter(|e| e.alive && e.flags.contains_key("immortal")) {
                e.flags.insert("risen".into(), 1);
            }
        }
        // the needle snaps: Koschei is mortal, at two fifths of his life, and breathing his last
        "break_needle" => {
            for e in ex.s.enemies.iter_mut().filter(|e| e.alive && e.flags.contains_key("immortal")) {
                e.flags.remove("immortal");
                e.flags.insert("last_breath".into(), 5);
                e.hp = (e.max_hp * 2 / 5).max(1);
            }
        }
        // the duck tires and settles for a round, then takes wing again
        "duck_land" | "duck_fly" => {
            let UnitRef::Enemy(uid) = ctx.src else { return };
            if let Some(e) = ex.s.enemies.iter_mut().find(|e| e.uid == uid) {
                if name == "duck_land" {
                    e.flags.remove("flying");
                } else {
                    e.flags.insert("flying".into(), 1);
                }
            }
        }
        // Likho's single eye settles on a hero: every aimed blow now goes to them,
        // and for one turn they play their cards unseen
        "likho_mark" => {
            let Some(UnitRef::Hero(slot)) = ctx.target else { return };
            for h in ex.s.heroes.iter_mut() {
                h.flags.remove("marked");
            }
            let h = &mut ex.s.heroes[slot as usize];
            h.flags.insert("marked".into(), 1);
            h.flags.insert("blind".into(), 1);
        }
        // the Poloz wraps a hero: no attacks from them until the serpent takes 15 damage,
        // or their next turn ends
        "poloz_coil" => {
            let (UnitRef::Enemy(uid), Some(UnitRef::Hero(slot))) = (ctx.src, ctx.target) else { return };
            ex.s.heroes[slot as usize].flags.insert("coiled".into(), uid as i32);
            if let Some(e) = ex.s.enemies.iter_mut().find(|e| e.uid == uid) {
                e.flags.insert("coil_left".into(), 15);
            }
        }
        // the Leshy tangles the paths: next turn every hero's aimed card finds a random enemy
        "tangle_paths" => {
            for h in ex.s.heroes.iter_mut().filter(|h| !h.downed) {
                h.flags.insert("confused".into(), 1);
            }
        }
        // one-turn afflictions an enemy lays on the hero it aimed at
        "drag_under" | "charm" | "sleep" => {
            let Some(UnitRef::Hero(slot)) = ctx.target else { return };
            let flag = match name {
                "drag_under" => "energy_less",
                "charm" => "charmed",
                _ => "asleep",
            };
            ex.s.heroes[slot as usize].flags.insert(flag.into(), 1);
        }
        // ...and on the whole party
        "quake" | "sirin_song" | "freeze" => {
            let (flag, n) = match name {
                "quake" => ("draw_less", 2),
                "sirin_song" => ("no_intents", 1),
                _ => ("frozen", 2),
            };
            for h in ex.s.heroes.iter_mut().filter(|h| !h.downed) {
                h.flags.insert(flag.into(), n);
            }
        }
        // the Bolotnik drags heroes into the mire: two of them, but never the whole party
        "mire_grab" => {
            let mut standing: Vec<u8> = ex.s.standing_heroes().map(|h| h.slot).collect();
            ex.s.rng.shuffle(&mut standing);
            let take = if standing.len() <= 1 { standing.len() } else { 2.min(standing.len() - 1) };
            for slot in standing.into_iter().take(take) {
                ex.s.heroes[slot as usize].flags.insert("mired".into(), 1);
            }
        }
        // the vurdalak elder calls a fallen servant back at half strength
        "raise_dead" => {
            if let Some(e) = ex.s.enemies.iter_mut().find(|e| !e.alive && !e.fled) {
                e.alive = true;
                e.hp = (e.max_hp / 2).max(1);
                e.block = 0;
                e.statuses.clear();
                let (unit, hp) = (UnitRef::Enemy(e.uid), e.hp);
                ex.ev.push(Event::Heal { unit, amount: hp, hp });
            }
        }
        // the fever moves on to the next hero
        "pass_fever" => {
            let UnitRef::Hero(me) = ctx.src else { return };
            let standing: Vec<u8> = ex.s.standing_heroes().map(|h| h.slot).collect();
            let next = standing.iter().copied().find(|s| *s > me).or_else(|| standing.first().copied());
            if let Some(next) = next {
                ex.add_card(next, "fever", Pile::Draw);
            }
        }
        // Mara's winter deepens while her crown stands: one card fewer each turn, three at most
        "winter" => {
            if ex.s.enemies.iter().any(|e| e.alive && e.def == "ice_crown") {
                for h in ex.s.heroes.iter_mut().filter(|h| !h.downed) {
                    let w = h.flags.entry("winter".into()).or_insert(0);
                    *w = (*w + 1).min(3);
                }
            }
        }
        "thaw" => {
            for h in ex.s.heroes.iter_mut() {
                h.flags.remove("winter");
            }
        }
        _ => debug_assert!(false, "unknown hook `{name}`"),
    }
}
