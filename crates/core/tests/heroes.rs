//! The six heroes with their real cards and relics.

mod common;
use common::*;
use nav_core::*;
use StatusKind as S;

const ME: UnitRef = UnitRef::Hero(0);

/// A real hero with a chosen deck and their starting relic.
fn with(c: &Content, hero_id: &str, deck: &[&str]) -> HeroSetup {
    let d = &c.heroes[hero_id];
    HeroSetup { def: hero_id.into(), hp: d.max_hp, max_hp: d.max_hp, deck: deck.iter().map(|x| (x.to_string(), false)).collect(), relics: vec![d.relic.clone()], falls: 0, potions: vec![] }
}

fn fight(hero_id: &str, deck: &[&str], enemies: &[&str]) -> (Content, Combat) {
    let c = content();
    let (mut s, _) = start(&c, &[with(&c, hero_id, deck)], &ids(enemies), 5);
    s.heroes[0].energy = 20;
    (c, s)
}

#[test]
fn every_hero_has_ten_cards_and_the_documented_hp() {
    let c = content();
    for (id, hp) in [("bogatyr", 85), ("vorozheya", 70), ("volhv", 72), ("volkolak", 80), ("znaharka", 68), ("skomoroh", 70)] {
        let h = &c.heroes[id];
        assert_eq!((h.max_hp, h.deck.len()), (hp, 10), "{id}");
    }
}

#[test]
fn every_hero_wins_the_first_fights_on_autopilot() {
    let c = content();
    for hero_id in c.heroes.keys().filter(|h| !h.starts_with("t_")) {
        for enemies in [vec!["grey_wolf", "grey_wolf"], vec!["kikimora"]] {
            let (mut s, _) = start(&c, &[HeroSetup::fresh(&c, hero_id).unwrap()], &ids(&enemies), 21);
            for _ in 0..600 {
                if s.phase != Phase::Heroes {
                    break;
                }
                let a = bot::choose(&c, &s, 0);
                apply(&c, &mut s, 0, &a).unwrap_or_else(|e| panic!("{hero_id}: illegal bot move {a:?}: {e:?}"));
            }
            assert_eq!(s.phase, Phase::Won, "{hero_id} vs {enemies:?}");
        }
    }
}

// ---- Vorozheya ----

#[test]
fn motanka_doll_rots_someone_at_the_start() {
    let (_, s) = fight("vorozheya", &[], &["dummy"]);
    assert_eq!(s.status(foe(&s, 0), S::Rot), 3);
}

#[test]
fn lifting_the_curse_cashes_in_every_mark() {
    let (c, mut s) = fight("vorozheya", &["vor_whisper", "vor_hex", "vor_black_candle", "vor_lift"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "vor_hex", Some(t));
    play(&c, &mut s, 0, "vor_black_candle", Some(t));
    assert_eq!(s.status(t, S::Hex), 4, "black candle doubles the Evil Eye");
    play(&c, &mut s, 0, "vor_whisper", Some(t));
    // 5 + 3 from the Evil Eye; rot 3 from the doll + 1
    assert_eq!((s.enemies[0].hp, s.status(t, S::Rot), s.status(t, S::Hex)), (92, 4, 3));
    play(&c, &mut s, 0, "vor_lift", Some(t));
    assert_eq!(s.enemies[0].hp, 92 - 3 * 7);
    assert_eq!((s.status(t, S::Rot), s.status(t, S::Hex)), (0, 0));
}

#[test]
fn pestilence_makes_rot_bite_twice_and_stay() {
    let (c, mut s) = fight("vorozheya", &["vor_pestilence"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "vor_pestilence", None);
    end_round(&c, &mut s);
    assert_eq!((s.enemies[0].hp, s.status(t, S::Rot)), (94, 3));
}

#[test]
fn transfer_moves_debuffs_to_the_enemy_and_blood_pact_feeds_an_ally() {
    let c = content();
    let party = [with(&c, "vorozheya", &["vor_transfer", "vor_blood_pact", "t_weak_me"]), hero("t_plain", &[])];
    let (mut s, _) = start(&c, &party, &ids(&["dummy"]), 5);
    let t = foe(&s, 0);
    s.heroes[1].statuses.insert(S::Vulnerable, 2);
    play(&c, &mut s, 0, "t_weak_me", None);
    play(&c, &mut s, 0, "vor_transfer", Some(t));
    assert_eq!((s.status(ME, S::Weak), s.status(UnitRef::Hero(1), S::Vulnerable)), (0, 0));
    assert_eq!((s.status(t, S::Weak), s.status(t, S::Vulnerable)), (1, 2));
    play(&c, &mut s, 0, "vor_blood_pact", Some(UnitRef::Hero(1)));
    assert_eq!((s.heroes[0].hp, s.heroes[1].energy), (66, 5));
}

// ---- Volkhv ----

#[test]
fn the_staff_doubles_the_first_rune_and_perun_counts_thunder() {
    let (c, mut s) = fight("volhv", &["vol_thunder", "vol_thunder", "vol_perun", "vol_invoke"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "vol_thunder", Some(ME));
    play(&c, &mut s, 0, "vol_thunder", Some(ME));
    play(&c, &mut s, 0, "vol_perun", Some(t));
    assert_eq!(s.enemies[0].hp, 90);
    play(&c, &mut s, 0, "vol_invoke", None);
    // three strikes of 6: the first rune fires twice
    assert_eq!(s.enemies[0].hp, 72);
}

#[test]
fn storm_hits_everyone_and_the_dream_makes_a_rune_free() {
    let (c, mut s) = fight("volhv", &["vol_storm", "vol_prophetic_dream", "vol_thunder", "vol_invoke"], &["dummy", "dummy"]);
    s.heroes[0].flags.remove("first_rune_twice");
    play(&c, &mut s, 0, "vol_storm", None);
    play(&c, &mut s, 0, "vol_prophetic_dream", None);
    let before = s.heroes[0].energy;
    play(&c, &mut s, 0, "vol_thunder", Some(ME));
    assert_eq!(s.heroes[0].energy, before, "the rune after the dream is free");
    play(&c, &mut s, 0, "vol_invoke", None);
    assert_eq!((s.enemies[0].hp, s.enemies[1].hp), (94, 94));
}

#[test]
fn kolyada_fires_a_rune_per_energy() {
    let (c, mut s) = fight("volhv", &["vol_frost", "vol_kolyada"], &["dummy"]);
    s.heroes[0].flags.remove("first_rune_twice");
    play(&c, &mut s, 0, "vol_frost", Some(ME));
    s.heroes[0].energy = 3;
    play(&c, &mut s, 0, "vol_kolyada", None);
    assert_eq!((s.heroes[0].block, s.heroes[0].energy), (12, 0));
}

// ---- Volkolak ----

#[test]
fn the_fang_draws_on_the_first_shift_only() {
    let (c, mut s) = fight("volkolak", &["wolf_shift", "wolf_shift", "wolf_knife", "wolf_knife", "wolf_knife", "wolf_dodge", "wolf_dodge"], &["dummy"]);
    let hand = s.heroes[0].hand.len();
    let shifts = s.heroes[0].hand.iter().filter(|x| x.def == "wolf_shift").count();
    if shifts == 0 {
        return;
    }
    play(&c, &mut s, 0, "wolf_shift", None);
    assert_eq!((s.heroes[0].form, s.heroes[0].hand.len()), (Form::Beast, hand - 1 + 2));
    if s.heroes[0].hand.iter().any(|x| x.def == "wolf_shift") {
        let hand = s.heroes[0].hand.len();
        play(&c, &mut s, 0, "wolf_shift", None);
        assert_eq!((s.heroes[0].form, s.heroes[0].hand.len()), (Form::Human, hand - 1));
    }
}

#[test]
fn beast_side_of_the_cards() {
    let (c, mut s) = fight("volkolak", &["wolf_shift", "wolf_knife", "wolf_dodge", "wolf_maul"], &["hitter"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "wolf_shift", None);
    play(&c, &mut s, 0, "wolf_knife", Some(t));
    assert_eq!((s.enemies[0].hp, s.status(t, S::Bleed)), (87, 2), "claws: 9 x1.5 and bleeding");
    play(&c, &mut s, 0, "wolf_dodge", Some(t));
    assert_eq!(s.enemies[0].hp, 81, "the leap is an attack in Beast form");
    play(&c, &mut s, 0, "wolf_maul", Some(t));
    assert_eq!(s.enemies[0].hp, 81 - 4 * 9);
}

#[test]
fn silver_chain_holds_the_beast_back_and_blood_moon_ends_hunger() {
    let (c, mut s) = fight("volkolak", &["wolf_silver_chain", "wolf_shift"], &["dummy"]);
    play(&c, &mut s, 0, "wolf_silver_chain", None);
    play(&c, &mut s, 0, "wolf_shift", None);
    assert_eq!((s.heroes[0].form, s.heroes[0].block), (Form::Human, 12));

    let (c, mut s) = fight("volkolak", &["wolf_blood_moon", "wolf_shift"], &["dummy"]);
    play(&c, &mut s, 0, "wolf_blood_moon", None);
    play(&c, &mut s, 0, "wolf_shift", None);
    end_round(&c, &mut s);
    assert_eq!((s.heroes[0].form, s.heroes[0].hp), (Form::Beast, 80));
}

#[test]
fn devour_heals_and_grows() {
    let (c, mut s) = fight("volkolak", &["wolf_devour", "t_pain"], &["weakling", "dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_pain", None);
    play(&c, &mut s, 0, "wolf_devour", Some(t));
    assert_eq!((s.heroes[0].max_hp, s.heroes[0].hp), (81, 81));
}

// ---- Znakharka ----

#[test]
fn herbs_act_at_once_and_brew_by_recipe() {
    let (c, mut s) = fight("znaharka", &["herb_wormwood", "herb_wormwood", "zn_brew"], &["dummy", "dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "herb_wormwood", Some(t));
    play(&c, &mut s, 0, "herb_wormwood", Some(t));
    assert_eq!((s.enemies[0].hp, s.heroes[0].cauldron.len()), (92, 2));
    play(&c, &mut s, 0, "zn_brew", Some(t));
    assert_eq!((s.enemies[0].hp, s.enemies[1].hp), (86, 94), "wormwood twice: 6 to all");
    assert!(s.heroes[0].cauldron.is_empty());
}

#[test]
fn documented_recipes_and_the_fern() {
    let (c, mut s) = fight("znaharka", &["herb_sleep", "herb_weep", "zn_brew", "t_pain", "t_pain"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "t_pain", None);
    play(&c, &mut s, 0, "t_pain", None);
    play(&c, &mut s, 0, "herb_sleep", None);
    play(&c, &mut s, 0, "herb_weep", None);
    assert_eq!(s.heroes[0].hp, 62);
    play(&c, &mut s, 0, "zn_brew", Some(t));
    assert_eq!(s.heroes[0].hp, 67, "sleep-grass and weeping-grass: heal 5");

    let (c, mut s) = fight("znaharka", &["herb_burst", "herb_fern", "zn_brew"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "herb_burst", Some(t));
    play(&c, &mut s, 0, "herb_fern", None);
    play(&c, &mut s, 0, "zn_brew", Some(t));
    // 5 on play; the fern stands in for a second burst-grass (24), and doubles the brew
    assert_eq!(s.enemies[0].hp, 100 - 5 - 48);
}

#[test]
fn the_birch_pot_holds_four_and_brew_for_all_widens() {
    let (c, mut s) = fight("znaharka", &["herb_sleep", "herb_sleep", "herb_sleep", "herb_sleep", "herb_sleep"], &["dummy"]);
    for _ in 0..5 {
        play(&c, &mut s, 0, "herb_sleep", None);
    }
    assert_eq!(s.heroes[0].cauldron.len(), 4);

    let (c, mut s) = fight("znaharka", &["herb_burst", "zn_brew_for_all", "zn_brew"], &["dummy", "dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "herb_burst", Some(t));
    play(&c, &mut s, 0, "zn_brew_for_all", None);
    play(&c, &mut s, 0, "zn_brew", Some(t));
    assert_eq!((s.enemies[0].hp, s.enemies[1].hp), (85, 90));
}

#[test]
fn living_water_raises_at_half() {
    let c = content();
    let party = [with(&c, "znaharka", &["zn_living_water"]), hero("t_plain", &["t_suicide"])];
    let (mut s, _) = start(&c, &party, &ids(&["dummy"]), 5);
    play(&c, &mut s, 1, "t_suicide", None);
    play(&c, &mut s, 0, "zn_living_water", Some(UnitRef::Hero(1)));
    assert_eq!((s.heroes[1].downed, s.heroes[1].hp), (false, 25));
    assert_eq!(s.heroes[0].exhaust.len(), 1);
}

// ---- Skomorokh ----

#[test]
fn echo_copies_an_ally_for_free_and_the_copy_burns() {
    let c = content();
    let party = [with(&c, "skomoroh", &["sk_echo"]), hero("t_plain", &["t_hit"])];
    let (mut s, _) = start(&c, &party, &ids(&["dummy"]), 5);
    let t = foe(&s, 0);
    play(&c, &mut s, 1, "t_hit", Some(t));
    play(&c, &mut s, 0, "sk_echo", None);
    let copy = s.heroes[0].hand.iter().find(|x| x.def == "t_hit").expect("echo gives a copy");
    assert!(copy.free);
    let energy = s.heroes[0].energy;
    play(&c, &mut s, 0, "t_hit", Some(t));
    assert_eq!((s.heroes[0].energy, s.enemies[0].hp), (energy, 180 - 20));
    assert!(s.heroes[0].discard.iter().all(|x| x.def != "t_hit"), "the copy is not kept");
}

#[test]
fn echo_alone_repeats_your_own_card() {
    let (c, mut s) = fight("skomoroh", &["sk_dance", "sk_echo"], &["dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "sk_dance", Some(t));
    play(&c, &mut s, 0, "sk_echo", None);
    play(&c, &mut s, 0, "sk_dance", Some(t));
    assert_eq!(s.enemies[0].hp, 90);
}

#[test]
fn bells_draw_once_a_turn_and_mummers_pay_the_party() {
    let c = content();
    let party = [with(&c, "skomoroh", &["sk_mummers", "sk_mask_bear", "sk_mask_goat", "sk_dance", "sk_dance", "sk_dance", "sk_dance"]), hero("t_plain", &[])];
    let (mut s, _) = start(&c, &party, &ids(&["dummy"]), 5);
    s.heroes[0].energy = 20;
    let has = |s: &Combat, id: &str| s.heroes[0].hand.iter().any(|x| x.def == id);
    if !(has(&s, "sk_mummers") && has(&s, "sk_mask_bear") && has(&s, "sk_mask_goat")) {
        return;
    }
    play(&c, &mut s, 0, "sk_mummers", None);
    let hand = s.heroes[0].hand.len();
    play(&c, &mut s, 0, "sk_mask_bear", None);
    assert_eq!((s.heroes[0].hand.len(), s.heroes[1].energy), (hand, 4), "one card back, one energy to the ally");
    let hand = s.heroes[0].hand.len();
    play(&c, &mut s, 0, "sk_mask_goat", None);
    assert_eq!((s.heroes[0].hand.len(), s.heroes[1].energy), (hand - 1, 4), "only the first change each turn pays");
}

#[test]
fn switcheroo_turns_a_blow_on_another_enemy() {
    let (c, mut s) = fight("skomoroh", &["sk_switcheroo"], &["hitter", "dummy"]);
    let t = foe(&s, 0);
    play(&c, &mut s, 0, "sk_switcheroo", Some(t));
    end_round(&c, &mut s);
    assert_eq!((s.heroes[0].hp, s.enemies[1].hp), (70, 90));
}

#[test]
fn death_dance_counts_the_whole_partys_cards() {
    let c = content();
    let party = [with(&c, "skomoroh", &["sk_death_dance"]), hero("t_plain", &["t_block", "t_block"])];
    let (mut s, _) = start(&c, &party, &ids(&["dummy"]), 5);
    let t = foe(&s, 0);
    play(&c, &mut s, 1, "t_block", None);
    play(&c, &mut s, 1, "t_block", None);
    play(&c, &mut s, 0, "sk_death_dance", Some(t));
    assert_eq!(s.enemies[0].hp, 180 - 6);
}
