//! "Gather a party": create or join a lobby, pick heroes, ready up.

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

fn enter_coop(mut commands: Commands, stage: Single<Entity, With<Stage>>, fonts: Res<Fonts>, l10n: Res<L10n>, mut entry: ResMut<CodeEntry>, mut status: ResMut<NetStatus>) {
    entry.0.clear();
    status.error = None;
    status.busy = false;
    commands.spawn(column(AppState::Coop, *stage)).with_children(|p| {
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

fn draw_lobby(
    mut commands: Commands,
    mut status: ResMut<NetStatus>,
    net: Option<Res<Net>>,
    db: Res<Db>,
    stage: Single<Entity, With<Stage>>,
    old: Query<Entity, With<LobbyRoot>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
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

    commands.spawn((LobbyRoot, column(AppState::Lobby, *stage))).with_children(|p| {
        p.spawn(theme::text(l10n.t("lobby-title"), &fonts.title, 64.0, theme::BONE));
        if net.lobby.is_some() {
            p.spawn(theme::text(l10n.tf("lobby-code", &[("code", net.code.clone().into())]), &fonts.num, 40.0, theme::CANDLE));
        }

        // seats
        p.spawn((Node { column_gap: px(18), margin: UiRect::vertical(px(12)), ..default() }, Pickable::IGNORE)).with_children(|row| {
            for slot in 0..nav_net::MAX_PLAYERS {
                let player = lobby.players.get(slot);
                let color = player.and_then(|p| p.hero.as_deref()).map(theme::hero_color).unwrap_or(theme::PEAT);
                row.spawn((
                    theme::panel_edged(
                        Node {
                            width: px(300),
                            height: px(170),
                            border: UiRect::all(px(4)),
                            padding: UiRect::all(px(12)),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(6),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        color,
                    ),
                    Pickable::IGNORE,
                ))
                .with_children(|seat| match player {
                    Some(pl) => {
                        let mut name = pl.name.clone();
                        if pl.peer == lobby.host {
                            name.push_str(&format!("  ·  {}", l10n.t("lobby-host")));
                        }
                        seat.spawn(theme::text(name, &fonts.body, 24.0, theme::BONE));
                        let hero = pl.hero.as_deref().map(|h| l10n.t(&format!("hero-{h}"))).unwrap_or_else(|| l10n.t("lobby-no-hero"));
                        seat.spawn(theme::text(hero, &fonts.title, 28.0, color.lighter(0.2)));
                        let state = if !pl.connected {
                            ("lobby-dropped", theme::FRESH_BLOOD)
                        } else if pl.ready {
                            ("battle-ready", theme::ROTLIGHT)
                        } else {
                            ("lobby-choosing", theme::DIM)
                        };
                        seat.spawn(theme::text(l10n.t(state.0), &fonts.body, 22.0, state.1));
                    }
                    None => {
                        seat.spawn(theme::text(l10n.t("lobby-empty"), &fonts.body, 22.0, theme::DIM));
                    }
                });
            }
        });

        // hero picker: a hero taken by someone else is dark
        p.spawn((Node { column_gap: px(12), ..default() }, Pickable::IGNORE)).with_children(|row| {
            for hero in db.0.heroes.keys() {
                let taken = lobby.players.iter().any(|pl| pl.peer != me && pl.hero.as_deref() == Some(hero.as_str()));
                let picked = mine.is_some_and(|m| m.hero.as_deref() == Some(hero.as_str()));
                let base = if taken { theme::SOOT } else { theme::hero_color(hero).darker(0.12) };
                let id = hero.clone();
                let mut button = row.spawn((
                    Button,
                    Hovered::default(),
                    Btn(base),
                    Node { width: px(270), padding: UiRect::axes(px(10), px(14)), border: UiRect::all(px(3)), justify_content: JustifyContent::Center, ..default() },
                    BackgroundColor(base),
                    BorderColor::all(if picked { theme::BONE } else { theme::hero_color(hero) }),
                    children![theme::text(
                        format!("{}  ·  {}", l10n.t(&format!("hero-{hero}")), l10n.n("lobby-level", profile.hero_level(hero) as i32)),
                        &fonts.title,
                        18.0,
                        if taken { theme::DIM } else { theme::BONE }
                    )],
                ));
                if !taken {
                    button.observe(move |_: On<Activate>, mut net: ResMut<Net>| {
                        let hero = if picked { None } else { Some(id.clone()) };
                        net.node.pick_hero(hero);
                    });
                }
            }
        });

        p.spawn((Node { column_gap: px(18), margin: UiRect::top(px(18)), ..default() }, Pickable::IGNORE)).with_children(|row| {
            let ready = mine.is_some_and(|m| m.ready);
            row.spawn((
                menu_button(l10n.t(if ready { "battle-unready" } else { "battle-ready" }), &fonts),
                observe(move |_: On<Activate>, mut net: ResMut<Net>| net.node.set_ready(!ready)),
            ));
            if net.lobby.is_some() {
                row.spawn((
                    menu_button(l10n.t("lobby-invite"), &fonts),
                    observe(|_: On<Activate>, steam: Option<Res<Steam>>, net: Res<Net>| {
                        if let Some(steam) = steam {
                            net::invite(&steam, &net);
                        }
                    }),
                ));
            }
            if is_host {
                row.spawn((
                    menu_button(l10n.t("lobby-start"), &fonts),
                    observe(|_: On<Activate>, mut net: ResMut<Net>, db: Res<Db>, time: Res<Time>| {
                        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1) ^ time.elapsed().as_nanos() as u64;
                        net.node.start(&db.0, seed);
                    }),
                ));
            }
        });
        // Circle of Nav: the host may pick any circle they have opened
        let circle = lobby.circle;
        p.spawn((Node { column_gap: px(14), align_items: AlignItems::Center, ..default() }, Pickable::IGNORE)).with_children(|row| {
            let max = profile.circle_unlocked;
            let step = |row: &mut ChildSpawnerCommands, label: &str, to: Option<u8>| {
                let base = if to.is_some() { theme::PEAT } else { theme::SOOT };
                let mut e = row.spawn((
                    Button,
                    Hovered::default(),
                    Btn(base),
                    Node { width: px(56), height: px(48), border: UiRect::all(px(2)), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
                    BackgroundColor(base),
                    BorderColor::all(Color::srgb(0.42, 0.33, 0.22)),
                    children![theme::text(label, &fonts.num, 28.0, if to.is_some() { theme::BONE } else { theme::DIM })],
                ));
                if let Some(to) = to {
                    e.observe(move |_: On<Activate>, mut net: ResMut<Net>| net.node.set_circle(to));
                }
            };
            if is_host {
                step(row, "−", (circle > 0).then(|| circle - 1));
            }
            row.spawn(theme::text(l10n.n("lobby-circle", circle as i32), &fonts.title, 26.0, if circle > 0 { theme::FRESH_BLOOD } else { theme::BONE }));
            if is_host {
                step(row, "+", (circle < max).then(|| circle + 1));
            }
        });
        if is_host && !lobby.can_start() {
            p.spawn(theme::text(l10n.t("lobby-waiting"), &fonts.body, 22.0, theme::DIM));
        }
        p.spawn((
            menu_button(l10n.t("lobby-leave"), &fonts),
            observe(|_: On<Activate>, mut commands: Commands, steam: Option<Res<Steam>>, net: Res<Net>, mut next: ResMut<NextState<AppState>>| {
                net::leave(&mut commands, steam.as_deref(), &net);
                next.set(AppState::Menu);
            }),
        ));
    });
}
