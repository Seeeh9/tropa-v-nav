//! A co-op transport over TCP, for friends on one local network or a VPN like Radmin, without
//! Steam. The protocol node does not care how its messages travel; here they go as frames over
//! sockets, the host in the middle passing on what is meant for others, as Steam would deliver it.
//!
//! The party screen starts it. In a dev build it can also be started from the environment:
//!
//!   TROPA_LAN=host:<port>          host a party on that port
//!   TROPA_LAN=join:<addr>:<port>   join it
//!   TROPA_SEED=<n>                 (host) a fixed seed for the run
//!   TROPA_LAN_HERO=<id>            take this hero and get ready at once; the host starts the run
//!                                  as soon as everyone is
//!   TROPA_FIGHT=<id>,<id>          the run opens on this fight (give every copy the same, with the
//!                                  same TROPA_HERO_HP and the like)
//!
//! A frame is its length (u32), the sender (u64), the addressee (u64, 0 for everyone) and the
//! message as `Msg::encode` writes it, all big-endian.

use crate::net::{Net, NetStatus};
use crate::session::Db;
use crate::AppState;
use bevy::prelude::*;
use nav_net::{Msg, Node, Peer, To};
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The port a party is gathered on; an address typed without one means this.
pub const PORT: u16 = 47771;
/// No message in the game comes near this: a longer frame is someone else's traffic.
const MOST_FRAME: usize = 64 * 1024 * 1024;
/// How long a peer may keep a send waiting before it counts as gone, and how long a knock takes.
const SEND_WAIT: Duration = Duration::from_secs(3);
const DIAL_WAIT: Duration = Duration::from_secs(6);

const HOST: Peer = 1;
const EVERYONE: Peer = 0;

pub fn plugin(app: &mut App) {
    app.add_systems(PreUpdate, receive.run_if(resource_exists::<Lan>))
        .add_systems(Update, (answered.run_if(resource_exists::<Dial>), hang_up.run_if(resource_exists::<Lan>), redial.run_if(resource_exists::<Lan>)))
        .add_systems(PostUpdate, transmit.run_if(resource_exists::<Lan>).before(crate::net::send));
    #[cfg(feature = "dev")]
    app.add_systems(OnEnter(AppState::Menu), open).add_systems(Update, (auto_lobby, open_on_fight, drop_line).chain().run_if(resource_exists::<Lan>));
}

/// A knock at a host's door, waited for off to the side so the game does not freeze on it.
#[derive(Resource)]
struct Dial(Mutex<Receiver<std::io::Result<TcpStream>>>, String);

fn net_of(node: Node, code: String, ephemeral: bool) -> Net {
    Net { node, lobby: None, code, locals: vec![], timer: 0.0, bot_me: false, booked: false, ephemeral }
}

/// Gathers a party on this machine, open on every network it is on. False if it cannot be done.
pub fn host(commands: &mut Commands, status: &mut NetStatus) -> bool {
    let listener = match TcpListener::bind(("0.0.0.0", PORT)) {
        Ok(l) => l,
        Err(e) => {
            warn!("LAN: cannot host on {PORT}: {e}");
            status.error = Some("coop-error-lan-port".into());
            return false;
        }
    };
    let _ = listener.set_nonblocking(true);
    let code = my_addresses().join("  /  ");
    info!("LAN: hosting on {PORT}, at {code}");
    // a saved party is picked up as the lobby opens; its players come back to their seats
    let node = match status.resume.take() {
        Some(save) => Node::resume_party(HOST, "host", save.lobby, save.n, save.run),
        None => Node::host(HOST, "host"),
    };
    commands.insert_resource(net_of(node, code, false));
    commands.insert_resource(Lan { listener: Some(listener), links: vec![], me: HOST, addr: None, redial: None });
    status.error = None;
    status.dirty = true;
    true
}

/// Knocks at the door of the host at this address.
pub fn dial(commands: &mut Commands, typed: &str, status: &mut NetStatus) {
    let addr = if typed.contains(':') { typed.to_string() } else { format!("{typed}:{PORT}") };
    let (tx, rx) = channel();
    info!("LAN: calling {addr}");
    let kept = addr.clone();
    std::thread::spawn(move || {
        let reached = addr.to_lowercase().to_socket_addrs().and_then(|mut found| found.next().ok_or_else(|| ErrorKind::NotFound.into())).and_then(|at| TcpStream::connect_timeout(&at, DIAL_WAIT));
        let _ = tx.send(reached);
    });
    commands.insert_resource(Dial(Mutex::new(rx), kept));
    status.busy = true;
    status.error = None;
}

fn answered(mut commands: Commands, dial: Res<Dial>, state: Res<State<AppState>>, mut status: ResMut<NetStatus>, mut next: ResMut<NextState<AppState>>) {
    let Ok(rx) = dial.0.lock() else { return };
    let Ok(reached) = rx.try_recv() else { return };
    let addr = dial.1.clone();
    commands.remove_resource::<Dial>();
    status.busy = false;
    // the player has since walked away from the screen
    if *state.get() != AppState::Coop {
        return;
    }
    let stream = match reached {
        Ok(s) => s,
        Err(e) => {
            warn!("LAN: no answer: {e}");
            status.error = Some("coop-error-lan".into());
            return;
        }
    };
    let _ = stream.set_nonblocking(true);
    let _ = stream.set_nodelay(true);
    let me = fresh_peer();
    info!("LAN: joined as {me}");
    commands.insert_resource(net_of(Node::join(me, &format!("guest{}", me % 100), HOST), "LAN".into(), false));
    commands.insert_resource(Lan { listener: None, links: vec![Link { stream, peer: Some(HOST), buf: vec![] }], me, addr: Some(addr), redial: None });
    status.dirty = true;
    next.set(AppState::Lobby);
}

/// A name no other guest will have, whichever machine they are on.
fn fresh_peer() -> Peer {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos() as u64);
    1000 + (nanos ^ (std::process::id() as u64) << 20)
}

/// The party is over: the sockets close.
fn hang_up(mut commands: Commands, net: Option<Res<Net>>) {
    if net.is_none() {
        commands.remove_resource::<Lan>();
    }
}

/// This machine's addresses friends can reach it at, a VPN's first since that is why it is there.
fn my_addresses() -> Vec<String> {
    let mut found = Vec::new();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const NO_WINDOW: u32 = 0x0800_0000;
        if let Ok(out) = std::process::Command::new("ipconfig").creation_flags(NO_WINDOW).output() {
            for line in String::from_utf8_lossy(&out.stdout).lines().filter(|l| l.contains("IPv4")) {
                if let Some(ip) = line.rsplit(':').next().map(str::trim).filter(|ip| ip.parse::<std::net::Ipv4Addr>().is_ok()) {
                    found.push(ip.to_string());
                }
            }
        }
    }
    // Radmin hands out 26.*, Hamachi 25.*
    found.sort_by_key(|ip: &String| match ip.split('.').next() {
        Some("26") => 0,
        Some("25") => 1,
        _ => 2,
    });
    found.truncate(3);
    if found.is_empty() {
        found.push("127.0.0.1".into());
    }
    found
}

#[derive(Resource)]
struct Lan {
    listener: Option<TcpListener>,
    links: Vec<Link>,
    me: Peer,
    /// A guest: where the host was reached, to call it again if the line drops.
    addr: Option<String>,
    /// A call back to the host under way.
    redial: Option<Mutex<Receiver<std::io::Result<TcpStream>>>>,
}

struct Link {
    stream: TcpStream,
    /// Who is at the other end, once they have said anything.
    peer: Option<Peer>,
    buf: Vec<u8>,
}

fn frame(from: Peer, to: Peer, msg: &[u8]) -> Vec<u8> {
    let mut f = Vec::with_capacity(20 + msg.len());
    f.extend_from_slice(&((16 + msg.len()) as u32).to_be_bytes());
    f.extend_from_slice(&from.to_be_bytes());
    f.extend_from_slice(&to.to_be_bytes());
    f.extend_from_slice(msg);
    f
}

/// Writes all of it, waiting out a full send buffer. A peer that will not take it in time is cut
/// off: half a frame sent can never be taken back, and the socket closing tells `receive`.
fn write_all(stream: &mut TcpStream, mut bytes: &[u8]) -> std::io::Result<()> {
    let until = Instant::now() + SEND_WAIT;
    while !bytes.is_empty() {
        match stream.write(bytes) {
            Ok(0) => return Err(ErrorKind::WriteZero.into()),
            Ok(n) => bytes = &bytes[n..],
            Err(e) if e.kind() == ErrorKind::WouldBlock && Instant::now() < until => std::thread::sleep(Duration::from_millis(1)),
            Err(e) => {
                let _ = stream.shutdown(Shutdown::Both);
                return Err(e);
            }
        }
    }
    Ok(())
}

#[cfg(feature = "dev")]
fn open(mut commands: Commands, mut done: Local<bool>, mut next: ResMut<NextState<AppState>>) {
    let Ok(spec) = std::env::var("TROPA_LAN") else { return };
    if std::mem::replace(&mut *done, true) {
        return;
    }
    let (node, lan) = if let Some(port) = spec.strip_prefix("host:") {
        let listener = match TcpListener::bind(("0.0.0.0", port.parse::<u16>().unwrap_or(PORT))) {
            Ok(l) => l,
            Err(e) => {
                error!("LAN: cannot host on {port}: {e}");
                return;
            }
        };
        let _ = listener.set_nonblocking(true);
        info!("LAN: hosting on {port}");
        (Node::host(HOST, "host"), Lan { listener: Some(listener), links: vec![], me: HOST, addr: None, redial: None })
    } else if let Some(addr) = spec.strip_prefix("join:") {
        // the host may still be starting: keep knocking for a while
        let until = Instant::now() + Duration::from_secs(30);
        let stream = loop {
            match TcpStream::connect(addr) {
                Ok(s) => break s,
                Err(_) if Instant::now() < until => std::thread::sleep(Duration::from_millis(250)),
                Err(e) => {
                    error!("LAN: cannot join {addr}: {e}");
                    return;
                }
            }
        };
        let _ = stream.set_nonblocking(true);
        let _ = stream.set_nodelay(true);
        let me = 1000 + std::process::id() as Peer;
        info!("LAN: joined {addr} as {me}");
        (Node::join(me, &format!("guest{}", me % 100), HOST), Lan { listener: None, links: vec![Link { stream, peer: Some(HOST), buf: vec![] }], me, addr: Some(addr.to_string()), redial: None })
    } else {
        error!("LAN: TROPA_LAN is host:<port> or join:<addr>:<port>, not {spec}");
        return;
    };
    commands.insert_resource(Net { node, lobby: None, code: "LAN".into(), locals: vec![], timer: 0.0, bot_me: false, booked: false, ephemeral: true });
    commands.insert_resource(lan);
    next.set(AppState::Lobby);
}

fn receive(mut commands: Commands, mut lan: ResMut<Lan>, net: Option<ResMut<Net>>, db: Res<Db>, mut status: ResMut<NetStatus>, mut next: ResMut<NextState<AppState>>) {
    let Some(mut net) = net else { return };
    let lan = &mut *lan;
    if let Some(listener) = &lan.listener {
        while let Ok((stream, from)) = listener.accept() {
            let _ = stream.set_nonblocking(true);
            let _ = stream.set_nodelay(true);
            info!("LAN: {from} came in");
            lan.links.push(Link { stream, peer: None, buf: vec![] });
        }
    }
    let mut chunk = [0u8; 16384];
    let mut frames: Vec<(usize, Peer, Peer, Vec<u8>)> = vec![];
    let mut gone = vec![];
    for (k, link) in lan.links.iter_mut().enumerate() {
        loop {
            match link.stream.read(&mut chunk) {
                Ok(0) => {
                    gone.push(k);
                    break;
                }
                Ok(n) => link.buf.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(_) => {
                    gone.push(k);
                    break;
                }
            }
        }
        while link.buf.len() >= 4 {
            let len = u32::from_be_bytes(link.buf[..4].try_into().unwrap()) as usize;
            // not one of ours: whatever it is, it is not listened to
            if !(16..=MOST_FRAME).contains(&len) {
                warn!("LAN: a frame of {len} bytes, closing that link");
                let _ = link.stream.shutdown(Shutdown::Both);
                link.buf.clear();
                gone.push(k);
                break;
            }
            if link.buf.len() < 4 + len {
                break;
            }
            let body: Vec<u8> = link.buf.drain(..4 + len).skip(4).collect();
            let said = u64::from_be_bytes(body[..8].try_into().unwrap());
            let to = u64::from_be_bytes(body[8..16].try_into().unwrap());
            // a link is whoever it first said it was, and can never speak as the host
            if link.peer.is_none() && said == HOST {
                warn!("LAN: a guest calling itself the host, closing that link");
                let _ = link.stream.shutdown(Shutdown::Both);
                link.buf.clear();
                gone.push(k);
                break;
            }
            let from = *link.peer.get_or_insert(said);
            frames.push((k, from, to, body[16..].to_vec()));
        }
    }
    for (k, from, to, bytes) in frames {
        // the host passes on what is meant for someone else, or for everyone
        if lan.listener.is_some() && to != HOST {
            let f = frame(from, to, &bytes);
            for (j, link) in lan.links.iter_mut().enumerate() {
                if j != k && (to == EVERYONE || link.peer == Some(to)) {
                    let _ = write_all(&mut link.stream, &f);
                }
            }
        }
        if to == EVERYONE || to == lan.me {
            if let Some(msg) = Msg::decode(&bytes) {
                net.node.handle(&db.0, from, msg);
            }
        }
    }
    gone.sort_unstable();
    gone.dedup();
    for k in gone.into_iter().rev() {
        warn!("LAN: {:?} left", lan.links[k].peer);
        let link = lan.links.remove(k);
        match link.peer {
            // the line to the host dropped in the middle of a run: it is called again (see
            // `redial`), and the party is not given up until that has gone on long enough
            Some(HOST) if lan.listener.is_none() && net.node.state.is_some() && lan.addr.is_some() => {
                status.lost.get_or_insert(0.0);
            }
            // the host is the party: without it there is nothing left to play
            Some(HOST) if lan.listener.is_none() => {
                status.host_gone = true;
                commands.remove_resource::<Net>();
                next.set(AppState::Coop);
                return;
            }
            Some(peer) => net.node.peer_left(peer),
            None => {}
        }
    }
}

/// Sends what the node has to say before the in-process router would swallow it.
fn transmit(mut lan: ResMut<Lan>, net: Option<ResMut<Net>>) {
    let Some(mut net) = net else { return };
    let lan = &mut *lan;
    for (to, msg) in net.node.take_out() {
        let bytes = msg.encode();
        let to = match to {
            To::All => EVERYONE,
            To::One(p) => p,
        };
        let f = frame(lan.me, to, &bytes);
        for link in &mut lan.links {
            // a guest has one link, to the host, which passes things on
            if lan.listener.is_none() || to == EVERYONE || link.peer == Some(to) {
                let _ = write_all(&mut link.stream, &f);
            }
        }
    }
}

#[cfg(feature = "dev")]
/// `TROPA_FIGHT`: as soon as the run exists here, it opens on that fight. Each copy does it for
/// itself, and from the same seed they all come to the same.
fn open_on_fight(mut commands: Commands, net: Option<ResMut<Net>>, db: Res<Db>, mut done: Local<bool>) {
    let (Some(mut net), false) = (net, *done) else { return };
    let Ok(list) = std::env::var("TROPA_FIGHT") else { return };
    let me = net.node.my_slot();
    let Some(run) = net.node.state.as_mut() else { return };
    *done = true;
    let enemies: Vec<String> = list.split(',').map(|s| s.trim().to_string()).filter(|s| db.0.enemies.contains_key(s)).collect();
    if enemies.is_empty() {
        return;
    }
    crate::net::dev_fight(&db.0, run, &enemies);
    if let (Some(combat), Some(me)) = (run.combat.clone(), me) {
        commands.insert_resource(crate::session::Session::from_state(combat, me, []));
    }
}

#[cfg(feature = "dev")]
/// `TROPA_LAN_HERO`: take a hero and get ready unasked; the host starts once all are ready.
fn auto_lobby(net: Option<ResMut<Net>>, db: Res<Db>, profile: Res<crate::profile::Profile>, state: Res<State<AppState>>) {
    let (Some(mut net), Ok(hero)) = (net, std::env::var("TROPA_LAN_HERO")) else { return };
    if *state.get() != AppState::Lobby {
        return;
    }
    let Some(slot) = net.node.my_slot() else { return };
    let me = net.node.lobby.players[slot as usize].clone();
    if me.hero.as_deref() != Some(hero.as_str()) {
        net.node.pick_hero(Some(hero), 10);
        return;
    }
    if !me.ready {
        net.node.set_ready(true);
        return;
    }
    if net.node.lobby.players.len() > 1 && net.node.lobby.can_start() && !net.node.lobby.started {
        let seed = std::env::var("TROPA_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(1));
        net.node.secrets = profile.secret_relics();
        net.node.start(&db.0, seed, profile.village());
    }
}

/// A guest cut off from the host calls it again when asked to (see `reconnect`), and on getting
/// through says hello: the host hands back the seat and the run as it stands.
fn redial(mut lan: ResMut<Lan>, net: Option<ResMut<Net>>, mut status: ResMut<NetStatus>) {
    let Some(mut net) = net else { return };
    let lan = &mut *lan;
    if lan.listener.is_some() {
        return;
    }
    if let Some(rx) = &lan.redial {
        let got = rx.lock().ok().and_then(|r| r.try_recv().ok());
        match got {
            Some(Ok(stream)) => {
                let _ = stream.set_nonblocking(true);
                let _ = stream.set_nodelay(true);
                info!("LAN: got through to the host again");
                lan.links.clear();
                lan.links.push(Link { stream, peer: Some(HOST), buf: vec![] });
                lan.redial = None;
                net.node.rejoin();
            }
            Some(Err(e)) => {
                warn!("LAN: the host does not answer yet: {e}");
                lan.redial = None;
            }
            None => {}
        }
        return;
    }
    if !status.redial {
        return;
    }
    let Some(addr) = lan.addr.clone() else { return };
    status.redial = false;
    // whatever is left of the old line is hung up; a new one is dialled off to the side
    for link in &lan.links {
        let _ = link.stream.shutdown(Shutdown::Both);
    }
    lan.links.clear();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let reached = addr.to_socket_addrs().and_then(|mut found| found.next().ok_or_else(|| ErrorKind::NotFound.into())).and_then(|at| TcpStream::connect_timeout(&at, DIAL_WAIT));
        let _ = tx.send(reached);
    });
    lan.redial = Some(Mutex::new(rx));
}

/// Dev: `TROPA_LAN_DROP=<seconds>` cuts a guest's line to the host once, that long after start,
/// as a dropped connection would, to watch the guest find its way back.
#[cfg(feature = "dev")]
fn drop_line(mut lan: ResMut<Lan>, time: Res<Time>, mut done: Local<bool>) {
    let Some(at) = std::env::var("TROPA_LAN_DROP").ok().and_then(|v| v.parse::<f32>().ok()) else { return };
    if *done || lan.listener.is_some() || time.elapsed_secs() < at {
        return;
    }
    *done = true;
    warn!("LAN (dev): cutting the line to the host");
    // dropped without a word, as a dead network leaves it: the host sees the link close
    lan.links.clear();
}
