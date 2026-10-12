//! The run layer: map, votes, fights, rewards, rest.

mod common;
use common::*;
use nav_core::*;

const SIX: [&str; 6] = ["bogatyr", "vorozheya", "volhv", "volkolak", "znaharka", "skomoroh"];

fn party(c: &Content, n: usize, seed: u64, circle: u8) -> Run {
    let heroes: Vec<Id> = SIX[..n].iter().map(|s| s.to_string()).collect();
    Run::new(c, &heroes, seed, circle).unwrap()
}

/// Everyone acts with the autopilot until the run ends; returns how many actions it took.
fn autoplay(c: &Content, run: &mut Run, limit: usize) -> usize {
    let mut steps = 0;
    while !matches!(run.stage, Stage::Victory | Stage::Defeat) {
        let mut moved = false;
        for p in 0..run.players.len() as u8 {
            if let Some(a) = run_bot(c, run, p) {
                run.apply(c, p, &a).unwrap_or_else(|e| panic!("bot move {a:?} refused: {e:?}"));
                moved = true;
                steps += 1;
            }
        }
        assert!(moved, "the run is stuck in {:?}", run.stage);
        assert!(steps < limit, "the run did not end in {limit} actions");
    }
    steps
}

#[test]
fn maps_follow_the_floor_rules() {
    for seed in 0..60 {
        let map = generate_map(&mut Rng::new(seed), Circle(0));
        assert_eq!(map.floors.len(), FLOORS);
        for (f, floor) in map.floors.iter().enumerate() {
            assert!(!floor.is_empty() && floor.len() <= MAP_WIDTH as usize);
            for node in floor {
                match f {
                    0 => assert_eq!(node.kind, NodeKind::Fight, "floor 1 is always a fight"),
                    8 => assert_eq!(node.kind, NodeKind::Chest, "floor 9 is always a chest"),
                    14 => assert_eq!(node.kind, NodeKind::Rest, "floor 15 is always a fire"),
                    _ => {
                        assert!(node.kind != NodeKind::Elite || f >= 5, "no elite before floor 6");
                        assert!(node.kind != NodeKind::Shop || f >= 2, "no shop before floor 3");
                        assert!(!matches!(node.kind, NodeKind::Chest | NodeKind::Boss));
                    }
                }
                if f + 1 < FLOORS {
                    assert!(!node.next.is_empty(), "seed {seed}: a dead end on floor {f}");
                    assert!(node.next.iter().all(|i| (*i as usize) < map.floors[f + 1].len()));
                }
            }
            // every node above the first floor can be reached
            if f > 0 {
                for i in 0..floor.len() as u8 {
                    assert!(map.floors[f - 1].iter().any(|p| p.next.contains(&i)), "seed {seed}: orphan node on floor {f}");
                }
            }
        }
    }
}

#[test]
fn node_shares_are_roughly_as_designed() {
    let mut count = std::collections::BTreeMap::new();
    let mut total = 0;
    for seed in 0..300 {
        let map = generate_map(&mut Rng::new(seed), Circle(0));
        for (f, floor) in map.floors.iter().enumerate() {
            if matches!(f, 0 | 8 | 14) {
                continue;
            }
            for n in floor {
                *count.entry(n.kind).or_insert(0) += 1;
                total += 1;
            }
        }
    }
    let share = |k| 100 * count.get(&k).copied().unwrap_or(0) / total;
    assert!((40..=62).contains(&share(NodeKind::Fight)), "fights {}%", share(NodeKind::Fight));
    assert!((15..=30).contains(&share(NodeKind::Event)), "omens {}%", share(NodeKind::Event));
    assert!((6..=16).contains(&share(NodeKind::Rest)), "fires {}%", share(NodeKind::Rest));
    assert!((2..=12).contains(&share(NodeKind::Elite)), "elites {}%", share(NodeKind::Elite));
    assert!((2..=10).contains(&share(NodeKind::Shop)), "shops {}%", share(NodeKind::Shop));
}

#[test]
fn the_first_circle_brings_more_elites() {
    let elites = |circle| -> usize {
        (0..200).map(|seed| generate_map(&mut Rng::new(seed), Circle(circle)).floors.iter().flatten().filter(|n| n.kind == NodeKind::Elite).count()).sum()
    };
    assert!(elites(1) > elites(0));
}

#[test]
fn a_solo_vote_moves_at_once_into_the_first_fight() {
    let c = content();
    let mut run = party(&c, 1, 3, 0);
    assert!(run.choices().iter().all(|(_, k)| *k == NodeKind::Fight));
    let ev = run.apply(&c, 0, &RunAction::Vote(0)).unwrap();
    assert_eq!(run.stage, Stage::Combat);
    assert_eq!(run.at, Some((0, run.choices_origin())));
    assert!(ev.contains(&RunEvent::CombatStarted));
    assert_eq!(run.combat.as_ref().unwrap().heroes[0].def, "bogatyr");
    assert_eq!(run.apply(&c, 0, &RunAction::Vote(0)), Err(RunError::WrongStage));
}

trait Origin {
    fn choices_origin(&self) -> u8;
}
impl Origin for Run {
    /// The node index the party is standing on.
    fn choices_origin(&self) -> u8 {
        self.at.map(|a| a.1).unwrap_or(0)
    }
}

#[test]
fn the_party_waits_for_every_vote_and_majority_wins() {
    let c = content();
    // a seed whose first floor offers at least two ways
    let mut run = (0..50).map(|s| party(&c, 3, s, 0)).find(|r| r.choices().len() >= 2).unwrap();
    run.apply(&c, 0, &RunAction::Vote(1)).unwrap();
    run.apply(&c, 1, &RunAction::Vote(0)).unwrap();
    assert!(matches!(run.stage, Stage::Map { .. }), "two of three have voted");
    assert_eq!(run.apply(&c, 2, &RunAction::Vote(9)), Err(RunError::BadChoice));
    // a player may move their token before the last vote lands
    run.apply(&c, 1, &RunAction::Vote(1)).unwrap();
    let want = run.choices()[1].0;
    let ev = run.apply(&c, 2, &RunAction::Vote(0)).unwrap();
    assert_eq!(run.at, Some((0, want)));
    assert!(!ev.iter().any(|e| matches!(e, RunEvent::Lot { .. })));
}

#[test]
fn a_tied_vote_is_settled_by_lot_not_by_the_host() {
    let c = content();
    let mut picked = std::collections::BTreeSet::new();
    for seed in 0..80 {
        let mut run = party(&c, 2, seed, 0);
        if run.choices().len() < 2 {
            continue;
        }
        run.apply(&c, 0, &RunAction::Vote(0)).unwrap();
        let ev = run.apply(&c, 1, &RunAction::Vote(1)).unwrap();
        let lot = ev.iter().find_map(|e| if let RunEvent::Lot { tied, chosen } = e { Some((tied.clone(), *chosen)) } else { None });
        let (tied, chosen) = lot.expect("a tie draws lots");
        assert_eq!(tied, vec![0, 1]);
        picked.insert(chosen);
    }
    assert_eq!(picked.len(), 2, "over many runs the lot falls both ways");
}

#[test]
fn rewards_offer_three_cards_of_your_own_hero() {
    let c = content();
    let mut run = party(&c, 1, 3, 0);
    run.apply(&c, 0, &RunAction::Vote(0)).unwrap();
    let gold = run.players[0].gold;
    while run.stage == Stage::Combat {
        let a = run_bot(&c, &run, 0).unwrap();
        run.apply(&c, 0, &a).unwrap();
    }
    let Stage::Reward { rewards } = &run.stage else { panic!("expected rewards, got {:?}", run.stage) };
    let r = &rewards[0];
    assert_eq!(r.cards.len(), 3);
    assert!((10..=20).contains(&r.gold));
    assert_eq!(run.players[0].gold, gold + r.gold, "gold is paid at once");
    for card in &r.cards {
        let d = &c.cards[card];
        assert_eq!(d.hero, "bogatyr");
        assert!(!d.keywords.contains(&Keyword::Coop), "no co-op cards for a lone hero");
        assert_ne!(d.rarity, Rarity::Starter);
    }
    let mut unique = r.cards.clone();
    unique.dedup();
    assert_eq!(unique.len(), 3);

    let (deck, want) = (run.players[0].deck.len(), r.cards[1].clone());
    run.apply(&c, 0, &RunAction::TakeCard(Some(1))).unwrap();
    assert_eq!(run.players[0].deck.len(), deck + 1);
    assert_eq!(run.players[0].deck.last().unwrap().0, want);
    assert!(matches!(run.stage, Stage::Map { .. }));
    assert_eq!(run.floors_cleared, 1);
}

#[test]
fn fire_heals_thirty_percent_or_upgrades_one_card() {
    let c = content();
    let mut run = party(&c, 2, 3, 0);
    run.stage = Stage::Rest { done: vec![false, false] };
    run.players[0].hp = 10;
    run.apply(&c, 0, &RunAction::Sleep).unwrap();
    assert_eq!(run.players[0].hp, 10 + 85 * 30 / 100);
    assert_eq!(run.apply(&c, 0, &RunAction::Sleep), Err(RunError::AlreadyDone));
    assert!(matches!(run.stage, Stage::Rest { .. }), "the other player is still at the fire");

    let gold = (run.players[0].gold, run.players[1].gold);
    run.apply(&c, 0, &RunAction::GiveGold { to: 1, amount: 40 }).unwrap();
    assert_eq!((run.players[0].gold, run.players[1].gold), (gold.0 - 40, gold.1 + 40));
    assert_eq!(run.apply(&c, 0, &RunAction::GiveGold { to: 1, amount: 9999 }), Err(RunError::NotEnoughGold));

    run.apply(&c, 1, &RunAction::Upgrade(0)).unwrap();
    assert!(run.players[1].deck[0].1);
    assert!(matches!(run.stage, Stage::Map { .. }));

    // Circle 5: the fire warms less
    let mut run = party(&c, 1, 3, 5);
    run.stage = Stage::Rest { done: vec![false] };
    run.players[0].hp = 10;
    run.apply(&c, 0, &RunAction::Sleep).unwrap();
    assert_eq!(run.players[0].hp, 10 + 85 * 20 / 100);
}

#[test]
fn an_upgraded_card_cannot_be_upgraded_again() {
    let c = content();
    let mut run = party(&c, 1, 3, 0);
    run.stage = Stage::Rest { done: vec![false] };
    run.players[0].deck[0].1 = true;
    assert_eq!(run.apply(&c, 0, &RunAction::Upgrade(0)), Err(RunError::BadChoice));
    assert_eq!(run.apply(&c, 0, &RunAction::Upgrade(999)), Err(RunError::BadChoice));
}

#[test]
fn circles_change_the_rules() {
    let c = content();
    let run = party(&c, 1, 3, 6);
    assert!(run.players[0].deck.iter().any(|(id, _)| id == "toska"), "Circle 6 starts with Longing");
    let mut run = party(&c, 1, 3, 9);
    run.apply(&c, 0, &RunAction::Vote(0)).unwrap();
    let combat = run.combat.as_ref().unwrap();
    assert_eq!(combat.raise_cost, 3);
    assert_eq!(combat.enemy_dmg, 110);
    assert_eq!(combat.hp_scale, 1100);
}

#[test]
fn wounds_falls_and_curses_carry_between_fights() {
    let c = content();
    let mut run = party(&c, 1, 3, 0);
    run.apply(&c, 0, &RunAction::Vote(0)).unwrap();
    {
        let combat = run.combat.as_mut().unwrap();
        combat.heroes[0].hp = 40;
        combat.heroes[0].falls = 2;
        combat.heroes[0].new_curses = 1;
        for e in &mut combat.enemies {
            e.hp = 1;
        }
    }
    while run.stage == Stage::Combat {
        let a = run_bot(&c, &run, 0).unwrap();
        run.apply(&c, 0, &a).unwrap();
    }
    let p = &run.players[0];
    assert!(p.hp <= 40 && p.hp > 0);
    assert_eq!(p.falls, 2);
    assert_eq!(p.deck.iter().filter(|(id, _)| id == NAV_MARK).count(), 1);
}

#[test]
fn every_hero_can_be_driven_through_a_whole_run() {
    let c = content();
    for hero in SIX {
        for seed in [1u64, 2] {
            let mut run = Run::new(&c, &[hero.to_string()], seed, 0).unwrap();
            autoplay(&c, &mut run, 60_000);
            // whichever way it ended, the books must add up
            assert!(run.floors_cleared > 0, "{hero} never cleared a floor");
            assert!(run.oberegi() >= run.floors_cleared);
            if run.stage == Stage::Victory {
                assert_eq!((run.act, run.bosses_slain), (3, 3));
            }
        }
    }
}

#[test]
fn a_full_party_run_replays_identically() {
    let c = content();
    let play = |seed| {
        let mut run = party(&c, 4, seed, 0);
        let steps = autoplay(&c, &mut run, 200_000);
        (run.hash64(), steps, run.stage.clone())
    };
    assert_eq!(play(11), play(11));
    assert_ne!(play(11).0, play(12).0);
}

/// Not a pass/fail check: prints how autopilot runs end, for balance reading.
#[test]
#[ignore]
fn report_autopilot_outcomes() {
    let c = content();
    for hero in SIX {
        let mut wins = 0;
        let mut floors = 0;
        for seed in 0..40u64 {
            let mut run = Run::new(&c, &[hero.to_string()], seed, 0).unwrap();
            autoplay(&c, &mut run, 60_000);
            wins += (run.stage == Stage::Victory) as u32;
            floors += run.floors_cleared;
        }
        println!("{hero:<10} wins {wins}/40, avg floors cleared {:.1} of 48", floors as f32 / 40.0);
    }
    let mut wins = 0;
    for seed in 0..20u64 {
        let mut run = party(&c, 4, seed, 0);
        autoplay(&c, &mut run, 200_000);
        wins += (run.stage == Stage::Victory) as u32;
    }
    println!("party of 4 wins {wins}/20");
}

// ---- potions, the pedlar, omens ----

#[test]
fn potions_are_drunk_in_a_fight_and_carried_out() {
    let c = content();
    let mut run = party(&c, 1, 3, 0);
    run.players[0].potions = vec!["mead".into(), "fire_water".into()];
    run.apply(&c, 0, &RunAction::Vote(0)).unwrap();
    let hp: Vec<i32> = run.combat.as_ref().unwrap().enemies.iter().map(|e| e.hp).collect();
    run.apply(&c, 0, &RunAction::Combat(CombatAction::UsePotion { slot: 0, target: None })).unwrap();
    assert_eq!(run.combat.as_ref().unwrap().heroes[0].energy, 5);
    run.apply(&c, 0, &RunAction::Combat(CombatAction::UsePotion { slot: 0, target: None })).unwrap();
    let after: Vec<i32> = run.combat.as_ref().unwrap().enemies.iter().map(|e| e.hp).collect();
    assert!(hp.iter().zip(&after).all(|(a, b)| *b == (*a - 10).max(0)));
    assert_eq!(
        run.apply(&c, 0, &RunAction::Combat(CombatAction::UsePotion { slot: 0, target: None })),
        Err(RunError::Combat(ActionError::NoSuchPotion))
    );
}

#[test]
fn the_pedlar_sells_removes_and_lets_you_leave() {
    let c = content();
    let mut run = party(&c, 2, 3, 0);
    // put the party at a shop by hand
    let stock: Vec<Stock> = vec![
        Stock { cards: vec![Some(("bogatyr_shield_wall".into(), 50))], relics: vec![], potions: vec![Some(("mead".into(), 45)), Some(("tar".into(), 45))] },
        Stock::default(),
    ];
    run.stage = Stage::Shop { stock, done: vec![false, false] };
    run.players[0].gold = 100;
    let deck = run.players[0].deck.len();
    run.apply(&c, 0, &RunAction::BuyCard(0)).unwrap();
    assert_eq!((run.players[0].gold, run.players[0].deck.len()), (50, deck + 1));
    assert_eq!(run.apply(&c, 0, &RunAction::BuyCard(0)), Err(RunError::SoldOut));
    run.apply(&c, 0, &RunAction::BuyPotion(0)).unwrap();
    assert_eq!(run.apply(&c, 0, &RunAction::BuyPotion(1)), Err(RunError::NotEnoughGold));
    assert_eq!(run.players[0].potions, vec!["mead".to_string()]);

    // removal costs 75, then 100
    run.players[1].gold = 180;
    let deck = run.players[1].deck.len();
    run.apply(&c, 1, &RunAction::RemoveCard(0)).unwrap();
    run.apply(&c, 1, &RunAction::RemoveCard(0)).unwrap();
    assert_eq!((run.players[1].gold, run.players[1].deck.len(), run.players[1].removals), (5, deck - 2, 2));

    run.apply(&c, 0, &RunAction::GiveGold { to: 1, amount: 5 }).unwrap();
    run.apply(&c, 0, &RunAction::LeaveShop).unwrap();
    assert!(matches!(run.stage, Stage::Shop { .. }));
    run.apply(&c, 1, &RunAction::LeaveShop).unwrap();
    assert!(matches!(run.stage, Stage::Map { .. }));
}

#[test]
fn pedlar_shelves_are_stocked_for_your_hero() {
    let c = content();
    let mut run = party(&c, 1, 3, 0);
    // walk the map by hand to the first shop on any path; simpler: ask for stock via a shop node
    run.stage = Stage::Map { votes: vec![None] };
    for seed in 0..200 {
        let mut r = party(&c, 1, seed, 0);
        // find a seed whose first floors lead to a shop quickly by brute force over the bot
        for _ in 0..400 {
            if let Stage::Shop { stock, .. } = &r.stage {
                let s = &stock[0];
                assert!(s.cards.len() >= 3, "shelves hold cards");
                for (id, price) in s.cards.iter().flatten() {
                    let d = &c.cards[id];
                    assert!(d.hero == "bogatyr" || d.hero.is_empty());
                    assert!(*price >= 45);
                }
                assert_eq!(s.potions.len(), 3);
                return;
            }
            if matches!(r.stage, Stage::Victory | Stage::Defeat) {
                break;
            }
            let Some(a) = run_bot(&c, &r, 0) else { break };
            r.apply(&c, 0, &a).unwrap();
        }
    }
    panic!("no run ever reached a pedlar");
}

#[test]
fn a_personal_omen_is_each_players_own_choice() {
    let c = content();
    let mut run = party(&c, 2, 3, 0);
    run.stage = Stage::Omen { id: "hungry_village".into(), picks: vec![None, None] };
    run.players[0].gold = 40;
    run.players[1].gold = 10;
    let relics = run.players[0].relics.len();
    run.apply(&c, 0, &RunAction::Choose(0)).unwrap();
    assert_eq!(run.players[0].gold, 0);
    // no relic pool beyond starters yet means nothing to hand out; the price is still paid
    assert!(run.players[0].relics.len() >= relics);
    assert_eq!(run.apply(&c, 0, &RunAction::Choose(2)), Err(RunError::AlreadyDone));
    assert_eq!(run.apply(&c, 1, &RunAction::Choose(0)), Err(RunError::NotEnoughGold));
    run.apply(&c, 1, &RunAction::Choose(1)).unwrap();
    assert_eq!(run.players[1].gold, 60);
    assert!(run.players[1].deck.iter().any(|(id, _)| id == "toska"));
    assert!(matches!(run.stage, Stage::Map { .. }));
}

#[test]
fn a_vote_omen_binds_the_whole_party_and_never_kills() {
    let c = content();
    let mut run = party(&c, 3, 3, 0);
    run.stage = Stage::Omen { id: "crossroads_stone".into(), picks: vec![None, None, None] };
    run.players[2].hp = 3;
    run.apply(&c, 0, &RunAction::Choose(1)).unwrap();
    run.apply(&c, 1, &RunAction::Choose(0)).unwrap();
    assert!(matches!(run.stage, Stage::Omen { .. }));
    let ev = run.apply(&c, 2, &RunAction::Choose(1)).unwrap();
    assert!(ev.contains(&RunEvent::OmenResolved { option: 1 }));
    assert_eq!(run.players[0].hp, 85 - 85 * 12 / 100);
    assert_eq!(run.players[2].hp, 1, "an omen wounds but does not kill");
    assert!(matches!(run.stage, Stage::Map { .. }));
}

/// Not a pass/fail check: prints which fights end solo autopilot runs.
#[test]
#[ignore]
fn report_where_solo_runs_die() {
    let c = content();
    let mut deaths: std::collections::BTreeMap<String, u32> = Default::default();
    let mut floors: Vec<u32> = Vec::new();
    for hero in SIX {
        for seed in 0..40u64 {
            let mut run = Run::new(&c, &[hero.to_string()], seed, 0).unwrap();
            autoplay(&c, &mut run, 60_000);
            if run.stage == Stage::Defeat {
                let mut names: Vec<String> = run.combat.as_ref().map(|k| k.enemies.iter().map(|e| e.def.clone()).collect()).unwrap_or_default();
                names.sort();
                names.dedup();
                *deaths.entry(format!("act {} {:?} {}", run.act, run.fighting.unwrap_or(NodeKind::Fight), names.join("+"))).or_insert(0) += 1;
                floors.push(run.floors_cleared);
            }
        }
    }
    let mut rows: Vec<_> = deaths.into_iter().collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.1));
    for (k, n) in rows.iter().take(12) {
        println!("{n:>4}  {k}");
    }
    floors.sort();
    println!("median floors cleared at death: {}", floors.get(floors.len() / 2).copied().unwrap_or(0));
}
