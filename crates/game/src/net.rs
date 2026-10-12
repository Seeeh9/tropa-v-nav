//! Moves `nav_net` messages over Steam: lobbies for finding each other,
//! Networking Messages (relayed P2P) for the traffic. In the dev build the
//! same node can instead talk to in-process bot clients with no Steam at all.

use crate::session::{Db, Session};
use crate::steam::Steam;
use crate::AppState;
use bevy::prelude::*;
use nav_core::{bot, Phase};
use nav_net::{Msg, Node, Note, Peer, To};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use steamworks::networking_types::{NetworkingIdentity, SendFlags};
use steamworks::{ChatMemberStateChange, GameLobbyJoinRequested, LobbyChatUpdate, LobbyId, LobbyType, SteamId};

/// Lobby metadata that tells our lobbies apart from everyone else's on a shared app id.
const GAME_KEY: &str = "game";
const GAME_VALUE: &str = "tropa_v_nav";
const CODE_KEY: &str = "code";
const CHANNEL: u32 = 0;

pub fn plugin(app: &mut App) {
    app.init_resource::<NetStatus>()
        .add_systems(Startup, hook_callbacks.run_if(resource_exists::<Steam>))
        .add_systems(PreUpdate, (steam_events, receive).chain().run_if(resource_exists::<Steam>))
        .add_systems(Update, (local_peers, autopilot, notes).chain().run_if(resource_exists::<Net>))
        .add_systems(PostUpdate, send.run_if(resource_exists::<Net>));
}

/// The co-op session this machine is part of.
#[derive(Resource)]
pub struct Net {
    pub node: Node,
    /// Steam lobby, or `None` for an offline dev session.
    pub lobby: Option<LobbyId>,
    pub code: String,
    /// Extra clients living in this process (dev build), played by the autopilot.
    pub locals: Vec<Node>,
    timer: f32,
}

impl Net {
    pub fn me(&self) -> Peer {
        self.node.me
    }
}

/// What the lobby screens show while Steam is working on a request.
#[derive(Resource, Default)]
pub struct NetStatus {
    pub busy: bool,
    pub error: Option<String>,
    /// The lobby list changed; the lobby screen should redraw.
    pub dirty: bool,
}

enum SteamEv {
    Created(Result<LobbyId, String>),
    Found(Vec<LobbyId>),
    Joined(Result<LobbyId, String>),
    Member { lobby: LobbyId, user: SteamId, left: bool },
    Invited(LobbyId),
}

/// Results of Steam requests, filled from Steam's callbacks.
#[derive(Resource, Clone, Default)]
pub struct Inbox(Arc<Mutex<Vec<SteamEv>>>);

/// Steam ids allowed to open a message session with us: the people in our lobby.
#[derive(Resource, Clone, Default)]
struct Allowed(Arc<Mutex<HashSet<u64>>>);

impl Inbox {
    fn push(&self, e: SteamEv) {
        if let Ok(mut q) = self.0.lock() {
            q.push(e);
        }
    }
}

/// Six characters without look-alikes (no 0/O, 1/I).
pub fn new_code(seed: u64) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = nav_core::Rng::new(seed);
    (0..6).map(|_| ALPHABET[rng.below(ALPHABET.len() as u32) as usize] as char).collect()
}

fn hook_callbacks(mut commands: Commands, steam: Res<Steam>) {
    let inbox = Inbox::default();
    let allowed = Allowed::default();
    let c = &steam.client;

    let q = inbox.clone();
    std::mem::forget(c.register_callback(move |u: LobbyChatUpdate| {
        let left = !matches!(u.member_state_change, ChatMemberStateChange::Entered);
        q.push(SteamEv::Member { lobby: u.lobby, user: u.user_changed, left });
    }));
    // accepting an invite, or "Join game" on a friend, from the overlay
    let q = inbox.clone();
    std::mem::forget(c.register_callback(move |r: GameLobbyJoinRequested| q.push(SteamEv::Invited(r.lobby_steam_id))));

    let ok = allowed.clone();
    c.networking_messages().session_request_callback(move |request| {
        let known = request.remote().steam_id().is_some_and(|id| ok.0.lock().is_ok_and(|set| set.contains(&id.raw())));
        if known {
            request.accept();
        } else {
            request.reject();
        }
    });

    // launched from an invite while the game was closed: `+connect_lobby <id>`
    let args: Vec<String> = std::env::args().collect();
    if let Some(id) = args.iter().position(|a| a == "+connect_lobby").and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<u64>().ok()) {
        inbox.push(SteamEv::Invited(LobbyId::from_raw(id)));
    }
    commands.insert_resource(inbox);
    commands.insert_resource(allowed);
}

// ---- requests from the UI ----

pub fn create_lobby(steam: &Steam, status: &mut NetStatus, inbox: &Inbox) {
    status.busy = true;
    status.error = None;
    let q = inbox.clone();
    steam.client.matchmaking().create_lobby(LobbyType::Public, nav_net::MAX_PLAYERS as u32, move |r| {
        q.push(SteamEv::Created(r.map_err(|e| e.to_string())));
    });
}

pub fn join_by_code(steam: &Steam, status: &mut NetStatus, inbox: &Inbox, code: &str) {
    use steamworks::{LobbyKey, LobbyListFilter, StringFilter, StringFilterKind};
    status.busy = true;
    status.error = None;
    let code = code.trim().to_uppercase();
    let mm = steam.client.matchmaking();
    mm.set_lobby_list_filter(LobbyListFilter {
        string: Some(vec![
            StringFilter(LobbyKey::new(GAME_KEY), GAME_VALUE, StringFilterKind::Equal),
            StringFilter(LobbyKey::new(CODE_KEY), &code, StringFilterKind::Equal),
        ]),
        ..Default::default()
    });
    let q = inbox.clone();
    mm.request_lobby_list(move |r| q.push(SteamEv::Found(r.unwrap_or_default())));
}

pub fn leave(commands: &mut Commands, steam: Option<&Steam>, net: &Net) {
    if let (Some(steam), Some(lobby)) = (steam, net.lobby) {
        steam.client.matchmaking().leave_lobby(lobby);
        steam.client.friends().set_rich_presence("status", None);
    }
    commands.remove_resource::<Net>();
}

/// Opens the Steam overlay's friend picker for this lobby.
pub fn invite(steam: &Steam, net: &Net) {
    if let Some(lobby) = net.lobby {
        steam.client.friends().activate_invite_dialog(lobby);
    }
}

/// A Steam-free session with bot clients in the same process (dev build only).
#[cfg(feature = "dev")]
pub fn start_local(commands: &mut Commands, db: &nav_core::Content, bots: usize) {
    let mut node = Node::host(1, "dev");
    let mut locals: Vec<Node> = (0..bots).map(|i| Node::join(2 + i as u64, &format!("bot {}", i + 1), 1)).collect();
    // seat everyone and give each bot a free hero
    for _ in 0..4 {
        route(db, &mut node, &mut locals);
    }
    let heroes: Vec<String> = db.heroes.keys().cloned().collect();
    for (i, bot) in locals.iter_mut().enumerate() {
        if let Some(h) = heroes.get(i + 1) {
            bot.pick_hero(Some(h.clone()));
        }
    }
    for _ in 0..4 {
        route(db, &mut node, &mut locals);
    }
    for bot in &mut locals {
        bot.set_ready(true);
    }
    route(db, &mut node, &mut locals);
    commands.insert_resource(Net { node, lobby: None, code: "LOCAL".into(), locals, timer: 0.0 });
}

// ---- systems ----

fn steam_events(
    mut commands: Commands,
    steam: Res<Steam>,
    inbox: Res<Inbox>,
    allowed: Res<Allowed>,
    mut net: Option<ResMut<Net>>,
    mut status: ResMut<NetStatus>,
    mut next: ResMut<NextState<AppState>>,
    time: Res<Time>,
) {
    let events: Vec<SteamEv> = inbox.0.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default();
    let mm = steam.client.matchmaking();
    let me = steam.client.user().steam_id();
    let name = steam.client.friends().name();
    for e in events {
        match e {
            SteamEv::Created(Ok(lobby)) => {
                let code = new_code(lobby.raw() ^ time.elapsed().as_nanos() as u64);
                mm.set_lobby_data(lobby, GAME_KEY, GAME_VALUE);
                mm.set_lobby_data(lobby, CODE_KEY, &code);
                commands.insert_resource(Net { node: Node::host(me.raw(), &name), lobby: Some(lobby), code, locals: vec![], timer: 0.0 });
                status.busy = false;
                status.dirty = true;
                next.set(AppState::Lobby);
            }
            SteamEv::Found(list) => match list.first() {
                Some(lobby) => {
                    let q = inbox.clone();
                    mm.join_lobby(*lobby, move |r| q.push(SteamEv::Joined(r.map_err(|_| "join failed".to_string()))));
                }
                None => {
                    status.busy = false;
                    status.error = Some("coop-error-not-found".into());
                }
            },
            SteamEv::Invited(lobby) => {
                if net.is_none() {
                    status.busy = true;
                    let q = inbox.clone();
                    mm.join_lobby(lobby, move |r| q.push(SteamEv::Joined(r.map_err(|_| "join failed".to_string()))));
                }
            }
            SteamEv::Joined(Ok(lobby)) => {
                // someone else's Spacewar lobby would not carry our tag
                if mm.lobby_data(lobby, GAME_KEY).as_deref() != Some(GAME_VALUE) {
                    mm.leave_lobby(lobby);
                    status.busy = false;
                    status.error = Some("coop-error-not-found".into());
                    continue;
                }
                let host = mm.lobby_owner(lobby);
                let code = mm.lobby_data(lobby, CODE_KEY).unwrap_or_default();
                // the host must be allowed before our hello reaches them and they answer
                if let Ok(mut set) = allowed.0.lock() {
                    set.extend(mm.lobby_members(lobby).iter().map(|m| m.raw()));
                }
                commands.insert_resource(Net { node: Node::join(me.raw(), &name, host.raw()), lobby: Some(lobby), code, locals: vec![], timer: 0.0 });
                status.busy = false;
                status.dirty = true;
                next.set(AppState::Lobby);
            }
            SteamEv::Created(Err(e)) | SteamEv::Joined(Err(e)) => {
                warn!("lobby request failed: {e}");
                status.busy = false;
                status.error = Some("coop-error-steam".into());
            }
            SteamEv::Member { lobby, user, left } => {
                let Some(net) = net.as_mut().filter(|n| n.lobby == Some(lobby)) else { continue };
                if left {
                    net.node.peer_left(user.raw());
                }
                // Steam picks the new owner itself when the host goes
                net.node.host_changed(mm.lobby_owner(lobby).raw());
                status.dirty = true;
            }
        }
    }
    if let Some(net) = &net {
        if let Some(lobby) = net.lobby {
            if let Ok(mut set) = allowed.0.lock() {
                *set = mm.lobby_members(lobby).iter().map(|m| m.raw()).collect();
            }
        }
    }
}

fn receive(steam: Res<Steam>, db: Res<Db>, mut net: Option<ResMut<Net>>) {
    let messages = steam.client.networking_messages().receive_messages_on_channel(CHANNEL, 64);
    let Some(net) = net.as_mut() else { return };
    for m in messages {
        let Some(from) = m.identity_peer().steam_id() else { continue };
        if let Some(msg) = Msg::decode(m.data()) {
            net.node.handle(&db.0, from.raw(), msg);
        }
    }
}

fn send(steam: Option<Res<Steam>>, db: Res<Db>, mut net: ResMut<Net>) {
    let net = &mut *net;
    let Some(lobby) = net.lobby else {
        route(&db.0, &mut net.node, &mut net.locals);
        return;
    };
    let Some(steam) = steam else { return };
    let out = net.node.take_out();
    if out.is_empty() {
        return;
    }
    let me = net.node.me;
    let members: Vec<SteamId> = steam.client.matchmaking().lobby_members(lobby).into_iter().filter(|m| m.raw() != me).collect();
    let nm = steam.client.networking_messages();
    for (to, msg) in out {
        let bytes = msg.encode();
        let targets: Vec<SteamId> = match to {
            To::One(p) => vec![SteamId::from_raw(p)],
            To::All => members.clone(),
        };
        for t in targets {
            // reliable and unbatched: a turn-based game cares about order, not throughput
            if let Err(e) = nm.send_message_to_user(NetworkingIdentity::new_steam_id(t), SendFlags::RELIABLE_NO_NAGLE | SendFlags::AUTO_RESTART_BROKEN_SESSION, &bytes, CHANNEL) {
                warn!("send to {} failed: {e:?}", t.raw());
            }
        }
    }
}

/// Delivers messages between the node and its in-process peers until all are quiet.
fn route(db: &nav_core::Content, node: &mut Node, locals: &mut [Node]) {
    for _ in 0..64 {
        let mut mail: Vec<(Peer, To, Msg)> = node.take_out().into_iter().map(|(to, m)| (node.me, to, m)).collect();
        for l in locals.iter_mut() {
            let from = l.me;
            mail.extend(l.take_out().into_iter().map(|(to, m)| (from, to, m)));
        }
        if mail.is_empty() {
            return;
        }
        for (from, to, msg) in mail {
            let deliver = |n: &mut Node| {
                let wanted = match to {
                    To::One(p) => p == n.me,
                    To::All => from != n.me,
                };
                if wanted {
                    if let Some(m) = Msg::decode(&msg.encode()) {
                        n.handle(db, from, m);
                    }
                }
            };
            deliver(node);
            for l in locals.iter_mut() {
                deliver(l);
            }
        }
    }
}

/// In-process clients take their turns with the autopilot, at a watchable pace.
fn local_peers(db: Res<Db>, time: Res<Time>, mut net: ResMut<Net>, session: Option<Res<Session>>) {
    if net.locals.is_empty() || session.is_some_and(|s| !s.idle()) {
        return;
    }
    net.timer -= time.delta_secs();
    if net.timer > 0.0 {
        return;
    }
    net.timer = 0.5;
    for l in net.locals.iter_mut() {
        let (Some(slot), Some(state)) = (l.my_slot(), l.state.as_ref()) else { continue };
        let Some(h) = state.hero(slot) else { continue };
        if state.phase == Phase::Heroes && !h.downed && !h.ready {
            let action = bot::choose(&db.0, state, slot);
            l.submit(&db.0, action);
            return;
        }
    }
}

fn autopilot(db: Res<Db>, time: Res<Time>, mut net: ResMut<Net>, mut wait: Local<f32>) {
    *wait -= time.delta_secs();
    if *wait <= 0.0 {
        *wait = 0.6;
        net.node.autopilot(&db.0);
    }
}

/// Turns protocol notes into what the screens need.
fn notes(
    mut commands: Commands,
    mut net: ResMut<Net>,
    mut session: Option<ResMut<Session>>,
    mut status: ResMut<NetStatus>,
    mut next: ResMut<NextState<AppState>>,
    l10n: Res<crate::l10n::L10n>,
) {
    for note in net.node.take_notes() {
        match note {
            Note::LobbyChanged | Note::HostChanged => status.dirty = true,
            Note::Started => {
                if let (Some(state), Some(me)) = (net.node.state.clone(), net.node.my_slot()) {
                    commands.insert_resource(Session::from_state(state, me));
                    next.set(AppState::Battle);
                }
            }
            Note::Applied { events, .. } => {
                if let (Some(s), Some(state)) = (session.as_mut(), net.node.state.as_ref()) {
                    s.truth = state.clone();
                    s.queue.extend(events);
                }
            }
            Note::Resynced => {
                if let Some(state) = net.node.state.clone() {
                    match (session.as_mut(), net.node.my_slot()) {
                        (Some(s), _) => {
                            s.truth = state.clone();
                            s.shown = state;
                            s.queue.clear();
                        }
                        // rejoined mid-fight: straight to the battle screen
                        (None, Some(me)) => {
                            commands.insert_resource(Session::from_state(state, me));
                            next.set(AppState::Battle);
                        }
                        _ => {}
                    }
                }
            }
            Note::Refused(e) => {
                if let Some(s) = session.as_mut() {
                    s.error = Some((l10n.t(&format!("error-{e:?}")), 1.6));
                }
            }
            Note::Ping { .. } => {}
        }
    }
}
