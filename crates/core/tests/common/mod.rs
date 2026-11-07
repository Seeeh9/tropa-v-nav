#![allow(dead_code)]
use nav_core::*;
use std::path::PathBuf;

pub const CARDS: &str = r#"[
    (id: "t_hit", kind: Attack, cost: 0, target: Enemy, effects: [Damage(N(10), Target)]),
    (id: "t_hit_all", kind: Attack, cost: 0, effects: [Damage(N(4), AllFoes)]),
    (id: "t_multi", kind: Attack, cost: 0, target: Enemy, effects: [Hits(N(2), 3, Target)]),
    (id: "t_block", kind: Skill, cost: 0, effects: [Block(N(5), Me)]),
    (id: "t_cost2", kind: Skill, cost: 2, effects: [Block(N(1), Me)]),
    (id: "t_cost3", kind: Skill, cost: 3, effects: [Block(N(1), Me)]),
    (id: "t_str", kind: Skill, cost: 0, effects: [Apply(Strength, N(2), Me)]),
    (id: "t_fort", kind: Skill, cost: 0, effects: [Apply(Fortitude, N(2), Me)]),
    (id: "t_weak_me", kind: Skill, cost: 0, effects: [Apply(Weak, N(1), Me)]),
    (id: "t_vuln", kind: Skill, cost: 0, target: Enemy, effects: [Apply(Vulnerable, N(2), Target)]),
    (id: "t_weak", kind: Skill, cost: 0, target: Enemy, effects: [Apply(Weak, N(2), Target)]),
    (id: "t_rot", kind: Skill, cost: 0, target: Enemy, effects: [Apply(Rot, N(3), Target)]),
    (id: "t_hex", kind: Skill, cost: 0, target: Enemy, effects: [Apply(Hex, N(2), Target)]),
    (id: "t_bleed", kind: Skill, cost: 0, target: Enemy, effects: [Apply(Bleed, N(4), Target)]),
    (id: "t_fear", kind: Skill, cost: 0, target: Enemy, effects: [Apply(Fear, N(50), Target)]),
    (id: "t_stun", kind: Skill, cost: 0, target: Enemy, effects: [Apply(Stun, N(1), Target)]),
    (id: "t_thorns", kind: Skill, cost: 0, effects: [Apply(Thorns, N(3), Me)]),
    (id: "t_mark", kind: Skill, cost: 0, target: Enemy, effects: [Apply(HuntMark, N(4), Target)]),
    (id: "t_draw", kind: Skill, cost: 0, effects: [Draw(N(2))]),
    (id: "t_draw9", kind: Skill, cost: 0, effects: [Draw(N(9))]),
    (id: "t_energy", kind: Skill, cost: 0, target: Ally, effects: [Energy(N(2), Target)]),
    (id: "t_heal", kind: Skill, cost: 0, effects: [Heal(N(8), Me)]),
    (id: "t_pain", kind: Skill, cost: 0, effects: [LoseHp(N(4), Me)]),
    (id: "t_suicide", kind: Skill, cost: 0, effects: [LoseHp(N(999), Me)]),
    (id: "t_revive", kind: Skill, cost: 0, target: Downed, effects: [Revive(50, Target)]),
    (id: "t_gift", kind: Skill, cost: 0, effects: [AddCard(card: "t_hit", pile: Hand, to: Me, count: 2)]),
    (id: "t_exhaust", kind: Skill, cost: 0, keywords: [Exhaust], effects: [Block(N(1), Me)]),
    (id: "t_retain", kind: Skill, cost: 3, keywords: [Retain], effects: [Block(N(1), Me)]),
    (id: "t_ether", kind: Skill, cost: 3, keywords: [Ethereal], effects: [Block(N(1), Me)]),
    (id: "t_link", kind: Attack, cost: 0, target: Enemy,
        effects: [Damage(N(5), Target), If(Link(Skill), [Damage(N(5), Target)])]),
    (id: "t_consume", kind: Attack, cost: 0, target: Enemy,
        effects: [Consume(Rot, Target), Damage(Consumed(3), Target)]),
    (id: "t_cleanse", kind: Skill, cost: 0, effects: [Cleanse(Me)]),
    (id: "t_x", kind: Attack, cost: 0, x_cost: true, target: Enemy, effects: [Repeat(X(1), [Damage(N(3), Target)])]),
    (id: "t_fatal", kind: Attack, cost: 0, target: Enemy,
        effects: [Damage(N(10), Target), If(Fatal, [Heal(N(8), Me)])]),
    (id: "t_coop", kind: Skill, cost: 0, effects: [If(Coop, [Block(N(9), Me)], [Block(N(2), Me)])]),
    (id: "t_rune_thunder", kind: Skill, cost: 0, effects: [Rune(Thunder, Me)]),
    (id: "t_rune_frost", kind: Skill, cost: 0, target: Ally, effects: [Rune(Frost, Target)]),
    (id: "t_rune_mist", kind: Skill, cost: 0, effects: [Rune(Mist, Me)]),
    (id: "t_rune_flame", kind: Skill, cost: 0, effects: [Rune(Flame, Me)]),
    (id: "t_rune_zhiva", kind: Skill, cost: 0, effects: [Rune(Zhiva, Me)]),
    (id: "t_invoke", kind: Skill, cost: 0, effects: [Invoke]),
    (id: "t_runes_dmg", kind: Attack, cost: 0, target: Enemy, effects: [Damage(Runes(Thunder, 5), Target)]),
    (id: "t_shift", kind: Skill, cost: 0, effects: [Shift]),
    (id: "t_claw", kind: Attack, cost: 0, target: Enemy, effects: [Damage(N(6), Target)],
        beast: (effects: [Damage(N(9), Target), Apply(Bleed, N(2), Target)])),
    (id: "t_bear", kind: Skill, cost: 0, effects: [Mask(Bear)]),
    (id: "t_goat", kind: Skill, cost: 0, effects: [Mask(Goat)]),
    (id: "t_mara", kind: Skill, cost: 0, effects: [Mask(Mara)]),
    (id: "t_played", kind: Attack, cost: 0, target: Enemy, effects: [Damage(PlayedThisRound(3), Target)]),
    (id: "t_power", kind: Power, cost: 0, passives: [(on: TurnStart, effects: [Apply(Strength, N(1), Me)])]),
    (id: "t_toss", kind: Skill, cost: 0, effects: [DiscardHand, Damage(Discarded(2), RandomFoe)]),
    (id: "t_toss1", kind: Skill, cost: 0, effects: [Discard(N(1))]),
    (id: "t_toss_pick", kind: Skill, cost: 0, effects: [Discard(N(2)), Damage(Discarded(3), AllFoes)]),
    (id: "t_sly", kind: Skill, cost: 3, effects: [Block(N(1), Me)], on_discard: [Damage(N(7), AllFoes)]),
    (id: "t_on_discard", kind: Power, cost: 0, passives: [(on: Discarded, effects: [Block(N(2), Me)])]),
    (id: "t_on_exhaust", kind: Power, cost: 0, passives: [(on: Exhausted, effects: [Block(N(3), Me)])]),
    (id: "t_burn", kind: Skill, cost: 0, effects: [ExhaustCards(N(2))]),
    (id: "t_recall", kind: Skill, cost: 0, effects: [Recall(N(1))]),
    (id: "t_up", kind: Attack, cost: 1, target: Enemy, effects: [Damage(N(6), Target)], up: (cost: 0, effects: [Damage(N(9), Target)])),
]"#;

pub const ENEMIES: &str = r#"[
    (id: "dummy", hp: (100, 100), moves: [(id: "wait", intent: Defend, effects: [])], pattern: [0]),
    (id: "weakling", hp: (10, 10), moves: [(id: "wait", intent: Defend, effects: [])], pattern: [0]),
    (id: "bones", hp: (10, 10), flags: ["rises"], moves: [(id: "hit", intent: Attack, targeted: true, effects: [Damage(N(10), Target)]), (id: "reassemble", intent: Buff, effects: [])], pattern: [0]),
    (id: "hitter", hp: (100, 100), moves: [(id: "hit", intent: Attack, targeted: true, effects: [Damage(N(10), Target)])], pattern: [0]),
    (id: "brute", hp: (100, 100), moves: [(id: "hit", intent: Attack, targeted: true, effects: [Damage(N(60), Target)])], pattern: [0]),
    (id: "armored", hp: (100, 100), moves: [(id: "wall", intent: Defend, effects: [Block(N(20), Me)])], pattern: [0]),
    (id: "summoner", hp: (100, 100), moves: [(id: "call", intent: Summon, effects: [Summon("weakling", 2)])], pattern: [0]),
    (id: "thief", hp: (18, 18), moves: [
        (id: "steal", intent: Debuff, targeted: true, effects: [StealGold(15)]),
        (id: "run", intent: Unknown, effects: [Flee]),
    ], pattern: [0, 1]),
    (id: "opener", hp: (100, 100), moves: [
        (id: "a", intent: Buff, effects: [Apply(Strength, N(1), Me)]),
        (id: "b", intent: Defend, effects: []),
    ], opening: [0], pattern: [1]),
    (id: "spore", hp: (10, 10), moves: [(id: "wait", intent: Defend, effects: [])], pattern: [0],
        on_death: [Apply(Rot, N(3), AllFoes)]),
]"#;

pub const HEROES: &str = r#"[
    (id: "t_moon", max_hp: 50, mechanic: Moon, deck: [], relic: "father_amulet"),
    (id: "t_plain", max_hp: 50, mechanic: Curse, deck: [], relic: "father_amulet"),
]"#;

pub fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data")
}

pub fn content() -> Content {
    let mut c = Content::load_dir(&data_dir()).expect("content loads");
    c.add_cards(CARDS).unwrap();
    c.add_enemies(ENEMIES).unwrap();
    c.add_heroes(HEROES).unwrap();
    c.validate().unwrap();
    c
}

pub fn hero(def: &str, deck: &[&str]) -> HeroSetup {
    HeroSetup {
        def: def.into(),
        hp: 50,
        max_hp: 50,
        deck: deck.iter().map(|d| (d.to_string(), false)).collect(),
        relics: vec![],
        falls: 0,
        potions: vec![],
        lost_limbs: 0,
        lost_arms: 0,
        beheaded: false,
        walks_headless: false,
    }
}

pub fn ids(v: &[&str]) -> Vec<Id> {
    v.iter().map(|s| s.to_string()).collect()
}

/// One plain hero with the given deck against the given enemies.
pub fn solo(deck: &[&str], enemies: &[&str]) -> (Content, Combat) {
    let c = content();
    let (s, _) = start(&c, &[hero("t_plain", deck)], &ids(enemies), 7);
    (c, s)
}

pub fn duo(a: &[&str], b: &[&str], enemies: &[&str]) -> (Content, Combat) {
    let c = content();
    let (s, _) = start(&c, &[hero("t_plain", a), hero("t_plain", b)], &ids(enemies), 7);
    (c, s)
}

pub fn foe(s: &Combat, i: usize) -> UnitRef {
    UnitRef::Enemy(s.enemies[i].uid)
}

/// Plays the first card with this id from the player's hand.
pub fn play(c: &Content, s: &mut Combat, p: u8, card: &str, target: Option<UnitRef>) -> Vec<Event> {
    try_play(c, s, p, card, target).unwrap_or_else(|e| panic!("play {card}: {e:?}"))
}

pub fn try_play(c: &Content, s: &mut Combat, p: u8, card: &str, target: Option<UnitRef>) -> Result<Vec<Event>, ActionError> {
    let uid = s.heroes[p as usize]
        .hand
        .iter()
        .find(|x| x.def == card)
        .unwrap_or_else(|| panic!("{card} not in hand: {:?}", s.heroes[p as usize].hand))
        .uid;
    apply(c, s, p, &CombatAction::Play { card: uid, target })
}

pub fn ready(c: &Content, s: &mut Combat, p: u8) -> Vec<Event> {
    apply(c, s, p, &CombatAction::Ready(true)).unwrap()
}

/// Everyone still standing presses Ready, closing the round.
pub fn end_round(c: &Content, s: &mut Combat) {
    let round = s.round;
    for p in 0..s.heroes.len() as u8 {
        if s.phase == Phase::Heroes && s.round == round && !s.heroes[p as usize].downed {
            ready(c, s, p);
        }
    }
}
