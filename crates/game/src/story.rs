//! The tale: full-screen painted pages with a few lines of text, shown at the start of a run,
//! on entering each act and at the end. Which ending a player gets depends on how the run went,
//! and every ending leads back to the gate: the next run opens with a page that remembers it.
//!
//! A page's id is both its picture (`assets/story/<id>-gemini.png`) and its text (`story-<id>`).
//! Pages are drawn over whatever screen is open and swallow its input until read or skipped.
//! In a party the scenes of the run (the opening, each act) are read together: pressing a button
//! is a vote, and the page turns for everyone once every player has voted; if all of them voted
//! to skip, the scene is dropped. Endings differ from player to player and are read alone.

use crate::art::Art;
use crate::audio::{Mixer, Narration};
use crate::l10n::L10n;
use crate::battle::Btn;
use crate::net::{Net, NetStatus};
use crate::profile::Profile;
use crate::theme::{self, Fonts};
use crate::{AppState, Stage as Canvas, STAGE_H, STAGE_W};
use bevy::audio::Volume;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};
use nav_core::{CardType, Run, RunAction, Stage};
use nav_net::Note;
use std::collections::{HashMap, VecDeque};

/// Every ending there is, in the order the chronicle lists them.
pub const ENDINGS: [&str; 8] = ["fall_forest", "fall_village", "fall_nav", "fall_koschei", "win_dawn", "win_scarred", "win_throne", "win_circle"];

/// Curses in the deck at the moment of victory that turn the hero towards the throne.
const THRONE_CURSES: usize = 3;

pub fn plugin(app: &mut App) {
    app.init_resource::<Tale>()
        .init_resource::<PendingVoice>()
        .add_systems(Update, ((watch_run, turn_pages, tally_votes, show_page, want_book, book_hands, read_secret).chain(), speak_when_open, keep_pages_ready));
    #[cfg(feature = "dev")]
    app.add_systems(Startup, dev_scene);
    #[cfg(feature = "dev")]
    app.add_systems(Update, dev_next_act);
}

/// Dev: `TROPA_STORY=pro|again|again-<ending>|act2|act3|act4|<ending>` opens that scene at once,
/// `TROPA_STORY_PAGE=<n>` on its n-th page, to look at it or take a screenshot.
#[cfg(feature = "dev")]
fn dev_scene(mut tale: ResMut<Tale>) {
    let Ok(scene) = std::env::var("TROPA_STORY") else { return };
    let mut pages = match scene.as_str() {
        "pro" => vec![page("pro_1"), page("pro_2"), page("pro_3")],
        "again" => vec![Page { picture: "story/again".into(), text: "story-again".into(), title: None, closes: false, reread: false }],
        s if s.starts_with("again-") => vec![Page { picture: "story/again".into(), text: format!("story-{s}"), title: None, closes: false, reread: false }],
        s if s.starts_with("act") => vec![page(s)],
        s => ending_pages(s),
    };
    let skip = std::env::var("TROPA_STORY_PAGE").ok().and_then(|n| n.parse::<usize>().ok()).unwrap_or(1).saturating_sub(1);
    if pages.last().is_some_and(|p| secret_of(p).is_some()) {
        pages.push(end_leaf());
    }
    tale.total = pages.len();
    tale.pages.extend(pages.drain(skip.min(pages.len().saturating_sub(1))..));
}

/// Dev: `TROPA_TELL_ACT2=1` tells the second act's opening a little after the book is first shut,
/// as if the party had just crossed into it.
#[cfg(feature = "dev")]
fn dev_next_act(mut tale: ResMut<Tale>, time: Res<Time>, mut shut: Local<f32>, mut told: Local<bool>) {
    if *told || std::env::var("TROPA_TELL_ACT2").is_err() || tale.open() || tale.past.is_empty() {
        return;
    }
    *shut += time.delta_secs();
    if *shut > 3.0 {
        *told = true;
        tale.tell(vec![page("act2")]);
    }
}

struct Page {
    picture: String,
    /// Fluent key of the text.
    text: String,
    /// Fluent key of a heading, for the first page of an ending.
    title: Option<String>,
    /// The page belongs to an ending: its last button closes the tale rather than sends you off.
    closes: bool,
    /// The page the book was left open on, read before: it lies there silent.
    reread: bool,
}

/// Pages waiting to be read; the front one is on screen.
#[derive(Resource, Default)]
pub struct Tale {
    pages: VecDeque<Page>,
    /// How many pages the scene being read had when it began, for "2 / 3".
    total: usize,
    shown: Option<Entity>,
    /// Votes cast on each page, by its text key: player slot -> wants to skip.
    votes: HashMap<String, HashMap<u8, bool>>,
    /// The book was shut before the scene was read out.
    skipped: bool,
    /// Pages of this run already read, the last read last: turning back goes to them, and the
    /// numbering runs on past them. One book for the whole run.
    past: Vec<Page>,
    /// A new run has begun: the book forgets what was opened in it.
    new_run: bool,
    /// The scene now told opens the run: the book lies shut until the reader lifts its cover. Any
    /// later scene finds it lying open.
    shut: bool,
}

/// The quiet leaf a chapter ends on, when its last page was glued: a glued page turned without
/// parting it must have somewhere to turn to.
fn end_leaf() -> Page {
    page("leaf_end")
}

impl Tale {
    fn tell(&mut self, mut pages: Vec<Page>) {
        if pages.last().is_some_and(|p| secret_of(p).is_some()) {
            pages.push(end_leaf());
        }
        // the book opens where it was left, on the last page read, and the new ones lie beyond it
        if self.pages.is_empty() {
            if let Some(mut last) = self.past.pop() {
                last.reread = true;
                self.pages.push_back(last);
            }
        }
        self.pages.extend(pages);
        self.total = self.past.len() + self.pages.len();
    }

    pub fn open(&self) -> bool {
        !self.pages.is_empty()
    }
}

/// The page's reading, waiting for the book to open (and its volume).
#[derive(Resource, Default)]
struct PendingVoice(Option<(Handle<AudioSource>, f32)>);

fn speak_when_open(mut commands: Commands, mut pending: ResMut<PendingVoice>, book: Res<crate::book::Book>, tale: Res<Tale>, speaking: Query<(), With<Narration>>) {
    if !tale.open() {
        pending.0 = None;
        return;
    }
    if book.open < 1.0 || !speaking.is_empty() {
        return;
    }
    if let Some((voice, volume)) = pending.0.take() {
        commands.spawn((Narration, AudioPlayer::new(voice), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume))));
    }
}

/// A page turned back is told the others as its key behind this mark.
const BACK: &str = "<";

fn page(id: &str) -> Page {
    Page { picture: format!("story/{id}"), text: format!("story-{id}"), title: None, closes: false, reread: false }
}

/// The act whose glued secret lies under this page: the last page of each act's opening (the
/// prologue's, or the greeting of a reader come back, for the first act).
fn secret_of(page: &Page) -> Option<u8> {
    match page.text.as_str() {
        "story-pro_3" => Some(1),
        t if t.starts_with("story-again") => Some(1),
        "story-act2" => Some(2),
        "story-act3" => Some(3),
        "story-act4" => Some(4),
        _ => None,
    }
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

/// Players who read the tale together: connected people, not the in-process bots.
fn readers(net: &Net) -> Vec<u8> {
    let bots: Vec<_> = net.locals.iter().map(|n| n.me).collect();
    net.node.lobby.players.iter().enumerate().filter(|(_, p)| p.connected && !bots.contains(&p.peer)).map(|(i, _)| i as u8).collect()
}

/// The page is read together, and turned by vote.
fn shared(net: &Net, page: &Page) -> bool {
    !page.closes && readers(net).len() > 1
}

fn silent(net: &Net) -> bool {
    // the autopilot does not read, and a screenshot run must not be covered by a page
    net.bot_me || (cfg!(feature = "dev") && std::env::var("TROPA_SHOT").is_ok() && std::env::var("TROPA_STORY").is_err())
}

#[allow(clippy::too_many_arguments)]
fn watch_run(
    net: Option<Res<Net>>,
    db: Res<crate::session::Db>,
    state: Res<State<AppState>>,
    time: Res<Time>,
    mut tale: ResMut<Tale>,
    mut profile: ResMut<Profile>,
    mut told: Local<Told>,
    mut settled: Local<f32>,
) {
    let Some(net) = net else {
        *told = Told::default();
        return;
    };
    let Some(run) = net.node.state.as_ref() else { return };
    if told.seed != Some(run.seed) {
        *told = Told { seed: Some(run.seed), act: run.act, ended: false };
        tale.past.clear();
        tale.new_run = true;
        // a run picked up from a save has had its opening already
        let fresh = run.act <= 1 && run.floors_cleared == 0 && run.at.is_none();
        if fresh && !silent(&net) {
            // read together, the opening must be the same page for all: a party always hears the prologue
            let party = readers(&net).len() > 1;
            tale.shut = true;
            match profile.last_ending.clone() {
                _ if party => tale.tell(vec![page("pro_1"), page("pro_2"), page("pro_3")]),
                Some(last) if profile.prologue_seen => {
                    tale.tell(vec![Page { picture: "story/again".into(), text: format!("story-again-{last}"), title: None, closes: false, reread: false }]);
                }
                _ if profile.prologue_seen => tale.tell(vec![Page { picture: "story/again".into(), text: "story-again".into(), title: None, closes: false, reread: false }]),
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
        // the last fight plays out and the end of the road is seen before the book opens on it
        if *state.get() != AppState::RunEnd {
            *settled = 0.0;
            return;
        }
        *settled += time.delta_secs();
        if *settled < 1.5 {
            return;
        }
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
        Node { padding: UiRect::axes(px(22), px(5)), border: UiRect::all(px(2)), justify_content: JustifyContent::Center, ..default() },
        BackgroundColor(theme::PEAT),
        BorderColor::all(Color::srgb(0.42, 0.33, 0.22)),
        children![theme::text(label, &fonts.title, 20.0, theme::BONE)],
    )
}

/// Pressed on a page: read on, or drop the rest of the scene.
#[derive(Event)]
enum Turn {
    Next,
    Skip,
    /// The reader turned the leaf by hand. A page read together turns for everyone at once,
    /// with no vote: whoever turns the leaf turns it for all.
    Leaf,
    /// Another reader turned the leaf by hand.
    TheirLeaf,
    /// The reader turned the left-hand page back, by hand or key: back a page.
    Back,
    /// Another reader turned a page back.
    TheirBack,
}

fn turn_pages(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut turned: MessageReader<crate::book::Turned>,
    mut turned_back: MessageReader<crate::book::TurnedBack>,
    mut torn: MessageReader<crate::book::Torn>,
    mut found: MessageReader<crate::book::Found>,
    mut tale: ResMut<Tale>,
    mut profile: ResMut<Profile>,
    net: Option<Res<Net>>,
    mut run_outbox: ResMut<crate::session::RunOutbox>,
    mut book: ResMut<crate::book::Book>,
) {
    // a secret peeled open whole by this reader: its relic, and the chronicle remembers it
    for f in found.read() {
        if f.whole && f.mine {
            run_outbox.0.push(RunAction::FindSecret { act: f.act });
            if !net.as_ref().is_some_and(|n| n.ephemeral) {
                profile.note_secret(f.act);
                profile.save();
            }
        }
    }
    // the book shut before its chapter was done: the unheard tale pays one who has it
    if std::mem::take(&mut tale.skipped) {
        let mine = net.as_ref().and_then(|n| Some((n.node.state.as_ref()?, n.node.my_slot()?)));
        if let Some((run, me)) = mine {
            if run.players.get(me as usize).is_some_and(|p| p.relics.iter().any(|r| r == nav_core::SECRET_RELICS[3])) {
                run_outbox.0.push(RunAction::SkipTale);
            }
        }
    }
    // what is torn from the tale is torn from the one who held it: the curse «Порча книги»,
    // and max HP for a great piece (the run decides, see `RunAction::TearPage`)
    for t in torn.read() {
        if t.mine {
            let lost = (t.lost * 100.0).ceil().clamp(1.0, 100.0) as u8;
            run_outbox.0.push(RunAction::TearPage { lost });
        }
    }
    if !tale.open() {
        turned.clear();
        turned_back.clear();
        return;
    }
    // the key that lifts the cover does not also turn the page under it, whichever system hears
    // it first: nothing turns until the book lies open
    if book.open < 1.0 {
        return;
    }
    // the reader turned the leaf by hand
    if turned.read().count() > 0 {
        commands.trigger(Turn::Leaf);
    } else if turned_back.read().count() > 0 {
        commands.trigger(Turn::Back);
    } else if keys.any_just_pressed([KeyCode::ArrowLeft, KeyCode::Backspace]) {
        // back from a secret is the book's own affair: the story stays on the page it lies under
        if !book.on_secret() {
            commands.trigger(Turn::Back);
        } else {
            book.turn_back_unseen();
        }
    } else if keys.any_just_pressed([KeyCode::Space, KeyCode::Enter, KeyCode::ArrowRight]) {
        commands.trigger(Turn::Next);
    } else if keys.just_pressed(KeyCode::Escape) {
        commands.trigger(Turn::Skip);
    }
}

fn on_turn(turn: On<Turn>, mut commands: Commands, mut tale: ResMut<Tale>, net: Option<ResMut<Net>>, speaking: Query<Entity, With<Narration>>, book: Res<crate::book::Book>) {
    // a button pressed while the book still lies shut, or is opening
    if book.open < 1.0 && matches!(*turn, Turn::Next | Turn::Skip) {
        return;
    }
    let skip = matches!(*turn, Turn::Skip);
    let by_hand = matches!(*turn, Turn::Leaf | Turn::TheirLeaf);
    // back a page: for everyone, when the page is read together
    if matches!(*turn, Turn::Back | Turn::TheirBack) {
        if tale.past.is_empty() {
            return;
        }
        if let (Some(mut net), Some(page)) = (net, tale.pages.front()) {
            if shared(&net, page) && matches!(*turn, Turn::Back) {
                net.node.book_turn(&format!("{BACK}{}", page.text));
            }
        }
        for e in &speaking {
            commands.entity(e).despawn();
        }
        if let Some(page) = tale.pages.front() {
            let key = page.text.clone();
            tale.votes.remove(&key);
        }
        if let Some(page) = tale.past.pop() {
            tale.pages.push_front(page);
        }
        if let Some(e) = tale.shown.take() {
            commands.entity(e).despawn();
        }
        return;
    }
    if let (Some(mut net), Some(page)) = (net, tale.pages.front()) {
        let together = shared(&net, page);
        let key = page.text.clone();
        // a leaf turned by hand turns for all, with no vote; the others are told
        if by_hand {
            if together && matches!(*turn, Turn::Leaf) {
                net.node.book_turn(&key);
            }
            tale.votes.remove(&key);
        } else if together {
            // in a party the button is a vote; the page turns once everyone has voted
            net.node.story_vote(&key, skip);
            return;
        }
    }
    advance(&mut commands, &mut tale, &speaking, skip);
}

fn advance(commands: &mut Commands, tale: &mut Tale, speaking: &Query<Entity, With<Narration>>, skip: bool) {
    // the storyteller stops mid-word when the page is turned
    for e in speaking {
        commands.entity(e).despawn();
    }
    if skip {
        tale.skipped = tale.pages.len() > 1;
        // skipped, not gone: they keep their place in the book
        while let Some(page) = tale.pages.pop_front() {
            tale.past.push(page);
        }
    } else if let Some(page) = tale.pages.pop_front() {
        tale.past.push(page);
    }
    if let Some(e) = tale.shown.take() {
        commands.entity(e).despawn();
    }
}

/// The "who has voted" line on a shared page.
#[derive(Component)]
struct VoteLabel;

/// Counts the party's votes on the page being read and turns it when everyone has voted.
fn tally_votes(
    mut commands: Commands,
    mut tale: ResMut<Tale>,
    net: Option<Res<Net>>,
    status: Option<ResMut<NetStatus>>,
    l10n: Res<L10n>,
    speaking: Query<Entity, With<Narration>>,
    mut labels: Query<&mut Text, With<VoteLabel>>,
) {
    if let Some(mut status) = status {
        for (from, page, skip) in status.story_votes.drain(..) {
            tale.votes.entry(page).or_default().insert(from, skip);
        }
    }
    let Some(net) = net else { return };
    let Some(page) = tale.pages.front() else { return };
    if !shared(&net, page) {
        return;
    }
    let readers = readers(&net);
    let cast = tale.votes.get(&page.text).cloned().unwrap_or_default();
    let voted: Vec<bool> = readers.iter().filter_map(|r| cast.get(r).copied()).collect();
    for mut text in &mut labels {
        text.0 = format!("{} {} / {}", l10n.t("story-votes"), voted.len(), readers.len());
    }
    if voted.len() < readers.len() {
        return;
    }
    let skip = voted.iter().all(|s| *s);
    let key = page.text.clone();
    tale.votes.remove(&key);
    advance(&mut commands, &mut tale, &speaking, skip);
}

/// Carries the hands over a page read together: this reader's out to the others, theirs into
/// the book, and a leaf they turned by hand turns the page here.
fn book_hands(mut commands: Commands, tale: Res<Tale>, net: Option<ResMut<Net>>, status: Option<ResMut<NetStatus>>, mut book: ResMut<crate::book::Book>) {
    use crate::book::HandNews;
    let news = std::mem::take(&mut book.news);
    let (Some(mut net), Some(mut status)) = (net, status) else { return };
    let notes = std::mem::take(&mut status.book);
    let Some(page) = tale.pages.front().map(|p| p.text.clone()) else { return };
    book.seat = net.node.my_slot().unwrap_or(0);
    for n in news {
        match n {
            HandNews::Hand { at, hold } => net.node.book_hand(&page, at, hold),
            HandNews::Tear(edge) => net.node.book_tear(&page, edge),
            HandNews::Carry { piece, point, at } => net.node.book_carry(&page, piece, point, at),
        }
    }
    let colour = |seat: u8| {
        net.node.state.as_ref().and_then(|run| run.players.get(seat as usize)).map(|p| theme::hero_color(&p.hero)).unwrap_or(theme::BONE)
    };
    for n in notes {
        match n {
            Note::BookHand { from, page: p, at, hold } if p == page => {
                let c = colour(from);
                book.remote_hand(from, c, at, hold);
            }
            Note::BookTear { page: p, edge, .. } if p == page => book.remote_tear(edge),
            Note::BookCarry { from, page: p, piece, point, at } if p == page => {
                let c = colour(from);
                book.remote_carry(from, c, piece, point, at);
            }
            Note::BookTurn { page: p, .. } if p == page => commands.trigger(Turn::TheirLeaf),
            Note::BookTurn { page: p, .. } if p.strip_prefix(BACK) == Some(page.as_str()) => commands.trigger(Turn::TheirBack),
            _ => {}
        }
    }
}

/// A secret peeled open whole is read aloud, like any page of the tale: the storyteller's own
/// voice, telling his own story. Whatever he was reading stops.
#[allow(clippy::too_many_arguments)]
fn read_secret(
    mut commands: Commands,
    mut found: MessageReader<crate::book::Found>,
    speaking: Query<Entity, With<Narration>>,
    l10n: Res<L10n>,
    profile: Res<Profile>,
    mixer: Res<Mixer>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    for f in found.read() {
        if !f.whole || mixer.narrator <= 0.0 {
            continue;
        }
        let spoken = format!("voice/{}/story-secret-{}.wav", l10n.lang.voice(profile.settings.voice_lang).code(), f.act);
        if art.root().join(&spoken).is_file() {
            for e in &speaking {
                commands.entity(e).despawn();
            }
            commands.spawn((Narration, AudioPlayer::new(assets.load::<AudioSource>(spoken)), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(mixer.narrator))));
        }
    }
}

/// The paintings of the pages still to be read are read in ahead of the turning that shows them,
/// and each page's secret with it; once read they are let go.
fn keep_pages_ready(tale: Res<Tale>, art: Res<Art>, assets: Res<AssetServer>, mut held: Local<(Vec<String>, Vec<Handle<Image>>)>) {
    let stems: Vec<String> = tale.pages.iter().flat_map(|p| std::iter::once(p.picture.clone()).chain(secret_of(p).map(|act| format!("story/secret_{act}")))).collect();
    if stems != held.0 {
        held.1 = stems.iter().filter_map(|s| art.image(&assets, s)).collect();
        held.0 = stems;
    }
}

/// The book lies on the table while there is a tale to read.
fn want_book(mut tale: ResMut<Tale>, mut book: ResMut<crate::book::Book>) {
    if std::mem::take(&mut tale.new_run) {
        book.forget_run();
    }
    if book.wanted != tale.open() {
        book.wanted = tale.open();
        let shut = std::mem::take(&mut tale.shut);
        if book.wanted && !shut {
            book.lay_open();
        }
    }
}

/// Page numbers of the spread at this place in a scene (1 the first), counting two pages hidden
/// under each glued leaf before it. Left pages are even, as in any book.
fn page_numbers(at: usize, glued_before: usize) -> (u32, u32) {
    let left = 2 * (at + glued_before) as u32;
    (left, left + 1)
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
    mixer: Res<Mixer>,
    profile: Res<Profile>,
    net: Option<Res<Net>>,
    mut book: ResMut<crate::book::Book>,
) {
    if !*hooked {
        *hooked = true;
        commands.add_observer(on_turn);
    }
    if tale.shown.is_some() {
        return;
    }
    let Some(page) = tale.pages.front() else { return };
    // the page read aloud, in the language chosen for speech; a page with no recording is silent
    let spoken = format!("voice/{}/{}.wav", l10n.lang.voice(profile.settings.voice_lang).code(), page.text);
    // the storyteller waits for the book to lie open before speaking
    if !page.reread && mixer.narrator > 0.0 && art.root().join(&spoken).is_file() {
        commands.insert_resource(PendingVoice(Some((assets.load::<AudioSource>(spoken), mixer.narrator))));
    }
    let together = net.as_ref().is_some_and(|n| shared(n, page));
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
    // the book on the table, rendered in 3D; the page's painting and text are printed in it, and
    // the next page waits under the leaf for the reader to turn it. The last page of an act's
    // opening is glued over that act's secret: the only sign is the numbering, which jumps by two
    let spread = |page: &Page, at: usize, glued_before: usize| {
        let numbers = page_numbers(at, glued_before);
        let secret = secret_of(page).map(|act| {
            let hidden = crate::book::Spread {
                picture: format!("story/secret_{act}"),
                title: None,
                text: l10n.t(&format!("story-secret-{act}")),
                numbers: (numbers.0 + 2, numbers.1 + 2),
                secret: None,
                peeled: true,
            };
            (act, Box::new(hidden))
        });
        crate::book::Spread { picture: page.picture.clone(), title: page.title.as_ref().map(|t| l10n.t(t)), text: l10n.t(&page.text), numbers, secret, peeled: false }
    };
    let glued = |pages: &[Page]| pages.iter().filter(|p| secret_of(p).is_some()).count();
    let before = glued(&tale.past);
    book.spread = spread(page, at, before);
    book.pages_ahead = tale.pages.len().saturating_sub(1);
    book.next = tale.pages.get(1).map(|p| spread(p, at + 1, before + secret_of(page).is_some() as usize));
    book.prev = tale.past.last().map(|p| spread(p, at - 1, before - secret_of(p).is_some() as usize));
    book.drag_ok = true;
    book.ending = page.closes;
    book.shared = together;
    commands.spawn((
        Node { position_type: PositionType::Absolute, width: px(STAGE_W), height: px(STAGE_H), ..default() },
        ImageNode::new(book.view.clone()),
        Pickable::IGNORE,
        ChildOf(root),
    ));
    // the text sits on a dark band along the bottom, like a caption under a painting
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            bottom: px(0),
            width: px(STAGE_W),
            padding: UiRect::new(px(150), px(150), px(8), px(10)),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(theme::SOOT.with_alpha(0.55)),
        Pickable::IGNORE,
        ChildOf(root),
        children![
            (
                Node { width: px(STAGE_W - 300.0), justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, ..default() },
                Pickable::IGNORE,
                children![
                    (
                        Node { column_gap: px(30), align_items: AlignItems::Center, ..default() },
                        Pickable::IGNORE,
                        children![
                            theme::text(format!("{at} / {}", tale.total.max(at)), &fonts.num, 26.0, theme::DIM),
                            (theme::text("", &fonts.body, 26.0, theme::CANDLE), VoteLabel, if together { Visibility::Inherited } else { Visibility::Hidden }),
                        ],
                    ),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_are_numbered_as_in_a_book() {
        assert_eq!(page_numbers(1, 0), (2, 3));
        assert_eq!(page_numbers(2, 0), (4, 5));
        // past a glued leaf the two pages hidden under it are skipped: 3 -> 6
        assert_eq!(page_numbers(2, 1), (6, 7));
        assert_eq!(page_numbers(3, 1), (8, 9));
    }

    #[test]
    fn the_last_page_of_each_act_opening_is_glued() {
        assert_eq!(secret_of(&page("pro_3")), Some(1));
        assert_eq!(secret_of(&page("pro_1")), None);
        assert_eq!(secret_of(&page("act2")), Some(2));
        assert_eq!(secret_of(&page("fall_nav_1")), None);
    }

    #[test]
    fn a_tale_told_in_two_parts_counts_its_pages_on() {
        let mut t = Tale::default();
        t.tell(vec![page("pro_1")]);
        t.tell(vec![page("pro_2")]);
        assert_eq!((t.total, t.pages.len()), (2, 2));
        assert!(t.open());
    }

    #[test]
    fn the_next_act_opens_on_the_last_page_read_and_counts_on() {
        let mut t = Tale::default();
        t.tell(vec![page("pro_1"), page("pro_2"), page("pro_3")]);
        while let Some(p) = t.pages.pop_front() {
            t.past.push(p);
        }
        t.tell(vec![page("act2")]);
        assert_eq!(t.pages.front().map(|p| (p.text.as_str(), p.reread)), Some(("story-leaf_end", true)));
        assert_eq!(t.pages.get(1).map(|p| p.text.as_str()), Some("story-act2"));
        assert_eq!(t.pages.get(2).map(|p| p.text.as_str()), Some("story-leaf_end"), "a glued page always has a leaf after it");
        assert_eq!((t.past.len(), t.total), (3, 6));
    }
}
