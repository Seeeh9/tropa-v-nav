//! Dev only: a co-op transport over TCP, so two copies of the game on one machine (or a LAN) can
//! play together without Steam. The protocol node does not care how its messages travel; here
//! they go as frames over sockets, the host in the middle passing on what is meant for others,
//! as Steam would deliver it.
//!
//!   TROPA_LAN=host:<port>          host a party on that port
//!   TROPA_LAN=join:<addr>:<port>   join it
//!   TROPA_SEED=<n>                 (host) a fixed seed for the run
//!   TROPA_LAN_HERO=<id>            take this hero and get ready at once; the host starts the run
//!                                  as soon as everyone is
//!
//! A frame is its length (u32), the sender (u64), the addressee (u64, 0 for everyone) and the
//! message as `Msg::encode` writes it, all big-endian.

use crate::net::Net;
use crate::session::Db;
use crate::AppState;
use bevy::prelude::*;
use nav_net::{Msg, Node, Peer, To};
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

const HOST: Peer = 1;
const EVERYONE: Peer = 0;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Menu), open)
        .add_systems(PreUpdate, receive.run_if(resource_exists::<Lan>))
        .add_systems(Update, auto_lobby.run_if(resource_exists::<Lan>))
        .add_systems(PostUpdate, transmit.run_if(resource_exists::<Lan>).before(crate::net::send));
}

#[derive(Resource)]
struct Lan {
    listener: Option<TcpListener>,
    links: Vec<Link>,
    me: Peer,
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

/// Writes all of it, waiting out a full send buffer: on a local socket that never lasts.
fn write_all(stream: &mut TcpStream, mut bytes: &[u8]) -> std::io::Result<()> {
    while !bytes.is_empty() {
        match stream.write(bytes) {
            Ok(0) => return Err(ErrorKind::WriteZero.into()),
            Ok(n) => bytes = &bytes[n..],
            Err(e) if e.kind() == ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(1)),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn open(mut commands: Commands, mut done: Local<bool>, mut next: ResMut<NextState<AppState>>) {
    let Ok(spec) = std::env::var("TROPA_LAN") else { return };
    if std::mem::replace(&mut *done, true) {
        return;
    }
    let (node, lan) = if let Some(port) = spec.strip_prefix("host:") {
        let listener = match TcpListener::bind(("127.0.0.1", port.parse::<u16>().unwrap_or(7777))) {
            Ok(l) => l,
            Err(e) => {
                error!("LAN: cannot host on {port}: {e}");
                return;
            }
        };
        let _ = listener.set_nonblocking(true);
        info!("LAN: hosting on {port}");
        (Node::host(HOST, "host"), Lan { listener: Some(listener), links: vec![], me: HOST })
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
        (Node::join(me, &format!("guest{}", me % 100), HOST), Lan { listener: None, links: vec![Link { stream, peer: Some(HOST), buf: vec![] }], me })
    } else {
        error!("LAN: TROPA_LAN is host:<port> or join:<addr>:<port>, not {spec}");
        return;
    };
    commands.insert_resource(Net { node, lobby: None, code: "LAN".into(), locals: vec![], timer: 0.0, bot_me: false, booked: false, ephemeral: true });
    commands.insert_resource(lan);
    next.set(AppState::Lobby);
}

fn receive(mut lan: ResMut<Lan>, net: Option<ResMut<Net>>, db: Res<Db>) {
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
            if link.buf.len() < 4 + len {
                break;
            }
            let body: Vec<u8> = link.buf.drain(..4 + len).skip(4).collect();
            let from = u64::from_be_bytes(body[..8].try_into().unwrap());
            let to = u64::from_be_bytes(body[8..16].try_into().unwrap());
            link.peer.get_or_insert(from);
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
    for k in gone.into_iter().rev() {
        warn!("LAN: {:?} left", lan.links[k].peer);
        lan.links.remove(k);
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
