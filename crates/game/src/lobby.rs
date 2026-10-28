//! "Gather a party": create or join a lobby, pick heroes, ready up.

use crate::art::Art;
use crate::battle::Btn;
use crate::l10n::L10n;
use crate::menu::menu_button;
use crate::net::{self, Inbox, Net, NetStatus};
use crate::session::Db;
use crate::steam::Steam;
use crate::theme::{self, Fonts};
use crate::{AppState, Stage, STAGE_H, STAGE_W};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};

pub fn plugin(app: &mut App) {
    app.init_resource::<CodeEntry>()
        .add_systems(OnEnter(AppState::Coop), enter_coop)
        .add_systems(Update, (type_code, coop_labels).run_if(in_state(AppState::Coop)))
        .add_systems(OnEnter(AppState::Lobby), |mut status: ResMut<NetStatus>| status.dirty = true)
        .add_systems(Update, draw_lobby.run_if(in_state(AppState::Lobby)));
}

/// The join code being typed.
#[derive(Resource, Default)]
struct CodeEntry(String);

#[derive(Component)]
enum CoopLabel {
    Code,
    Status,
}

#[derive(Component)]
struct LobbyRoot;

fn column(state: AppState, stage: Entity) -> impl Bundle {
    (
        DespawnOnExit(state),
        Node {
            position_type: PositionType::Absolute,
            width: px(STAGE_W),
            height: px(STAGE_H),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(20),
            ..default()
        },
        BackgroundColor(theme::SOOT),
        Pickable::IGNORE,
        ChildOf(stage),
    )
}

fn enter_coop(
    mut commands: Commands,
    stage: Single<Entity, With<Stage>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    mut entry: ResMut<CodeEntry>,
    mut status: ResMut<NetStatus>,
) {
    entry.0.clear();
    status.error = None;
    status.busy = false;
    commands.spawn(column(AppState::Coop, *stage)).with_children(|p| {
        p.spawn(crate::menu::backdrop(&art, &assets, "bg/coop", 0.7));
        p.spawn(theme::text(l10n.t("coop-title"), &fonts.title, 72.0, theme::BONE));
        p.spawn((
            menu_button(l10n.t("coop-create"), &fonts),
            observe(|_: On<Activate>, steam: Option<Res<Steam>>, inbox: Option<Res<Inbox>>, mut status: ResMut<NetStatus>| {
                match (steam, inbox) {
                    (Some(steam), Some(inbox)) => net::create_lobby(&steam, &mut status, &inbox),
                    _ => status.error = Some("coop-error-steam".into()),
                }
            }),
        ));
        p.spawn((theme::text(l10n.t("coop-join-hint"), &fonts.body, 24.0, theme::DIM), Node { margin: UiRect::top(px(24)), ..default() }));
        p.spawn((
            theme::panel(Node {
                width: px(520),
                height: px(84),
                border: UiRect::all(px(3)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            }),
            children![(CoopLabel::Code, theme::text("", &fonts.num, 52.0, theme::CANDLE))],
        ));
        p.spawn((
            menu_button(l10n.t("coop-join"), &fonts),
            observe(|_: On<Activate>, steam: Option<Res<Steam>>, inbox: Option<Res<Inbox>>, entry: Res<CodeEntry>, mut status: ResMut<NetStatus>| {
                if entry.0.len() != 6 {
                    status.error = Some("coop-error-code".into());
                    return;
                }
                match (steam, inbox) {
                    (Some(steam), Some(inbox)) => net::join_by_code(&steam, &mut status, &inbox, &entry.0),
                    _ => status.error = Some("coop-error-steam".into()),
                }
            }),
        ));
        #[cfg(feature = "dev")]
        p.spawn((
            menu_button(l10n.t("coop-local"), &fonts),
            observe(|_: On<Activate>, mut commands: Commands, db: Res<Db>, mut next: ResMut<NextState<AppState>>| {
                net::start_offline(&mut commands, &db.0, "dev", 3);
                next.set(AppState::Lobby);
            }),
        ));
        p.spawn((CoopLabel::Status, theme::text("", &fonts.body, 24.0, theme::FRESH_BLOOD)));
        p.spawn((
            menu_button(l10n.t("coop-back"), &fonts),
            observe(|_: On<Activate>, mut next: ResMut<NextState<AppState>>| next.set(AppState::Menu)),
        ));
    });
}

fn type_code(mut keys: MessageReader<KeyboardInput>, mut entry: ResMut<CodeEntry>) {
    for k in keys.read() {
        if !k.state.is_pressed() {
            continue;
        }
        match &k.logical_key {
            Key::Backspace => {
                entry.0.pop();
            }
            Key::Character(s) => {
                for ch in s.chars().filter(|c| c.is_ascii_alphanumeric()) {
                    if entry.0.len() < 6 {
                        entry.0.push(ch.to_ascii_uppercase());
                    }
                }
            }
            _ => {}
        }
    }
}

fn coop_labels(entry: Res<CodeEntry>, status: Res<NetStatus>, l10n: Res<L10n>, mut labels: Query<(&CoopLabel, &mut Text)>) {
    for (label, mut text) in &mut labels {
        let s = match label {
            // six slots, the untyped ones shown as dots
            CoopLabel::Code => (0..6).map(|i| entry.0.chars().nth(i).unwrap_or('·').to_string()).collect::<Vec<_>>().join(" "),
            CoopLabel::Status => match (&status.error, status.busy) {
                (Some(key), _) => l10n.t(key),
                (None, true) => l10n.t("coop-busy"),
                _ => String::new(),
            },
        };
        if text.0 != s {
            text.0 = s;
        }
    }
}

/// A button sized for the lobby's bottom bar.
fn bar_button(label: String, fonts: &Fonts, width: f32, base: Color) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        Btn(base),
        Node { width: px(width), padding: UiRect::axes(px(16), px(14)), border: UiRect::all(px(3)), justify_content: JustifyContent::Center, ..default() },
        BackgroundColor(base),
        BorderColor::all(Color::srgb(0.42, 0.33, 0.22)),
        children![theme::text(label, &fonts.title, 26.0, theme::BONE)],
    )
}

fn absolute(left: f32, top: f32) -> Node {
    Node { position_type: PositionType::Absolute, left: px(left), top: px(top), ..default() }
}

/// The party sits round a fire: the seats along the top, the chosen hero large in the middle
/// with what they bring, every hero's face along the bottom to pick from.
fn draw_lobby(
    mut commands: Commands,
    mut status: ResMut<NetStatus>,
    net: Option<Res<Net>>,
    db: Res<Db>,
    stage: Single<Entity, With<Stage>>,
    old: Query<Entity, With<LobbyRoot>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    profile: Res<crate::profile::Profile>,
    mut next: ResMut<NextState<AppState>>,
) {
    let Some(net) = net else {
        next.set(AppState::Menu);
        return;
    };
    if !status.dirty {
        return;
    }
    status.dirty = false;
    for e in &old {
        commands.entity(e).despawn();
    }
    let lobby = &net.node.lobby;
    let me = net.me();
    let mine = lobby.players.iter().find(|p| p.peer == me);
    let is_host = net.node.is_host();
    let frame = Color::srgb(0.42, 0.33, 0.22);

    let root = commands
        .spawn((
            LobbyRoot,
            DespawnOnExit(AppState::Lobby),
            Node { position_type: PositionType::Absolute, width: px(STAGE_W), height: px(STAGE_H), ..default() },
            BackgroundColor(theme::SOOT),
            Pickable::IGNORE,
            ChildOf(*stage),
        ))
        .id();
    commands.spawn((crate::menu::backdrop(&art, &assets, "bg/lobby", 0.8), ChildOf(root)));
    // the faces to pick from sit on a darker band, so they read over the firelight
    commands.spawn((
        Node { position_type: PositionType::Absolute, left: px(0), bottom: px(0), width: px(STAGE_W), height: px(400), ..default() },
        BackgroundColor(theme::SOOT.with_alpha(0.62)),
        Pickable::IGNORE,
        ChildOf(root),
    ));

    // title and the join code
    commands.spawn((
        Node { position_type: PositionType::Absolute, top: px(22), width: px(STAGE_W), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() },
        Pickable::IGNORE,
        ChildOf(root),
    ))
    .with_children(|p| {
        p.spawn(theme::text(l10n.t("lobby-title"), &fonts.title, 56.0, theme::BONE));
        if net.lobby.is_some() {
            p.spawn(theme::text(l10n.tf("lobby-code", &[("code", net.code.clone().into())]), &fonts.num, 34.0, theme::CANDLE));
        }
    });

    // seats: each player's face, name, hero and readiness
    let seats_top = if net.lobby.is_some() { 132.0 } else { 104.0 };
    commands.spawn((
        Node { position_type: PositionType::Absolute, top: px(seats_top), width: px(STAGE_W), column_gap: px(18), justify_content: JustifyContent::Center, ..default() },
        Pickable::IGNORE,
        ChildOf(root),
    ))
    .with_children(|row| {
        for slot in 0..nav_net::MAX_PLAYERS {
            let player = lobby.players.get(slot);
            let hero = player.and_then(|p| p.hero.as_deref());
            let color = hero.map(theme::hero_color).unwrap_or(frame);
            row.spawn((
                theme::panel_edged(
                    Node { width: px(380), height: px(112), border: UiRect::all(px(3)), padding: UiRect::all(px(8)), column_gap: px(12), align_items: AlignItems::Center, ..default() },
                    color,
                ),
                Pickable::IGNORE,
            ))
            .with_children(|seat| {
                let face = hero.and_then(|h| art.image(&assets, &format!("portraits/{h}")));
                seat.spawn((
                    Node { width: px(94), height: px(94), border: UiRect::all(px(2)), ..default() },
                    BorderColor::all(color),
                    BackgroundColor(theme::SOOT),
                    Pickable::IGNORE,
                ))
                .with_children(|f| {
                    if let Some(face) = face {
                        f.spawn((ImageNode::new(face), Node { width: percent(100), height: percent(100), ..default() }, Pickable::IGNORE));
                    }
                });
                seat.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(2), ..default() }, Pickable::IGNORE)).with_children(|col| match player {
                    Some(pl) => {
                        let mut name = pl.name.clone();
                        if pl.peer == lobby.host {
                            name.push_str(&format!("  ·  {}", l10n.t("lobby-host")));
                        }
                        col.spawn(theme::text(name, &fonts.body, 20.0, theme::BONE));
                        match hero {
                            Some(h) => col.spawn(theme::text(l10n.t(&format!("hero-{h}")), &fonts.title, 24.0, color.lighter(0.2))),
                            None => col.spawn(theme::text(l10n.t("lobby-no-hero"), &fonts.body, 18.0, theme::DIM)),
                        };
                        let state = if !pl.connected {
                            ("lobby-dropped", theme::FRESH_BLOOD)
                        } else if pl.ready {
                            ("battle-ready", theme::ROTLIGHT)
                        } else {
                            ("lobby-choosing", theme::DIM)
                        };
                        col.spawn(theme::text(l10n.t(state.0), &fonts.body, 18.0, state.1));
                    }
                    None => {
                        col.spawn(theme::text(l10n.t("lobby-empty"), &fonts.body, 20.0, theme::DIM));
                    }
                });
            });
        }
    });

    // the chosen hero, large, with what they bring
    match mine.and_then(|m| m.hero.as_ref().map(|h| (m, h))).and_then(|(m, h)| db.0.heroes.get(h).map(|d| (m, h, d))) {
        Some((m, hero, def)) => {
            let color = theme::hero_color(hero);
            let figure_h = 460.0;
            let figure_w = figure_h * art.aspect(&format!("heroes/{hero}")).unwrap_or(0.75);
            if let Some(figure) = art.image(&assets, &format!("heroes/{hero}")) {
                commands.spawn((
                    ImageNode::new(figure),
                    Node { width: px(figure_w), height: px(figure_h), ..absolute(560.0 - figure_w / 2.0, 222.0) },
                    Pickable::IGNORE,
                    ChildOf(root),
                ));
            }
            commands
                .spawn((
                    theme::panel_edged(
                        Node {
                            width: px(820),
                            padding: UiRect::axes(px(28), px(20)),
                            border: UiRect::all(px(3)),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(12),
                            ..absolute(840.0, 236.0)
                        },
                        color,
                    ),
                    Pickable::IGNORE,
                    ChildOf(root),
                ))
                .with_children(|p| {
                    p.spawn(theme::text(l10n.t(&format!("hero-{hero}")), &fonts.title, 60.0, color.lighter(0.25)));
                    p.spawn(theme::text(
                        format!("{}  ·  {}", l10n.n("lobby-hp", def.max_hp), l10n.n("lobby-level", profile.hero_level(hero) as i32)),
                        &fonts.num,
                        26.0,
                        theme::CANDLE,
                    ));
                    p.spawn(theme::text(l10n.t(&format!("hero-{hero}-desc")), &fonts.body, 22.0, theme::BONE));
                    p.spawn((theme::text(l10n.t("lobby-relic"), &fonts.body, 18.0, theme::DIM), Node { margin: UiRect::top(px(6)), ..default() }));
                    // the shrine lets a hero start with another relic: each choice is a button
                    let shrine = profile.building("shrine").min(2) as usize;
                    let choices = std::iter::once(&def.relic).chain(def.alt_relics.iter().take(shrine));
                    for (k, relic) in choices.enumerate() {
                        let picked = m.starter as usize == k;
                        let base = if picked && shrine > 0 { theme::SWAMP.with_alpha(0.9) } else { theme::SOOT.with_alpha(0.7) };
                        let mut row = p.spawn((
                            Node { column_gap: px(14), align_items: AlignItems::Center, padding: UiRect::all(px(6)), border: UiRect::all(px(2)), ..default() },
                            BackgroundColor(base),
                            BorderColor::all(if picked && shrine > 0 { theme::BONE } else { Color::NONE }),
                        ));
                        row.with_children(|r| {
                            r.spawn((art.node(&assets, &format!("relics/{relic}"), theme::PEAT), Node { width: px(64), height: px(64), flex_shrink: 0.0, ..default() }, Pickable::IGNORE));
                            r.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(2), ..default() }, Pickable::IGNORE)).with_children(|t| {
                                t.spawn(theme::text(l10n.t(&format!("relic-{relic}")), &fonts.title, 22.0, theme::CANDLE));
                                t.spawn(theme::text(l10n.t(&format!("relic-{relic}-desc")), &fonts.body, 18.0, theme::BONE));
                            });
                        });
                        if shrine > 0 {
                            row.insert((Button, Hovered::default(), Btn(base)));
                            row.observe(move |_: On<Activate>, mut net: ResMut<Net>| net.node.pick_starter(k as u8));
                        } else {
                            row.insert(Pickable::IGNORE);
                        }
                    }
                });
        }
        None => {
            commands.spawn((
                theme::text(l10n.t("lobby-pick"), &fonts.title, 52.0, theme::BONE),
                Node { position_type: PositionType::Absolute, top: px(400), width: px(STAGE_W), justify_content: JustifyContent::Center, ..default() },
                TextLayout::justify(Justify::Center),
                ChildOf(root),
            ));
        }
    }

    // every hero's face: a hero another player took is dimmed and says whose it is
    commands.spawn((
        Node { position_type: PositionType::Absolute, top: px(690), width: px(STAGE_W), column_gap: px(26), justify_content: JustifyContent::Center, align_items: AlignItems::End, ..default() },
        Pickable::IGNORE,
        ChildOf(root),
    ))
    .with_children(|row| {
        for hero in db.0.heroes.keys() {
            let owner = lobby.players.iter().find(|pl| pl.peer != me && pl.hero.as_deref() == Some(hero.as_str()));
            let picked = mine.is_some_and(|m| m.hero.as_deref() == Some(hero.as_str()));
            let color = theme::hero_color(hero);
            let size = if picked { 168.0 } else { 148.0 };
            let id = hero.clone();
            let mut button = row.spawn((
                Button,
                Hovered::default(),
                Btn(theme::SOOT),
                Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(6), padding: UiRect::all(px(4)), ..default() },
                BackgroundColor(theme::SOOT),
            ));
            button.with_children(|b| {
                let (mut face, _) = art.node(&assets, &format!("portraits/{hero}"), color);
                if owner.is_some() {
                    face.color = Color::srgb(0.3, 0.3, 0.3);
                }
                b.spawn((
                    face,
                    Node { width: px(size), height: px(size), border: UiRect::all(px(if picked { 5 } else { 3 })), ..default() },
                    BorderColor::all(if picked { theme::BONE } else { color }),
                    Pickable::IGNORE,
                ));
                b.spawn(theme::text(l10n.t(&format!("hero-{hero}")), &fonts.title, 22.0, if owner.is_some() { theme::DIM } else { color.lighter(0.3) }));
                let under = match owner {
                    Some(o) => l10n.tf("lobby-taken", &[("name", o.name.clone().into())]),
                    None => l10n.n("lobby-level", profile.hero_level(hero) as i32),
                };
                b.spawn(theme::text(under, &fonts.body, 16.0, theme::DIM));
            });
            if owner.is_none() {
                button.observe(move |_: On<Activate>, mut net: ResMut<Net>, profile: Res<crate::profile::Profile>| {
                    let level = profile.hero_level(&id);
                    let hero = if picked { None } else { Some(id.clone()) };
                    net.node.pick_hero(hero, level);
                });
            }
        }
    });

    // the bottom bar: leave on the left, the run's terms in the middle, setting out on the right
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(50),
            right: px(50),
            bottom: px(26),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::End,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(root),
    ))
    .with_children(|bar| {
        bar.spawn((
            bar_button(l10n.t("lobby-leave"), &fonts, 330.0, theme::PEAT),
            observe(|_: On<Activate>, mut commands: Commands, steam: Option<Res<Steam>>, net: Res<Net>, mut next: ResMut<NextState<AppState>>| {
                net::leave(&mut commands, steam.as_deref(), &net);
                next.set(AppState::Menu);
            }),
        ));
        bar.spawn((Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(8), ..default() }, Pickable::IGNORE)).with_children(|mid| {
            if is_host && !lobby.can_start() {
                mid.spawn(theme::text(l10n.t("lobby-waiting"), &fonts.body, 18.0, theme::DIM));
            }
            mid.spawn((Node { column_gap: px(18), align_items: AlignItems::Center, ..default() }, Pickable::IGNORE)).with_children(|row| {
                // Circle of Nav: the host may pick any circle they have opened
                let circle = lobby.circle;
                let max = profile.circle_unlocked;
                let step = |row: &mut ChildSpawnerCommands, label: &str, to: Option<u8>| {
                    let base = if to.is_some() { theme::PEAT } else { theme::SOOT };
                    let mut e = row.spawn((
                        Button,
                        Hovered::default(),
                        Btn(base),
                        Node { width: px(48), height: px(48), border: UiRect::all(px(2)), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
                        BackgroundColor(base),
                        BorderColor::all(frame),
                        children![theme::text(label, &fonts.num, 28.0, if to.is_some() { theme::BONE } else { theme::DIM })],
                    ));
                    if let Some(to) = to {
                        e.observe(move |_: On<Activate>, mut net: ResMut<Net>| net.node.set_circle(to));
                    }
                };
                if is_host {
                    step(row, "−", (circle > 0).then(|| circle - 1));
                }
                row.spawn(theme::text(l10n.n("lobby-circle", circle as i32), &fonts.title, 24.0, if circle > 0 { theme::FRESH_BLOOD } else { theme::BONE }));
                if is_host {
                    step(row, "+", (circle < max).then(|| circle + 1));
                }
                // turn timer: off by default; the host may set 60, 90 or 120 seconds
                if lobby.players.len() > 1 {
                    let timer = lobby.turn_timer;
                    let label = match timer {
                        Some(s) => l10n.n("lobby-timer", s as i32),
                        None => l10n.t("lobby-timer-off"),
                    };
                    if is_host {
                        row.spawn((
                            bar_button(label, &fonts, 400.0, theme::PEAT),
                            observe(move |_: On<Activate>, mut net: ResMut<Net>| {
                                let next = match timer {
                                    None => Some(60),
                                    Some(60) => Some(90),
                                    Some(90) => Some(120),
                                    _ => None,
                                };
                                net.node.set_turn_timer(next);
                            }),
                        ));
                    } else {
                        row.spawn(theme::text(label, &fonts.body, 20.0, theme::DIM));
                    }
                }
            });
        });
        bar.spawn((Node { column_gap: px(14), ..default() }, Pickable::IGNORE)).with_children(|row| {
            if net.lobby.is_some() {
                row.spawn((
                    bar_button(l10n.t("lobby-invite"), &fonts, 260.0, theme::PEAT),
                    observe(|_: On<Activate>, steam: Option<Res<Steam>>, net: Res<Net>| {
                        if let Some(steam) = steam {
                            net::invite(&steam, &net);
                        }
                    }),
                ));
            }
            let ready = mine.is_some_and(|m| m.ready);
            let can_ready = mine.is_some_and(|m| m.hero.is_some());
            row.spawn((
                bar_button(l10n.t(if ready { "battle-unready" } else { "battle-ready" }), &fonts, 240.0, if ready { theme::SWAMP } else if can_ready { theme::PEAT } else { theme::SOOT }),
                observe(move |_: On<Activate>, mut net: ResMut<Net>| net.node.set_ready(!ready)),
            ));
            if is_host {
                let base = if lobby.can_start() { theme::RUST_BLOOD } else { theme::SOOT };
                row.spawn((
                    bar_button(l10n.t("lobby-start"), &fonts, 260.0, base),
                    observe(|_: On<Activate>, mut net: ResMut<Net>, db: Res<Db>, time: Res<Time>, profile: Res<crate::profile::Profile>| {
                        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1) ^ time.elapsed().as_nanos() as u64;
                        // the host's village decides which relics, potions and omens can turn up
                        net.node.secrets = profile.secret_relics();
                        net.node.start(&db.0, seed, profile.village());
                    }),
                ));
            }
        });
    });
}
