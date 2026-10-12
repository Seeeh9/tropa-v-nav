use nav_core::*;
use nav_net::*;
use std::path::PathBuf;

const EXTRA_HEROES: &str = r#"[
    (id: "h2", max_hp: 70, mechanic: Curse, relic: "father_amulet", deck: ["bogatyr_strike", "bogatyr_strike", "bogatyr_strike", "bogatyr_defend", "bogatyr_defend", "bogatyr_shield_wall"]),
    (id: "h3", max_hp: 72, mechanic: Runes, relic: "father_amulet", deck: ["bogatyr_strike", "bogatyr_strike", "bogatyr_strike", "bogatyr_defend", "bogatyr_defend", "bogatyr_sweep"]),
    (id: "h4", max_hp: 80, mechanic: Moon, relic: "father_amulet", deck: ["bogatyr_strike", "bogatyr_strike", "bogatyr_strike", "bogatyr_defend", "bogatyr_defend", "bogatyr_guard"]),
]"#;
const HEROES: [&str; 4] = ["bogatyr", "h2", "h3", "h4"];

fn content() -> Content {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/data");
    let mut c = Content::load_dir(&dir).unwrap();
    c.add_heroes(EXTRA_HEROES).unwrap();
    c
}

fn ids(v: &[&str]) -> Vec<Id> {
    v.iter().map(|s| s.to_string()).collect()
}

/// A started fight with `n` players; peer ids are 1..=n and peer 1 hosts.
fn party(c: &Content, n: usize, enemies: &[&str], seed: u64) -> Hub {
    let mut hub = Hub::default();
    hub.nodes.push(Node::host(1, "host"));
    for p in 2..=n as u64 {
        hub.nodes.push(Node::join(p, &format!("p{p}"), 1));
    }
    hub.pump(c);
    for (i, node) in hub.nodes.iter_mut().enumerate() {
        node.pick_hero(Some(HEROES[i].into()));
    }
    hub.pump(c);
    for node in &mut hub.nodes {
        node.set_ready(true);
    }
    hub.pump(c);
    assert!(hub.node(1).start(c, seed, ids(enemies)), "lobby should be startable");
    hub.pump(c);
    hub
}

fn hashes(hub: &Hub) -> Vec<u64> {
    hub.nodes.iter().filter(|n| !hub.offline.contains(&n.me)).map(|n| n.state.as_ref().unwrap().hash64()).collect()
}

fn assert_in_sync(hub: &Hub) {
    let h = hashes(hub);
    assert!(h.windows(2).all(|w| w[0] == w[1]), "nodes diverged: {h:x?}");
}

/// Every connected player's own machine picks a move with the autopilot logic and submits it.
fn play_out(c: &Content, hub: &mut Hub, limit: usize) -> Phase {
    for _ in 0..limit {
        let phase = hub.nodes.iter().find(|n| !hub.offline.contains(&n.me)).unwrap().state.as_ref().unwrap().phase;
        if phase != Phase::Heroes {
            return phase;
        }
        for i in 0..hub.nodes.len() {
            let node = &mut hub.nodes[i];
            if hub.offline.contains(&node.me) {
                continue;
            }
            let (Some(slot), Some(state)) = (node.my_slot(), node.state.as_ref()) else { continue };
            let Some(h) = state.hero(slot) else { continue };
            if state.phase != Phase::Heroes || h.downed || h.ready {
                continue;
            }
            let action = bot::choose(c, state, slot);
            node.submit(c, action);
            hub.pump(c);
            assert_in_sync(hub);
        }
        // the host steers anyone who dropped
        while hub.nodes.iter_mut().find(|n| n.is_host() && !hub.offline.contains(&n.me)).is_some_and(|h| h.autopilot(c)) {
            hub.pump(c);
            assert_in_sync(hub);
        }
    }
    panic!("fight did not end in {limit} rounds of moves");
}

#[test]
fn lobby_fills_and_heroes_are_unique() {
    let c = content();
    let mut hub = Hub::default();
    hub.nodes.push(Node::host(1, "host"));
    for p in 2..=5 {
        hub.nodes.push(Node::join(p, "x", 1));
    }
    hub.pump(&c);
    assert_eq!(hub.node(1).lobby.players.len(), 4, "the fifth player doesn't fit");
    let host_view = hub.node(1).lobby.clone();
    assert_eq!(hub.node(3).lobby, host_view);
    // the hub broadcasts to every node, so peer 5 hears the lobby; what matters is it has no seat
    assert_eq!(hub.node(1).lobby.slot_of(5), None);
    assert_eq!(hub.node(5).my_slot(), None);

    hub.node(1).pick_hero(Some("bogatyr".into()));
    hub.node(2).pick_hero(Some("bogatyr".into()));
    hub.pump(&c);
    let lobby = hub.node(4).lobby.clone();
    assert_eq!(lobby.players[0].hero.as_deref(), Some("bogatyr"));
    assert_eq!(lobby.players[1].hero, None, "a taken hero can't be picked twice");

    hub.node(2).set_ready(true);
    hub.pump(&c);
    assert!(!hub.node(1).lobby.players[1].ready, "no ready without a hero");
    assert!(!hub.node(1).start(&c, 1, ids(&["grey_wolf"])), "can't start until everyone is ready");
    assert!(!hub.node(2).start(&c, 1, ids(&["grey_wolf"])), "only the host starts");
}

#[test]
fn four_players_stay_in_sync_to_the_end() {
    let c = content();
    for seed in [3, 4, 5] {
        let mut hub = party(&c, 4, &["kikimora", "grey_wolf", "grey_wolf", "grey_wolf"], seed);
        assert_in_sync(&hub);
        assert_eq!(hub.node(2).state.as_ref().unwrap().heroes.len(), 4);
        assert_eq!(hub.node(3).my_slot(), Some(2));
        let end = play_out(&c, &mut hub, 400);
        assert_ne!(end, Phase::Heroes);
        assert_in_sync(&hub);
        assert!(hub.nodes.iter().all(|n| n.n == hub.nodes[0].n));
    }
}

#[test]
fn illegal_action_is_refused_and_changes_nothing() {
    let c = content();
    let mut hub = party(&c, 2, &["kikimora"], 9);
    let before = hashes(&hub);
    hub.node(2).take_notes();
    hub.node(2).submit(&c, CombatAction::Play { card: 999_999, target: None });
    hub.pump(&c);
    assert_eq!(hashes(&hub), before);
    assert!(hub.node(2).take_notes().contains(&Note::Refused(ActionError::NoSuchCard)));
}

#[test]
fn a_cheating_client_cannot_act_for_someone_else() {
    let c = content();
    let mut hub = party(&c, 2, &["kikimora"], 9);
    // peer 2 forges a "host says" message; it must be ignored by peer 1 (the real host)
    let before = hashes(&hub);
    hub.node(1).handle(&c, 2, Msg::Did { n: 1, player: 0, action: CombatAction::Ready(true) });
    hub.pump(&c);
    assert_eq!(hashes(&hub), before);
}

#[test]
fn drifted_client_is_repaired_by_snapshot() {
    let c = content();
    let mut hub = party(&c, 2, &["kikimora"], 9);
    hub.node(2).state.as_mut().unwrap().heroes[0].hp -= 7;
    assert_ne!(hashes(&hub)[0], hashes(&hub)[1]);
    hub.node(2).take_notes();
    // closing the round makes the host publish a hash
    hub.node(1).submit(&c, CombatAction::Ready(true));
    hub.node(2).submit(&c, CombatAction::Ready(true));
    hub.pump(&c);
    assert_in_sync(&hub);
    assert!(hub.node(2).take_notes().contains(&Note::Resynced));
}

#[test]
fn dropped_player_is_steered_by_autopilot_and_can_return() {
    let c = content();
    let mut hub = party(&c, 3, &["kikimora", "grey_wolf"], 6);
    // peer 3 loses connection
    hub.offline.push(3);
    hub.node(1).peer_left(3);
    hub.node(2).peer_left(3);
    hub.pump(&c);
    assert!(!hub.node(2).lobby.players[2].connected);
    let round = hub.node(1).state.as_ref().unwrap().round;
    for _ in 0..40 {
        if hub.node(1).state.as_ref().unwrap().round > round + 1 {
            break;
        }
        for p in [1, 2] {
            let node = hub.node(p);
            let (slot, state) = (node.my_slot().unwrap(), node.state.as_ref().unwrap());
            if state.phase == Phase::Heroes && !state.heroes[slot as usize].ready && !state.heroes[slot as usize].downed {
                let a = bot::choose(&c, state, slot);
                node.submit(&c, a);
                hub.pump(&c);
            }
        }
        while hub.node(1).autopilot(&c) {
            hub.pump(&c);
        }
    }
    assert!(hub.node(1).state.as_ref().unwrap().round > round + 1, "rounds keep closing without the dropped player");

    // peer 3 comes back with a stale state and says hello again
    hub.offline.clear();
    let stale = hub.node(3).state.as_ref().unwrap().hash64();
    assert_ne!(stale, hub.node(1).state.as_ref().unwrap().hash64());
    let rejoin = Node::join(3, "p3", 1);
    *hub.node(3) = rejoin;
    hub.pump(&c);
    assert_in_sync(&hub);
    assert!(hub.node(1).lobby.players[2].connected);
    assert_eq!(hub.node(3).my_slot(), Some(2));
    assert!(!hub.node(1).autopilot(&c) || hub.node(1).state.as_ref().unwrap().heroes[2].ready, "autopilot lets go of a returned player");
    let end = play_out(&c, &mut hub, 400);
    assert_ne!(end, Phase::Heroes);
    assert_in_sync(&hub);
}

#[test]
fn fight_survives_the_host_leaving() {
    let c = content();
    let mut hub = party(&c, 3, &["kikimora", "grey_wolf"], 8);
    // a few moves under the first host
    for p in [2, 3] {
        let node = hub.node(p);
        let a = bot::choose(&c, node.state.as_ref().unwrap(), node.my_slot().unwrap());
        node.submit(&c, a);
        hub.pump(&c);
    }
    // peer 2 asks for something the host never gets to answer
    hub.offline.push(1);
    let node = hub.node(2);
    let a = bot::choose(&c, node.state.as_ref().unwrap(), node.my_slot().unwrap());
    let before = node.n;
    node.submit(&c, a);
    hub.pump(&c);
    assert_eq!(hub.node(2).n, before, "nothing confirms it while the host is gone");

    // Steam hands the lobby to peer 2
    for p in [2, 3] {
        hub.node(p).peer_left(1);
        hub.node(p).host_changed(2);
    }
    hub.pump(&c);
    assert!(hub.node(2).is_host());
    assert_in_sync(&hub);
    let end = play_out(&c, &mut hub, 400);
    assert_ne!(end, Phase::Heroes);
    assert_in_sync(&hub);
}

#[test]
fn messages_round_trip_through_bytes() {
    let c = content();
    let hub = party(&c, 2, &["kikimora"], 9);
    let state = hub.nodes[0].state.clone().unwrap();
    let msg = Msg::Snapshot { n: 5, lobby: hub.nodes[0].lobby.clone(), state: Box::new(state.clone()) };
    let Some(Msg::Snapshot { n, state: back, .. }) = Msg::decode(&msg.encode()) else { panic!("lost in transit") };
    assert_eq!(n, 5);
    assert_eq!(back.hash64(), state.hash64());
}
