//! The tale: full-screen painted pages with a few lines of text, shown at the start of a run,
//! on entering each act and at the end. Which ending a player gets depends on how the run went,
//! and every ending leads back to the gate: the next run opens with a page that remembers it.
//!
//! A page's id is both its picture (`assets/story/<id>-gemini.png`) and its text (`story-<id>`).
//! Pages are drawn over whatever screen is open and swallow its input until read or skipped;
//! each player reads at their own pace, nothing here touches the shared run.

use crate::art::Art;
use crate::l10n::L10n;
use crate::battle::Btn;
use crate::net::Net;
use crate::profile::Profile;
use crate::theme::{self, Fonts};
use crate::{Stage as Canvas, STAGE_H, STAGE_W};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};
use nav_core::{CardType, Run, Stage};
use std::collections::VecDeque;

/// Every ending there is, in the order the chronicle lists them.
pub const ENDINGS: [&str; 8] = ["fall_forest", "fall_village", "fall_nav", "fall_koschei", "win_dawn", "win_scarred", "win_throne", "win_circle"];

/// Curses in the deck at the moment of victory that turn the hero towards the throne.
const THRONE_CURSES: usize = 3;

pub fn plugin(app: &mut App) {
    app.init_resource::<Tale>().add_systems(Update, (watch_run, turn_pages, show_page).chain());
    #[cfg(feature = "dev")]
    app.add_systems(Startup, dev_scene);
}

/// Dev: `TROPA_STORY=pro|again|again-<ending>|act2|act3|act4|<ending>` opens that scene at once,
/// `TROPA_STORY_PAGE=<n>` on its n-th page, to look at it or take a screenshot.
#[cfg(feature = "dev")]
fn dev_scene(mut tale: ResMut<Tale>) {
    let Ok(scene) = std::env::var("TROPA_STORY") else { return };
    let mut pages = match scene.as_str() {
        "pro" => vec![page("pro_1"), page("pro_2"), page("pro_3")],
        "again" => vec![Page { picture: "story/again".into(), text: "story-again".into(), title: None, closes: false }],
        s if s.starts_with("again-") => vec![Page { picture: "story/again".into(), text: format!("story-{s}"), title: None, closes: false }],
        s if s.starts_with("act") => vec![page(s)],
        s => ending_pages(s),
    };
    let skip = std::env::var("TROPA_STORY_PAGE").ok().and_then(|n| n.parse::<usize>().ok()).unwrap_or(1).saturating_sub(1);
    tale.total = pages.len();
    tale.pages.extend(pages.drain(skip.min(pages.len().saturating_sub(1))..));
}

struct Page {
    picture: String,
    /// Fluent key of the text.
    text: String,
    /// Fluent key of a heading, for the first page of an ending.
    title: Option<String>,
    /// The page belongs to an ending: its last button closes the tale rather than sends you off.
    closes: bool,
}

/// Pages waiting to be read; the front one is on screen.
#[derive(Resource, Default)]
pub struct Tale {
    pages: VecDeque<Page>,
    /// How many pages the scene being read had when it began, for "2 / 3".
    total: usize,
    shown: Option<Entity>,
}

impl Tale {
    fn tell(&mut self, pages: Vec<Page>) {
        if self.pages.is_empty() {
            self.total = 0;
        }
        self.total += pages.len();
        self.pages.extend(pages);
    }

    pub fn open(&self) -> bool {
        !self.pages.is_empty()
    }
}

fn page(id: &str) -> Page {
    Page { picture: format!("story/{id}"), text: format!("story-{id}"), title: None, closes: false }
}

/// The ending this player has earned.
pub fn ending(run: &Run, me: usize, db: &nav_core::Content) -> &'static str {
    if run.stage != Stage::Victory {
        return match run.act {
            0 | 1 => "fall_forest",
            2 => "fall_village",
            3 => "fall_nav",
            _ => "fall_koschei",
        };
    }
    let Some(pl) = run.players.get(me) else { return "win_dawn" };
    let curses = pl.deck.iter().filter(|(id, _)| db.cards.get(id).is_some_and(|c| c.kind == CardType::Curse)).count();
    // a price was paid: someone fell on the way, or the victor crawled out barely alive
    let scarred = run.players.iter().any(|p| p.falls > 0 || p.hp <= 0) || pl.hp * 4 <= pl.max_hp;
    if run.circle >= 10 {
        "win_circle"
    } else if curses >= THRONE_CURSES {
        "win_throne"
    } else if scarred {
        "win_scarred"
    } else {
        "win_dawn"
    }
}

fn ending_pages(id: &str) -> Vec<Page> {
    let mut pages = vec![page(&format!("{id}_1")), page(&format!("{id}_2"))];
    pages[0].title = Some(format!("story-name-{id}"));
    for p in &mut pages {
        p.closes = true;
    }
    pages
}

/// What the run being watched has already been told.
#[derive(Default)]
struct Told {
    seed: Option<u64>,
    act: u8,
    ended: bool,
}

fn silent(net: &Net) -> bool {
    // the autopilot does not read, and a screenshot run must not be covered by a page
    net.bot_me || (cfg!(feature = "dev") && std::env::var("TROPA_SHOT").is_ok() && std::env::var("TROPA_STORY").is_err())
}

fn watch_run(net: Option<Res<Net>>, db: Res<crate::session::Db>, mut tale: ResMut<Tale>, mut profile: ResMut<Profile>, mut told: Local<Told>) {
    let Some(net) = net else {
        *told = Told::default();
        return;
    };
    let Some(run) = net.node.state.as_ref() else { return };
    if told.seed != Some(run.seed) {
        *told = Told { seed: Some(run.seed), act: run.act, ended: false };
        // a run picked up from a save has had its opening already
        let fresh = run.act <= 1 && run.floors_cleared == 0 && run.at.is_none();
        if fresh && !silent(&net) {
            match profile.last_ending.clone() {
                Some(last) if profile.prologue_seen => {
                    tale.tell(vec![Page { picture: "story/again".into(), text: format!("story-again-{last}"), title: None, closes: false }]);
                }
                _ if profile.prologue_seen => tale.tell(vec![Page { picture: "story/again".into(), text: "story-again".into(), title: None, closes: false }]),
                _ => {
                    tale.tell(vec![page("pro_1"), page("pro_2"), page("pro_3")]);
                    if !net.ephemeral {
                        profile.prologue_seen = true;
                        profile.save();
                    }
                }
            }
        }
    }
    if run.act > told.act {
        told.act = run.act;
        if (2..=4).contains(&run.act) && !silent(&net) {
            tale.tell(vec![page(&format!("act{}", run.act))]);
        }
    }
    if matches!(run.stage, Stage::Victory | Stage::Defeat) && !told.ended {
        told.ended = true;
        let Some(me) = net.node.my_slot() else { return };
        let id = ending(run, me as usize, &db.0);
        if !silent(&net) {
            tale.tell(ending_pages(id));
        }
        if !net.bot_me && !net.ephemeral {
            profile.note_ending(id);
        }
    }
}

#[derive(Component)]
struct PageRoot;

fn page_button(label: String, fonts: &Fonts) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        Btn(theme::PEAT),
        Node { width: px(300), padding: UiRect::axes(px(20), px(10)), border: UiRect::all(px(3)), justify_content: JustifyContent::Center, ..default() },
        BackgroundColor(theme::PEAT),
        BorderColor::all(Color::srgb(0.42, 0.33, 0.22)),
        children![theme::text(label, &fonts.title, 28.0, theme::BONE)],
    )
}

/// Pressed on a page: read on, or drop the rest of the scene.
#[derive(Event)]
enum Turn {
    Next,
    Skip,
}

fn turn_pages(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, tale: Res<Tale>) {
    if !tale.open() {
        return;
    }
    if keys.any_just_pressed([KeyCode::Space, KeyCode::Enter, KeyCode::ArrowRight]) {
        commands.trigger(Turn::Next);
    } else if keys.just_pressed(KeyCode::Escape) {
        commands.trigger(Turn::Skip);
    }
}

fn on_turn(turn: On<Turn>, mut commands: Commands, mut tale: ResMut<Tale>) {
    match *turn {
        Turn::Next => {
            tale.pages.pop_front();
        }
        Turn::Skip => tale.pages.clear(),
    }
    if let Some(e) = tale.shown.take() {
        commands.entity(e).despawn();
    }
}

fn show_page(
    mut commands: Commands,
    mut tale: ResMut<Tale>,
    mut hooked: Local<bool>,
    canvas: Single<Entity, With<Canvas>>,
    fonts: Res<Fonts>,
    l10n: Res<L10n>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    if !*hooked {
        *hooked = true;
        commands.add_observer(on_turn);
    }
    if tale.shown.is_some() {
        return;
    }
    let Some(page) = tale.pages.front() else { return };
    let left = tale.pages.len();
    let at = tale.total.saturating_sub(left) + 1;
    let last = left == 1;
    let onward = match (last, page.closes) {
        (false, _) => "story-next",
        (true, true) => "story-end",
        (true, false) => "story-close",
    };
    let root = commands
        .spawn((
            PageRoot,
            Node { position_type: PositionType::Absolute, width: px(STAGE_W), height: px(STAGE_H), ..default() },
            BackgroundColor(theme::SOOT),
            GlobalZIndex(200),
            // not `Pickable::IGNORE`: the page hides the screen beneath and takes its clicks
            Pickable::default(),
            ChildOf(*canvas),
        ))
        .id();
    commands.spawn((
        Node { position_type: PositionType::Absolute, width: px(STAGE_W), height: px(STAGE_H), ..default() },
        art.node(&assets, &page.picture, theme::SOOT),
        Pickable::IGNORE,
        ChildOf(root),
    ));
    if let Some(title) = &page.title {
        commands.spawn((
            Node { position_type: PositionType::Absolute, left: px(0), top: px(48), width: px(STAGE_W), justify_content: JustifyContent::Center, ..default() },
            Pickable::IGNORE,
            ChildOf(root),
            children![(
                Node { padding: UiRect::axes(px(40), px(10)), ..default() },
                BackgroundColor(theme::SOOT.with_alpha(0.78)),
                children![theme::text(l10n.t(title), &fonts.title, 60.0, theme::CANDLE)],
            )],
        ));
    }
    // the text sits on a dark band along the bottom, like a caption under a painting
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            bottom: px(0),
            width: px(STAGE_W),
            padding: UiRect::new(px(150), px(150), px(34), px(30)),
            flex_direction: FlexDirection::Column,
            row_gap: px(22),
            border: UiRect::top(px(3)),
            ..default()
        },
        BackgroundColor(theme::SOOT.with_alpha(0.88)),
        BorderColor::all(Color::srgb(0.42, 0.33, 0.22)),
        Pickable::IGNORE,
        ChildOf(root),
        children![
            (theme::text(l10n.t(&page.text), &fonts.body, 34.0, theme::BONE), Node { width: px(STAGE_W - 300.0), ..default() }),
            (
                Node { width: px(STAGE_W - 300.0), justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, ..default() },
                Pickable::IGNORE,
                children![
                    theme::text(format!("{at} / {}", tale.total.max(at)), &fonts.num, 26.0, theme::DIM),
                    (
                        Node { column_gap: px(24), ..default() },
                        Pickable::IGNORE,
                        children![
                            (
                                page_button(l10n.t("story-skip"), &fonts),
                                if last { Visibility::Hidden } else { Visibility::Inherited },
                                observe(|_: On<Activate>, mut commands: Commands| commands.trigger(Turn::Skip)),
                            ),
                            (page_button(l10n.t(onward), &fonts), observe(|_: On<Activate>, mut commands: Commands| commands.trigger(Turn::Next))),
                        ],
                    ),
                ],
            ),
        ],
    ));
    tale.shown = Some(root);
}
