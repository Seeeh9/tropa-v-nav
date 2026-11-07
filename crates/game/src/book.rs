//! The book of the tale: a leather chronicle on a candlelit oak table, drawn in 3D.
//!
//! The scene is rendered into a picture (`Book::view`) that the story page shows in place of its
//! flat painting, so it sits in the scaled stage like any other UI. Every printed face of paper is
//! a picture too: a little UI tree (parchment, framed painting, printed text) rendered by its own
//! camera into a texture, which the meshes wear.
//!
//! Pages turn as real paper does: the leaf being turned is a cloth of particles (`leaf`) held at
//! the spine. The reader takes hold of the right-hand page anywhere and carries it over; let go
//! past the spine, it falls onto the left and the story reads on, let go too early, it falls back.
//! When the story turns the page itself (a button, a key), an unseen hand turns the leaf.

use crate::art::Art;
use crate::audio::Mixer;
use crate::theme::Fonts;
use bevy::asset::RenderAssetUsages;
use bevy::audio::Volume;
use bevy::camera::RenderTarget;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::picking::pointer::{PointerId, PointerLocation, PointerPressState};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::sprite::{BorderRect, SliceScaleMode, TextureSlicer};
use bevy::ui::widget::NodeImageMode;
use bevy::ui::UiTargetCamera;
use leaf::{Floor, Leaf, NX, NZ};
use std::f32::consts::PI;

mod leaf;

/// Size of the rendered scene, in pixels: the stage itself.
const VIEW: UVec2 = UVec2::new(1920, 1080);
/// Size of one page's texture; pages are 1 x PAGE_H in the scene.
const PAGE_PX: UVec2 = UVec2::new(1024, 1400);
const PAGE_W: f32 = 1.0;
const PAGE_H: f32 = 1.37;
/// Gap between the spine and the inner edge of a page.
const GUTTER: f32 = 0.005;
/// Height of the top of the page block above the table.
const TOP: f32 = 0.09;
/// How far from the hand's line of sight a leaf in the air is still within its reach.
const CATCH_REACH: f32 = PAGE_W * 0.22;
/// How high over the book a leaf may be and still count as down.
const NEARLY_DOWN: f32 = 0.04;
/// Where a leaf lies: on the right-hand page, on the left-hand one, and on the table round the book.
const LEAF_FLOOR: Floor = Floor { right: TOP + 0.004, left: TOP + 0.009, half: (PAGE_W + 0.06, PAGE_H / 2.0 + 0.04) };
/// Distance from the edge of the frame texture to its inner window, in texture pixels.
const FRAME_SLICE: f32 = 380.0;
/// Where the reader's eye rests, and the point of the spread it looks at.
const EYE: Vec3 = Vec3::new(0.0, 1.66, 1.02);
const LOOK: Vec3 = Vec3::new(0.0, 0.0, 0.16);
/// How long the unseen hand takes to carry a leaf over, in seconds.
const AUTO_TURN: f32 = 0.9;
/// Steps a second the leaf's paper is run at.
const LEAF_RATE: f32 = 60.0;
/// How often a reader's hand is told to the others, per second.
const HAND_RATE: f32 = 20.0;
/// How long another reader's hand stays in sight after their machine last told of it, seconds.
const GHOST_LINGER: f32 = 1.5;
/// How much more a pull past the paper's reach counts than the mouse moved.
const PULL: f32 = 2.5;
/// How far past the paper's reach the hand goes before it pulls at the paper, in page widths.
const SLACK: f32 = 0.9;
/// Past this share of its paper lost, a torn leaf is no page any more.
const WHOLE_PAGE_GONE: f32 = 0.5;
/// A leaf let go swinging at least this fast (sides a second) goes that way, wherever it is.
const FLICK: f32 = 0.5;
/// A leaf lying flatter than this over one side lies there; less, it hangs in the air.
const LIES_FLAT: f32 = 0.8;
/// Stage pixels at the foot of the screen that belong to the story's buttons, not the book.
const BUTTON_BAND: f32 = 90.0;

pub fn plugin(app: &mut App) {
    #[cfg(feature = "dev")]
    app.add_systems(Update, dev_ghost.before(follow_ghosts));
    app.add_message::<Turned>()
        .add_message::<TurnedBack>()
        .add_message::<Torn>()
        .add_message::<Found>()
        .add_systems(Startup, setup)
        .add_systems(Update, keep_seal)
        .add_systems(Update, ((open_cover, follow_story, hold_leaf, follow_ghosts, carry_remote, move_leaf, redraw_pages, keep_scraps, run_scraps).chain(), flicker, sway_camera, fit_pages));
}

/// Sent when the reader has turned the page by hand: the story reads on.
#[derive(Message)]
pub struct Turned;

/// Sent when the reader has turned the left-hand page back by hand: the story goes back a page.
#[derive(Message)]
pub struct TurnedBack;

/// Sent when a torn leaf has come to rest having lost more paper: `lost` is the share of the
/// leaf newly cut loose from the spine, and `mine` says this reader's hand held it, so the cost
/// is theirs. A leaf torn in another reader's hands tears here too, but costs them, not us.
#[derive(Message)]
pub struct Torn {
    pub lost: f32,
    pub mine: bool,
}

/// What the open spread shows.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Spread {
    /// Painting on the left page, an asset stem such as `story/pro_1`.
    pub picture: String,
    /// Heading over the painting, already translated.
    pub title: Option<String>,
    /// The page's text, already translated.
    pub text: String,
    /// Page numbers printed at the foot of the two pages.
    pub numbers: (u32, u32),
    /// The right-hand page is two leaves glued together, and between them lies this act's
    /// secret spread. Taken up like any page, the glued pair turns over as one and the secret is
    /// never seen; the only sign is the numbers jumping, 3 then 6. To part them a corner must first be worked
    /// loose: alone by rubbing at it with a nail, in a party by two readers pulling at opposite
    /// corners together. Then a slow pull peels the leaf off the secret; a jerk tears it.
    pub secret: Option<(u8, Box<Spread>)>,
    /// This spread was glued shut: each page bears, mirrored and faint, ink torn from the other.
    pub peeled: bool,
}

/// Sent when a reader has peeled a glued leaf apart and its secret spread lies open. `whole`
/// says the leaf came away untorn (a torn one loses its secret); `mine`, that this reader's
/// hand held it.
#[derive(Message)]
pub struct Found {
    pub act: u8,
    pub whole: bool,
    pub mine: bool,
}

/// One side of a spread.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Half {
    Left,
    Right,
}

/// The faces of paper in the scene, each wearing its own picture.
#[derive(Clone, Copy, PartialEq)]
enum Face {
    /// The left-hand page, glued under the cover.
    Left,
    /// The right-hand page, on the block.
    Right,
    /// The leaf being turned: the side that faces up while it lies on the right...
    LeafFront,
    /// ...and the side that faces up once it lies on the left.
    LeafBack,
}

const FACES: [Face; 4] = [Face::Left, Face::Right, Face::LeafFront, Face::LeafBack];

/// What the reader's hand did over a page read together, for the story to tell the others.
pub enum HandNews {
    /// Where the hand is in the scene, and the point of the leaf it holds, if any.
    Hand { at: [f32; 3], hold: Option<u16> },
    /// The leaf the reader holds tore along this edge.
    Tear(u16),
    /// The reader carries torn paper (`piece` 0 the leaf in play, else a scrap's age) by `point`,
    /// or has let it go.
    Carry { piece: u32, point: Option<u16>, at: [f32; 3] },
}

/// Another reader's hand over the book, as their machine last told it.
struct Ghost {
    at: Vec3,
    /// How fast it was moving between the last two tellings: between tellings it is carried on
    /// at that speed, so the paper it holds is drawn along smoothly and not jerked.
    speed: Vec3,
    /// The point of the leaf it holds.
    hold: Option<usize>,
    /// The leaf it holds is being peeled off its secret.
    peeling: bool,
    /// The glued corner it pulls at.
    tug: Option<usize>,
    color: Color,
    /// Seconds since it was last heard of.
    quiet: f32,
}

/// Who is turning the leaf.
enum Hand {
    /// The reader, with the mouse.
    Reader,
    /// Another reader, whose machine sends where their hand is (their seat).
    Remote(u8),
    /// The story, by its button: carried over in an arc, `t` from 0 to 1.
    Unseen { t: f32, point: usize, z: f32 },
    /// The reader, carrying a piece torn loose: it goes wherever the hand goes.
    Loose,
    /// Nobody: the leaf falls where it will.
    Gone,
    /// Another reader, carrying the leaf torn loose (their seat).
    Carried(u8),
    /// The reader, working at a corner of a glued leaf to get it loose.
    Nail,
}

struct Turning {
    /// The spread the leaf belongs to, and the one under it.
    from: Spread,
    to: Spread,
    /// The story has read on: the leaf must end up on the left.
    committed: bool,
    hand: Hand,
    /// Seconds since the leaf was let go: paper caught on the edge of the book may never quite
    /// come to rest, so after a while it is taken as lying where it lies.
    loose: f32,
    /// This reader's hand held the leaf on this turn.
    mine: bool,
    /// The leaf is the top of a glued pair being peeled off its secret (that act's).
    peel: Option<u8>,
    /// The leaf is the left-hand page, turned back over to the right.
    back: bool,
    /// The turn is the book's own: it goes between a page and the secret under it, and the story
    /// knows nothing of it.
    own: bool,
    /// The story's page this turn leads to: `to`, except when turned back onto a secret, which
    /// lies open in the story's page before.
    story_to: Spread,
}

/// What came of a leaf coming to rest.
#[derive(Default, Debug, PartialEq)]
struct Settled {
    /// A secret peeled open: its act, whether whole, whether this reader had a hand in it.
    found: Option<(u8, bool, bool)>,
    /// Paper newly lost, and whether it was in this reader's hand.
    torn: Option<(f32, bool)>,
    /// The page was torn clean out: what lay under it is where the story now is.
    read_on: bool,
}

/// Strokes of a nail that work a glued corner loose, how far each must go, and how long two
/// readers must pull at opposite corners to do the same.
const STROKES: f32 = 6.0;
const STROKE: f32 = 0.02;
const TUG_TIME: f32 = 1.5;
/// How high the worked corner has curled by the time it comes loose.
const CURL: f32 = 0.05;
/// How long a hand that took a glued leaf off to one side of the cursor eases onto it, seconds.
const EASE: f32 = 0.6;
/// Marks on the point a hand's news tells of: it is pulling at a glued corner, not turning the
/// leaf; or it holds a leaf being peeled.
const TUG_BIT: u16 = 0x8000;
const PEEL_BIT: u16 = 0x4000;

/// A nail rubbing back and forth: counts the strokes.
#[derive(Default)]
struct Rub {
    last: Option<Vec3>,
    heading: Vec2,
    stroke: f32,
}

impl Rub {
    /// The nail has moved here: was a stroke just finished, by turning back after going far enough?
    fn to(&mut self, at: Vec3) -> bool {
        let Some(last) = self.last else {
            self.last = Some(at);
            return false;
        };
        let d = Vec2::new(at.x - last.x, at.z - last.z);
        if d.length() < 1e-4 {
            return false;
        }
        self.last = Some(at);
        let turned = self.heading.dot(d) < 0.0;
        let done = turned && self.stroke >= STROKE;
        self.stroke = if turned { d.length() } else { self.stroke + d.length() };
        self.heading = d;
        done
    }
}

/// Is this place on the right-hand page one of the free corners, where a nail can get under it?
fn at_corner(hit: Vec3) -> bool {
    hit.x > GUTTER + PAGE_W * 0.72 && hit.x < GUTTER + PAGE_W + 0.02 && hit.z.abs() > PAGE_H * 0.3 && hit.z.abs() < PAGE_H / 2.0 + 0.02
}

/// Do two hands on a leaf (grid points) pull from different sides of it: head against foot?
fn pulled_apart(a: (usize, usize), b: (usize, usize)) -> bool {
    a.0.abs_diff(b.0) + a.1.abs_diff(b.1) >= leaf::NZ / 2
}

/// How far the cover has swung on its spine, 0 shut to 1 lying open.
fn cover_turn(open: f32) -> Quat {
    let t = open.clamp(0.0, 1.0);
    Quat::from_rotation_z(t * t * (3.0 - 2.0 * t) * PI)
}

/// How long a let-go leaf may keep stirring before it is taken as come to rest, in seconds.
const SETTLE: f32 = 4.0;
/// How soon a whole leaf lying flat on one side after being let go is taken as at rest, seconds.
const LANDED: f32 = 0.4;
/// How near the hand must come to loose paper to pick it up, and how high it carries it.
const LOOSE_REACH: f32 = 0.06;
const LIFT: f32 = 0.07;

#[derive(Resource)]
pub struct Book {
    /// The scene as a picture, for the story page to show.
    pub view: Handle<Image>,
    /// Is the book wanted on the table: the story sets this while a tale is being read.
    pub wanted: bool,
    /// What the story wants on the spread.
    pub spread: Spread,
    /// The spread after this one, when there is one: what turning the leaf uncovers.
    pub next: Option<Spread>,
    /// How many of the story's pages lie beyond `spread`.
    pub pages_ahead: usize,
    /// The spread before this one, when the tale has one: what turning the left-hand page back
    /// over uncovers.
    pub prev: Option<Spread>,
    /// May the reader turn the leaf by hand.
    pub drag_ok: bool,
    /// Is the page read together: the reader's hand is shown to the others through `news`.
    pub shared: bool,
    /// What the story is to send the others, oldest first.
    pub news: Vec<HandNews>,
    /// The other readers' hands, by seat.
    ghosts: [Option<Ghost>; 4],
    /// Torn paper another reader carries, as last told: seat, piece, point, where, seconds since.
    carried: Option<(u8, u32, u16, Vec3, f32)>,
    /// Paper another reader let go, waiting to fall: the piece.
    dropped: Vec<u32>,
    /// The spread the pages beneath show.
    shown: Option<Spread>,
    /// The story's spread the book has last followed.
    followed: Option<Spread>,
    /// Acts whose glued secret has been peeled open while this book lies open.
    opened: Vec<u8>,
    /// What each face is to wear, and what it wears now (indexed like `FACES`).
    faces: [Option<(Spread, Half)>; 4],
    drawn: [Option<(Spread, Half)>; 4],
    turn: Option<Turning>,
    leaf: Leaf,
    /// A torn leaf lies where it came to rest, in place of a whole page.
    kept: bool,
    /// Share of the leaf whose loss has been paid for.
    charged: f32,
    /// Time not yet run by the leaf's paper (see `LEAF_RATE`).
    leaf_clock: f32,
    /// Page numbers torn out of the book for good: they lie bare from now on, this whole run.
    gone: Vec<u32>,
    /// Order in which paper came to lie where it lies, and the place of the leaf in play in it.
    paper_tick: u32,
    leaf_layer: u32,
    /// How fast the leaf in the hand is swinging over the spine, sides a second.
    swing: f32,
    last_side: Option<f32>,
    /// Torn leaves set aside for a fresh one, with what their two faces wear: they stay in the
    /// scene as scraps (`keep_scraps`).
    scraps_out: Vec<(Leaf, (Spread, Half), (Spread, Half))>,
    /// Pages that have come to show something new since the scraps last heard: whatever scrap
    /// lay on them now lies under the new leaf, out of sight.
    covered: Vec<Half>,
    leaf_meshes: (Handle<Mesh>, Handle<Mesh>),
    /// The fibres along the leaf's tears.
    fringe: Handle<Mesh>,
    /// 0 closed .. 1 open.
    pub open: f32,
    /// The cover's sounds have been played for this opening.
    heard: bool,
    /// The reader has lifted the cover: the book lies shut on the table until they do.
    pub lifted: bool,
    /// Acts whose glued corner has been worked loose while this book lies open.
    loose: Vec<u8>,
    /// How far the corner of the glued page has been worked, 0..1.
    worked: f32,
    /// The nail at work.
    rub: Rub,
    /// This reader helped work the corner loose: what is found under it is theirs too.
    helped: bool,
    /// Seconds the hand is still easing onto where the cursor is: it took the paper by a point
    /// off to one side of it, and must not snatch at it to get there.
    easing: f32,
    /// This reader's seat, to settle which of two hands peels a leaf they loosened together.
    pub seat: u8,
    /// The hand is over a glued corner, or at work on it: the story says what can be done.
    pub over_glue: bool,
    /// The tale is at its end: there is nothing left to be sealed.
    pub ending: bool,
}

/// Dev: what the book does goes to `book.log` beside the game, to read after a bug seen in play.
#[cfg(feature = "dev")]
fn note(line: String) {
    use std::io::Write;
    use std::sync::Once;
    static FRESH: Once = Once::new();
    let path = crate::art::assets_dir().join("../book.log");
    FRESH.call_once(|| {
        let _ = std::fs::remove_file(&path);
    });
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{line}");
    }
}

#[cfg(not(feature = "dev"))]
fn note(line: String) {
    debug!("book: {line}");
}

impl Book {
    fn state(&self) -> String {
        let n = |s: &Option<Spread>| s.as_ref().map_or("-".to_string(), |s| format!("{}|{}{}", s.numbers.0, s.numbers.1, if s.peeled { " secret" } else { "" }));
        let turn = self.turn.as_ref().map_or("none".to_string(), |t| format!("{}|{} back={} own={} peel={:?} committed={}", t.to.numbers.0, t.to.numbers.1, t.back, t.own, t.peel, t.committed));
        format!("story {}|{} shown {} next {} prev {} turn [{turn}] side {:.2} torn {} kept {} opened {:?} loose {:?}", self.spread.numbers.0, self.spread.numbers.1, n(&self.shown), n(&self.next), n(&self.prev), self.leaf.side(), self.leaf.torn(), self.kept, self.opened, self.loose)
    }

    fn new(view: Handle<Image>, leaf: Leaf, leaf_meshes: (Handle<Mesh>, Handle<Mesh>), fringe: Handle<Mesh>) -> Self {
        Book {
            view,
            wanted: false,
            spread: Spread::default(),
            next: None,
            pages_ahead: 0,
            prev: None,
            drag_ok: false,
            shared: false,
            news: Vec::new(),
            ghosts: Default::default(),
            carried: None,
            dropped: Vec::new(),
            shown: None,
            followed: None,
            opened: Vec::new(),
            faces: Default::default(),
            drawn: Default::default(),
            turn: None,
            leaf,
            kept: false,
            charged: 0.0,
            leaf_clock: 0.0,
            gone: Vec::new(),
            paper_tick: 0,
            leaf_layer: 0,
            swing: 0.0,
            last_side: None,
            scraps_out: Vec::new(),
            covered: Vec::new(),
            leaf_meshes,
            fringe,
            open: 0.0,
            heard: false,
            lifted: false,
            loose: Vec::new(),
            worked: 0.0,
            rub: Rub::default(),
            helped: false,
            easing: 0.0,
            seat: 0,
            over_glue: false,
            ending: false,
        }
    }

    /// How many of the story's pages lie beyond this spread, if it is near enough to tell: a secret
    /// lies between its glued page and the next.
    fn ahead_of(&self, s: &Spread) -> Option<usize> {
        let holds = |page: &Spread| *page == *s || page.secret.as_ref().is_some_and(|(_, inner)| **inner == *s);
        if holds(&self.spread) {
            Some(self.pages_ahead)
        } else if self.next.as_ref().is_some_and(holds) {
            Some(self.pages_ahead.saturating_sub(1))
        } else {
            None
        }
    }

    /// Is the right-hand page lying open under any leaf in the air the last that can be reached. A
    /// glued page counts until its secret has been opened, or the seal would give the secret away;
    /// from then on the seal lies on the secret, the last page there is.
    fn right_page_is_last(&self) -> bool {
        let lying = match &self.turn {
            // the leaf is the left page coming over: what lies on the right stays till it lands
            Some(t) if t.back => &t.from,
            Some(t) => &t.to,
            None => self.shown.as_ref().unwrap_or(&self.spread),
        };
        let opened = lying.secret.as_ref().is_some_and(|(act, _)| self.opened.contains(act));
        self.ahead_of(lying) == Some(0) && !opened
    }

    /// The leaf is all but down where it is going: nobody would see it finish at once.
    fn nearly_down(&self) -> bool {
        self.leaf.side().abs() > 0.9 && self.leaf.points.iter().all(|p| p.y < TOP + NEARLY_DOWN)
    }

    /// A hand takes the leaf out of the air, just where it is. Let go, a leaf the story has read on
    /// past still goes on over.
    fn catch(&mut self, point: usize) -> bool {
        let Some(turn) = self.turn.as_mut() else { return false };
        if !matches!(turn.hand, Hand::Gone | Hand::Unseen { .. }) {
            return false;
        }
        turn.hand = Hand::Reader;
        turn.mine = true;
        turn.loose = 0.0;
        self.leaf.grab = Some((point, self.leaf.points[point]));
        self.easing = EASE;
        note(format!("caught at {point}; {}", self.state()));
        true
    }

    /// A leaf caught in the air after the story had read on and let go back where it came from:
    /// the hand has the last word. It is now a turn the other way, wearing the same faces, and the
    /// story has to step back with it. Is the story to be told so?
    fn reverse(&mut self) -> bool {
        let Some(turn) = self.turn.as_mut() else { return false };
        std::mem::swap(&mut turn.from, &mut turn.to);
        turn.back = !turn.back;
        turn.committed = true;
        if turn.own {
            return false;
        }
        turn.story_to = turn.to.clone();
        true
    }

    /// A new run: a new reading of the book, its glued leaves whole again.
    pub fn forget_run(&mut self) {
        self.gone.clear();
        self.drawn = Default::default();
        self.opened.clear();
        self.loose.clear();
    }

    /// Lays the book open at once, with no lifting of the cover: a scene read in the midst of a
    /// run finds the book already open.
    pub fn lay_open(&mut self) {
        self.lifted = true;
        self.heard = true;
        self.open = 1.0;
    }

    /// The act whose glued leaf lies on the right, still shut and not worked loose.
    fn glued_act(&self) -> Option<u8> {
        let act = self.shown.as_ref()?.secret.as_ref()?.0;
        (!self.opened.contains(&act) && !self.loose.contains(&act)).then_some(act)
    }

    /// Does the left-hand page lie open on a secret, with its glued page just before it?
    pub fn on_secret(&self) -> bool {
        self.shown.as_ref().is_some_and(|s| s.peeled)
    }

    /// The reader's nail goes to a corner of the glued leaf.
    fn start_nail(&mut self, hit: Vec3) -> Option<usize> {
        let shown = self.shown.clone()?;
        let (act, secret) = shown.secret.clone()?;
        self.fresh_leaf();
        self.leaf.glue_down();
        self.leaf.tearing = false;
        self.wear(Face::LeafFront, &shown, Half::Right);
        self.wear(Face::LeafBack, &secret, Half::Left);
        self.wear(Face::Right, &secret, Half::Right);
        let point = leaf::at(leaf::NX - 1, if hit.z < 0.0 { 0 } else { leaf::NZ - 1 });
        self.leaf.grab = Some((point, self.leaf.points[point]));
        self.rub = Rub::default();
        let story_to = self.spread.clone();
        self.turn = Some(Turning { from: shown, to: *secret, committed: false, hand: Hand::Nail, loose: 0.0, mine: true, peel: Some(act), back: false, own: true, story_to });
        Some(point)
    }

    /// Is another reader pulling at the corner opposite this point?
    fn other_tug(&self, point: usize) -> Option<u8> {
        let here = self.leaf.home(point);
        self.ghosts.iter().enumerate().find_map(|(seat, g)| {
            let t = g.as_ref()?.tug.filter(|t| *t < self.leaf.points.len())?;
            pulled_apart(here, self.leaf.home(t)).then_some(seat as u8)
        })
    }

    /// The corner has come loose under the reader's hand. Alone they hold it and may pull at once;
    /// of two readers the lower seat peels, and the other lets go and watches.
    fn loosen(&mut self, act: u8, partner: Option<u8>) {
        if !self.loose.contains(&act) {
            self.loose.push(act);
        }
        self.helped = true;
        self.worked = 1.0;
        self.easing = EASE;
        let peels = partner.is_none_or(|other| self.seat < other);
        self.leaf.tearing = true;
        let Some(turn) = self.turn.as_mut() else { return };
        if peels {
            turn.hand = Hand::Reader;
        } else {
            turn.hand = Hand::Gone;
            self.leaf.grab = None;
        }
    }

    /// A hand reaches for the book while a whole leaf is still falling: it is down at once, where
    /// it was going, and the hand takes what it reached for. Not while the story has yet to catch
    /// up with the turn, nor a torn leaf, which falls where it will.
    fn land_now(&mut self) -> Option<Settled> {
        let turn = self.turn.as_ref()?;
        if !matches!(turn.hand, Hand::Gone | Hand::Unseen { .. }) || self.leaf.torn() {
            return None;
        }
        if turn.committed && !turn.own && turn.story_to != self.spread {
            return None;
        }
        let side = self.leaf.side();
        let on_left = match (turn.back, turn.committed) {
            (true, committed) => !committed,
            (false, true) => true,
            (false, false) => turn.peel.is_some() && side < 0.0,
        };
        self.leaf = Leaf::new(GUTTER, PAGE_W, PAGE_H, LEAF_FLOOR);
        if on_left {
            self.leaf.lay_left();
        }
        Some(self.settle())
    }

    /// Turns the left-hand page back over by the book's own hand, from a secret to its glued page.
    pub fn turn_back_unseen(&mut self) -> bool {
        if self.turn.is_some() || !self.take_up_back(Hand::Gone) {
            return false;
        }
        self.leaf.tearing = false;
        let point = self.leaf.nearest(Vec3::new(-(GUTTER + PAGE_W), LEAF_FLOOR.left, PAGE_H * 0.4));
        let z = self.leaf.points[point].z;
        if let Some(turn) = self.turn.as_mut() {
            turn.committed = true;
            turn.hand = Hand::Unseen { t: 0.0, point, z };
        }
        true
    }

    /// Keeps the paper in step with the story. Was a leaf set turning by the unseen hand?
    fn follow(&mut self) -> bool {
        if !self.wanted {
            self.shown = None;
            self.followed = None;
            self.turn = None;
            self.kept = false;
            self.worked = 0.0;
            self.helped = false;
            return false;
        }
        // the story's page, not what lies open: a secret peeled open is the book's own, and the
        // story knows nothing of it
        let spread = self.spread.clone();
        if self.followed.as_ref() == Some(&spread) {
            return false;
        }
        note(format!("follow: {}", self.state()));
        self.followed = Some(spread.clone());
        // the book is already there: a page torn out uncovered it
        if self.turn.is_none() && self.shown.as_ref() == Some(&spread) {
            return false;
        }
        self.worked = 0.0;
        self.helped = false;
        // the reader has carried the leaf over, and the story has read on to what it uncovered
        if let Some(turn) = self.turn.as_mut().filter(|t| t.story_to == spread) {
            turn.committed = true;
            self.shown = Some(turn.to.clone());
            return false;
        }
        let mut flip = false;
        match self.shown.take() {
            // the book has only just opened, or a turn is under way: no time for the leaf, the pages change
            // going back a page the unseen hand does not turn: the pages change
            Some(from) if self.open >= 1.0 && self.turn.is_none() && spread.numbers.0 >= from.numbers.0 => {
                self.fresh_leaf();
                // the story's own hand is sure: it never tears what it turns
                self.leaf.tearing = false;
                let point = self.leaf.nearest(Vec3::new(GUTTER + PAGE_W, LEAF_FLOOR.right, PAGE_H * 0.4));
                let z = self.leaf.points[point].z;
                self.wear(Face::LeafFront, &from, Half::Right);
                self.wear(Face::LeafBack, &spread, Half::Left);
                self.wear(Face::Right, &spread, Half::Right);
                self.turn = Some(Turning { from, to: spread.clone(), committed: true, hand: Hand::Unseen { t: 0.0, point, z }, loose: 0.0, mine: false, peel: None, back: false, own: false, story_to: spread.clone() });
                flip = true;
            }
            _ => {
                self.turn = None;
                self.fresh_leaf();
                self.wear(Face::Left, &spread, Half::Left);
                self.wear(Face::Right, &spread, Half::Right);
                // the book opened at another place: its old scraps lie somewhere far back in it
                self.covered.push(Half::Right);
            }
        }
        self.shown = Some(spread);
        flip
    }

    /// The leaf has come to rest: on the far side it becomes the page lying there, on the near
    /// side it was never turned; a torn leaf stays as it lies, holes and loose pieces and all, over
    /// the page beneath.
    fn settle(&mut self) -> Settled {
        let mut out = Settled::default();
        note(format!("settle: {}", self.state()));
        let Some(turn) = self.turn.take() else { return out };
        let Turning { from, to, committed, mine, peel, back, own, .. } = turn;
        let mine = mine || self.helped;
        let side = self.leaf.side();
        let over = if back { side > 0.0 } else { side < 0.0 };
        // a leaf peeled over onto the left has opened its secret, whoever's hand it was in
        if let Some(act) = peel.filter(|_| over) {
            self.opened.push(act);
            out.found = Some((act, !self.leaf.torn(), mine));
        }
        if own && over {
            self.shown = Some(to.clone());
        }
        let done = (committed || own) && over;
        if self.leaf.torn() {
            self.kept = true;
            // only paper newly cut loose costs: a leaf taken up torn again has paid for its old holes
            let lost = self.leaf.lost();
            if lost - self.charged > 0.005 {
                out.torn = Some((lost - self.charged, mine));
                self.charged = lost;
            }
            // torn out, or left hanging: it is no page any more, only paper lying about, and the
            // book shows one whole spread beneath it, never the halves of two
            // torn out: both its sides are gone from the book for good
            if lost > WHOLE_PAGE_GONE {
                for face in [Face::LeafFront, Face::LeafBack] {
                    if let Some((spread, half)) = &self.faces[face as usize] {
                        let number = match half {
                            Half::Left => spread.numbers.0,
                            Half::Right => spread.numbers.1,
                        };
                        if number > 0 && !self.gone.contains(&number) {
                            self.gone.push(number);
                        }
                    }
                }
                self.drawn = Default::default();
            }
            if lost > WHOLE_PAGE_GONE || side.abs() < LIES_FLAT || (committed && !over) {
                out.read_on = !committed && !own && !back && lost > WHOLE_PAGE_GONE && self.next.is_some();
                let at = if committed || done || out.read_on { &to } else { &from };
                let at = at.clone();
                self.kept = false;
                self.fresh_leaf();
                self.wear(Face::Left, &at, Half::Left);
                self.wear(Face::Right, &at, Half::Right);
                self.covered.extend([Half::Left, Half::Right]);
                self.shown = Some(at);
            }
        } else if back {
            // turned back over, the leaf is the right-hand page before; fallen back, the left again
            if done {
                self.wear(Face::Right, &to, Half::Right);
            } else {
                self.wear(Face::Left, &from, Half::Left);
            }
        } else if done {
            self.wear(Face::Left, &to, Half::Left);
        } else {
            self.wear(Face::Right, &from, Half::Right);
        }
        out
    }

    fn wear(&mut self, face: Face, spread: &Spread, half: Half) {
        let new = Some((spread.clone(), half));
        // a leaf come down on the left lies over the scraps that lay there
        if face == Face::Left && self.faces[face as usize] != new {
            self.covered.push(Half::Left);
        }
        self.faces[face as usize] = new;
    }

    /// Readies the leaf to be turned. A torn leaf left lying on the right-hand page is taken up
    /// as it is, torn; one lying elsewhere stays where it lies, as a scrap, and a fresh leaf is
    /// taken.
    fn fresh_leaf(&mut self) {
        self.fresh_leaf_for(true);
    }

    /// As `fresh_leaf`, for a leaf turned that way: a torn leaf lying on the side it is taken
    /// from is taken up as it is. Returns whether it was.
    fn fresh_leaf_for(&mut self, forward: bool) -> bool {
        let side = self.leaf.side();
        let reuse = self.kept && if forward { side > 0.5 } else { side < -0.5 };
        if !reuse {
            let old = std::mem::replace(&mut self.leaf, Leaf::new(GUTTER, PAGE_W, PAGE_H, LEAF_FLOOR));
            self.charged = 0.0;
            // torn paper, at rest or still falling, is never just gone
            if old.torn() {
                let faces = (self.faces[Face::LeafFront as usize].clone(), self.faces[Face::LeafBack as usize].clone());
                if let (Some(front), Some(back)) = faces {
                    self.scraps_out.push((old, front, back));
                }
            }
        }
        self.kept = false;
        reuse
    }

    /// Another reader's hand, as their machine tells it: `color` is their hero's.
    pub fn remote_hand(&mut self, seat: u8, color: Color, at: [f32; 3], hold: Option<u16>) {
        if let Some(slot) = self.ghosts.get_mut(seat as usize) {
            let at = Vec3::from_array(at);
            let speed = match slot {
                Some(g) if g.quiet > 1e-3 && g.quiet < 0.5 => (at - g.at) / g.quiet,
                _ => Vec3::ZERO,
            };
            // a hand at a glued corner is not turning the leaf; the marks say which it is
            let point = hold.map(|h| usize::from(h & !(TUG_BIT | PEEL_BIT)));
            let tugging = hold.is_some_and(|h| h & TUG_BIT != 0);
            let peeling = hold.is_some_and(|h| h & PEEL_BIT != 0);
            *slot = Some(Ghost { at, speed, hold: point.filter(|_| !tugging), peeling, tug: point.filter(|_| tugging), color, quiet: 0.0 });
        }
    }

    /// Torn paper another reader carries (`point` held) or lets go (`None`).
    pub fn remote_carry(&mut self, seat: u8, color: Color, piece: u32, point: Option<u16>, at: [f32; 3]) {
        self.remote_hand(seat, color, at, None);
        match point {
            Some(p) => self.carried = Some((seat, piece, p, Vec3::from_array(at), 0.0)),
            None => {
                if self.carried.is_some_and(|c| c.0 == seat) {
                    self.carried = None;
                }
                self.dropped.push(piece);
            }
        }
    }

    /// The leaf another reader holds tore there.
    pub fn remote_tear(&mut self, edge: u16) {
        if matches!(self.turn.as_ref().map(|t| &t.hand), Some(Hand::Remote(_)) | Some(Hand::Gone) | Some(Hand::Carried(_))) {
            self.leaf.tear_edge(edge as usize);
        }
    }

    /// Starts turning the leaf over to the next spread: the right-hand page shows that spread at
    /// once, and the leaf wears this one's right side and the next one's left.
    fn take_up(&mut self, hand: Hand) -> bool {
        let Some(shown) = self.shown.clone() else { return false };
        // a glued pair turns over as one, unless its corner has been worked loose: then the hand
        // lifts the top leaf off the secret beneath it. Peeled open once, the secret is a page
        // like any other, though the story has no part in it
        let (to, peel, own) = match shown.secret.clone() {
            Some((act, secret)) if self.opened.contains(&act) => (*secret, None, true),
            Some((act, secret)) if self.loose.contains(&act) => (*secret, Some(act), true),
            _ => match self.next.clone() {
                Some(next) => (next, None, false),
                None => return false,
            },
        };
        let story_to = if own { self.spread.clone() } else { to.clone() };
        self.fresh_leaf();
        if peel.is_some() {
            self.leaf.glue_down();
        }
        note(format!("take up: to {}|{} peel {peel:?} own {own}; {}", to.numbers.0, to.numbers.1, self.state()));
        // a leaf held on another machine tears there; here it only follows
        self.leaf.tearing = !matches!(hand, Hand::Remote(_));
        self.wear(Face::LeafFront, &shown, Half::Right);
        self.wear(Face::LeafBack, &to, Half::Left);
        self.wear(Face::Right, &to, Half::Right);
        let mine = matches!(hand, Hand::Reader);
        self.turn = Some(Turning { from: shown, to, committed: false, hand, loose: 0.0, mine, peel, back: false, own, story_to });
        true
    }

    /// Starts turning the left-hand page back over to the spread before: the left page shows
    /// that spread at once, and the leaf, lying on the left, wears this one's left side up and
    /// the one before's right side under it.
    fn take_up_back(&mut self, hand: Hand) -> bool {
        let Some(shown) = self.shown.clone() else { return false };
        // from a secret back to its glued page, the book's own turn; a secret opened before lies
        // between this page and the one before, and is what the leaf uncovers
        let (to, story_to, own) = if shown.peeled {
            (self.spread.clone(), self.spread.clone(), true)
        } else {
            let Some(prev) = self.prev.clone() else { return false };
            match prev.secret.clone().filter(|(act, _)| self.opened.contains(act)) {
                Some((_, secret)) => (*secret, prev, false),
                None => (prev.clone(), prev, false),
            }
        };
        note(format!("take up back: to {}|{} own {own}; {}", to.numbers.0, to.numbers.1, self.state()));
        // a torn leaf turned over onto the left is turned back as it is, holes and all
        if !self.fresh_leaf_for(false) {
            self.leaf.lay_left();
        }
        self.leaf.tearing = true;
        self.wear(Face::LeafBack, &shown, Half::Left);
        self.wear(Face::LeafFront, &to, Half::Right);
        self.wear(Face::Left, &to, Half::Left);
        let mine = matches!(hand, Hand::Reader);
        self.turn = Some(Turning { from: shown, to, committed: false, hand, loose: 0.0, mine, peel: None, back: true, own, story_to });
        true
    }
}

#[derive(Component)]
struct BookCamera;
#[derive(Component)]
struct PageCamera;
#[derive(Component)]
struct Cover;
/// The wax seal on the cord that binds the leaves not yet open.
#[derive(Component)]
struct Seal;
/// The left half of the back cover and of the page block, this wide once the book lies open.
#[derive(Component)]
struct LeftHalf(f32);
#[derive(Component)]
struct LeafMesh;
/// The wisp showing another reader's hand, by their seat.
#[derive(Component)]
struct GhostHand(u8);
#[derive(Component)]
struct Candle {
    base: f32,
}
#[derive(Component)]
struct Flame;
/// The UI tree drawn onto one face of paper.
#[derive(Component)]
struct PageCanvas(Face);

/// The stage's `UiScale` fits the UI to the window, and it reaches UI drawn into textures too; the
/// pages undo it so one page pixel stays one texture pixel at any window size.
fn fit_pages(scale: Res<UiScale>, mut targets: Query<(&mut RenderTarget, &mut Projection), With<PageCamera>>) {
    let want = 1.0 / scale.0;
    for (mut target, mut projection) in &mut targets {
        let stale = matches!(target.as_ref(), RenderTarget::Image(image) if (image.scale_factor - want).abs() > 1e-4);
        if stale {
            if let RenderTarget::Image(image) = target.as_mut() {
                image.scale_factor = want;
            }
            // Bevy reads a camera's target again only when its projection changes, not the target
            // itself: a window resized in the middle of a tale (made full screen) left the pages
            // drawn at the old scale, stretched
            projection.set_changed();
        }
    }
}

fn target(images: &mut Assets<Image>, size: UVec2) -> Handle<Image> {
    images.add(Image::new_target_texture(size.x, size.y, TextureFormat::Bgra8UnormSrgb, None))
}

fn tiled(assets: &AssetServer, art: &Art, stem: &str) -> Option<Handle<Image>> {
    let path = art.find(stem, "png", "gemini")?;
    Some(assets.load_builder().with_settings(|s: &mut ImageLoaderSettings| {
        s.sampler = ImageSampler::linear();
        s.sampler.get_or_init_descriptor().set_address_mode(ImageAddressMode::Repeat);
    }).load(path))
}

/// The leaf as a mesh: its front faces up while it lies on the right, its back once on the left.
/// The back's pictures run the other way across, so the spine stays at the spine.
fn leaf_mesh(leaf: &Leaf, back: bool) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    shape_leaf(&mut mesh, leaf, back);
    mesh
}

/// How far the fringe of a torn edge reaches into the paper (a band of bared white fibres over
/// the print) and out past the edge (loose fibres), in scene units; a page is one unit wide.
const FRINGE_IN: f32 = 0.012;
const FRINGE_OUT: f32 = 0.01;
/// Length of torn edge one repeat of the fringe's picture covers.
const FRINGE_SPAN: f32 = 0.12;

/// A cheap, repeatable hash of a number to 0..1.
fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65535.0
}

/// The picture laid along a torn edge: across it (rows) from inside the paper to beyond the edge,
/// along it (columns, repeating). Inside, the paper's top layer is torn away and its pale fibres
/// show, fading into the print; the edge itself wanders; past it, loose fibres hang out.
fn fringe_picture() -> Image {
    use bevy::render::render_resource::{Extent3d, TextureDimension};
    const W: u32 = 256;
    const H: u32 = 64;
    // value noise along the edge, wrapping so the picture repeats without a seam
    let wander = |x: u32| -> f32 {
        let mut sum = 0.0;
        for (period, weight, salt) in [(64u32, 0.6, 1u32), (16, 0.3, 2), (4, 0.1, 3)] {
            let cells = W / period;
            let k = x / period;
            let t = (x % period) as f32 / period as f32;
            let s = t * t * (3.0 - 2.0 * t);
            let a = hash(k % cells + salt * 1000);
            let b = hash((k + 1) % cells + salt * 1000);
            sum += weight * (a + (b - a) * s);
        }
        sum
    };
    let edge_at = FRINGE_IN / (FRINGE_IN + FRINGE_OUT);
    let mut data = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        let v = y as f32 / (H - 1) as f32;
        for x in 0..W {
            let edge = edge_at + (wander(x) - 0.5) * 0.35;
            // a loose fibre here and there, hanging out past the edge a way
            let strand = hash(x * 7 + 11) < 0.22 && v < edge + 0.06 + hash(x * 13 + 5) * 0.4;
            let alpha = if v <= edge {
                // the bared fibres fade in from the print towards the edge
                let t = (v / edge).clamp(0.0, 1.0);
                t * t * 0.95
            } else if strand {
                0.75
            } else {
                0.0
            };
            let grain = 0.9 + 0.1 * hash(x * 131 + y * 977);
            let cream = [0.95, 0.92, 0.85].map(|c: f32| (c * grain * 255.0) as u8);
            data.extend_from_slice(&[cream[0], cream[1], cream[2], (alpha * 255.0) as u8]);
        }
    }
    let mut image = Image::new(Extent3d { width: W, height: H, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD);
    image.sampler = ImageSampler::linear();
    image.sampler.get_or_init_descriptor().set_address_mode(ImageAddressMode::Repeat);
    image
}

/// The fringes of every torn edge, on both faces of the paper.
fn shape_fringe(mesh: &mut Mesh, leaf: &Leaf) {
    let (mut positions, mut normals, mut uvs, mut indices) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for rim in leaf.rims() {
        let u0 = (rim.seed as f32 * 0.618).fract();
        let u1 = u0 + rim.a.distance(rim.b) / FRINGE_SPAN;
        for lift in [0.0008, -0.0008] {
            let up = rim.normal * lift;
            let base = positions.len() as u32;
            for (p, into, u) in [(rim.a, rim.into_a, u0), (rim.b, rim.into_b, u1)] {
                positions.push((p + into * FRINGE_IN + up).to_array());
                uvs.push([u, 0.0]);
                positions.push((p - into * FRINGE_OUT + up).to_array());
                uvs.push([u, 1.0]);
                normals.push(rim.normal.to_array());
                normals.push(rim.normal.to_array());
            }
            indices.extend([base, base + 1, base + 2, base + 2, base + 1, base + 3]);
        }
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
}

/// Lays the leaf's particles into its mesh. A tear adds particles, so everything is rebuilt.
fn shape_leaf(mesh: &mut Mesh, leaf: &Leaf, back: bool) {
    let positions: Vec<[f32; 3]> = leaf.points.iter().map(|p| p.to_array()).collect();
    let normals: Vec<[f32; 3]> = leaf.normals().iter().map(|n| if back { (-*n).to_array() } else { n.to_array() }).collect();
    let uvs: Vec<[f32; 2]> = (0..leaf.points.len())
        .map(|p| {
            let [u, v] = leaf.uv(p);
            [if back { 1.0 - u } else { u }, v]
        })
        .collect();
    let mut indices = Vec::with_capacity(leaf.cells().len() * 6);
    for &[hs, he, fs, fe] in leaf.cells() {
        let quad = [hs, fs, he, he, fs, fe].map(|p| p as u32);
        if back {
            indices.extend(quad.iter().rev());
        } else {
            indices.extend(quad);
        }
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
    art: Res<Art>,
) {
    let view = target(&mut images, VIEW);
    let textures = FACES.map(|_| target(&mut images, PAGE_PX));

    // the scene's camera, asleep until a tale is read
    commands.spawn((
        BookCamera,
        Camera3d::default(),
        Camera { order: -10, is_active: false, clear_color: ClearColorConfig::Custom(Color::srgb(0.02, 0.015, 0.01)), ..default() },
        RenderTarget::Image(view.clone().into()),
        Transform::from_translation(EYE).looking_at(LOOK, Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight { color: Color::srgb(1.0, 0.85, 0.7), brightness: 60.0, ..default() });

    let mat = |materials: &mut Assets<StandardMaterial>, tex: Option<Handle<Image>>, tint: Color, rough: f32, repeat: f32| {
        materials.add(StandardMaterial {
            base_color: tint,
            base_color_texture: tex,
            perceptual_roughness: rough,
            uv_transform: bevy::math::Affine2::from_scale(Vec2::splat(repeat)),
            ..default()
        })
    };

    // the table
    let oak = mat(&mut materials, tiled(&assets, &art, "book/table"), Color::WHITE, 0.85, 3.0);
    commands.spawn((Mesh3d(meshes.add(Plane3d::default().mesh().size(9.0, 6.0))), MeshMaterial3d(oak), Transform::default()));

    // the block of pages and the back cover under it
    let edges = mat(&mut materials, tiled(&assets, &art, "book/edges"), Color::WHITE, 0.9, 1.0);
    let leather = mat(&mut materials, tiled(&assets, &art, "book/leather"), Color::WHITE, 0.7, 2.0);
    // shut, it all lies on the right; the left half spreads out as the cover comes over (`open_cover`)
    let (back_w, block_w) = (PAGE_W + 0.06, PAGE_W + 0.01);
    let back_y = 0.0125;
    let block_y = 0.025 + (TOP - 0.025) / 2.0;
    let back = meshes.add(Cuboid::new(back_w, 0.025, PAGE_H + 0.08));
    let block = meshes.add(Cuboid::new(block_w, TOP - 0.025, PAGE_H));
    commands.spawn((Mesh3d(back.clone()), MeshMaterial3d(leather.clone()), Transform::from_xyz(back_w / 2.0, back_y, 0.0)));
    commands.spawn((Mesh3d(block.clone()), MeshMaterial3d(edges.clone()), Transform::from_xyz(block_w / 2.0, block_y, 0.0)));
    commands.spawn((LeftHalf(back_w), Mesh3d(back), MeshMaterial3d(leather.clone()), Transform::from_xyz(0.0, back_y, 0.0).with_scale(Vec3::new(0.0, 1.0, 1.0)), Visibility::Hidden));
    commands.spawn((LeftHalf(block_w), Mesh3d(block), MeshMaterial3d(edges), Transform::from_xyz(0.0, block_y, 0.0).with_scale(Vec3::new(0.0, 1.0, 1.0)), Visibility::Hidden));

    // the faces of paper, each wearing its own rendered picture; the right-hand page lies on the
    // block, the left-hand one is the first leaf and turns over with the cover
    let page_mat = |materials: &mut Assets<StandardMaterial>, tex: &Handle<Image>| {
        materials.add(StandardMaterial { base_color_texture: Some(tex.clone()), perceptual_roughness: 0.95, ..default() })
    };
    let page = meshes.add(Plane3d::default().mesh().size(PAGE_W, PAGE_H));
    let left_mat = page_mat(&mut materials, &textures[Face::Left as usize]);
    commands.spawn((
        Mesh3d(page.clone()),
        MeshMaterial3d(page_mat(&mut materials, &textures[Face::Right as usize])),
        Transform::from_xyz(PAGE_W / 2.0 + GUTTER, TOP + 0.002, 0.0),
    ));

    // the leaf being turned, hidden until a page turns
    let leaf = Leaf::new(GUTTER, PAGE_W, PAGE_H, LEAF_FLOOR);
    let front = meshes.add(leaf_mesh(&leaf, false));
    let back = meshes.add(leaf_mesh(&leaf, true));
    for (mesh, face) in [(front.clone(), Face::LeafFront), (back.clone(), Face::LeafBack)] {
        let mut leaf = commands.spawn((LeafMesh, Mesh3d(mesh), MeshMaterial3d(page_mat(&mut materials, &textures[face as usize])), Transform::default(), Visibility::Hidden));
        // the two sides lie in one place: one shadow is enough, two would darken each other; and
        // a crumpled leaf lying a hair over the page would speckle itself with its own shadow
        leaf.insert(bevy::light::NotShadowReceiver);
        if face == Face::LeafBack {
            leaf.insert(bevy::light::NotShadowCaster);
        }
    }

    // the fringes of the leaf's tears: pale fibres along every torn edge
    let mut fringe = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    shape_fringe(&mut fringe, &leaf);
    let fringe = meshes.add(fringe);
    let fibres = materials.add(StandardMaterial {
        base_color_texture: Some(images.add(fringe_picture())),
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        cull_mode: None,
        perceptual_roughness: 0.95,
        ..default()
    });
    commands.spawn((LeafMesh, Mesh3d(fringe.clone()), MeshMaterial3d(fibres), Transform::default(), Visibility::Hidden, bevy::light::NotShadowCaster, bevy::light::NotShadowReceiver));

    // the front cover hinges on the spine; closed it lies over the right page
    let cover_front = materials.add(StandardMaterial {
        base_color_texture: art.image(&assets, "book/cover"),
        perceptual_roughness: 0.6,
        ..default()
    });
    let endpaper = mat(&mut materials, tiled(&assets, &art, "book/endpaper"), Color::WHITE, 0.8, 1.0);
    let board = Vec3::new(PAGE_W + 0.06, 0.022, PAGE_H + 0.08);
    commands
        .spawn((Cover, Transform::from_xyz(0.0, TOP + 0.004, 0.0), Visibility::default()))
        .with_children(|c| {
            c.spawn((Mesh3d(meshes.add(Cuboid::new(board.x, board.y, board.z))), MeshMaterial3d(leather), Transform::from_xyz(board.x / 2.0, board.y / 2.0, 0.0)));
            // the painted face on top, the endpaper beneath
            let face = meshes.add(Plane3d::default().mesh().size(board.x, board.z));
            c.spawn((Mesh3d(face.clone()), MeshMaterial3d(cover_front), Transform::from_xyz(board.x / 2.0, board.y + 0.001, 0.0)));
            c.spawn((
                Mesh3d(face),
                MeshMaterial3d(endpaper),
                Transform::from_xyz(board.x / 2.0, -0.001, 0.0).with_rotation(Quat::from_rotation_x(PI)),
            ));
            // the first leaf under the board: upside down while shut, upright once the cover lies
            // open (a half turn about x, then one about y, undoes the cover's half turn about z)
            c.spawn((
                Mesh3d(page),
                MeshMaterial3d(left_mat),
                Transform::from_xyz(PAGE_W / 2.0 + GUTTER, -0.002, 0.0).with_rotation(Quat::from_rotation_x(PI) * Quat::from_rotation_y(PI)),
            ));
        });

    // the seal on the cord binding the leaves not yet open: the wax lies on the right-hand page by
    // its outer corner, the cord runs off over the edge; a leaf turning passes over it
    let seal_mat = materials.add(StandardMaterial { base_color_texture: art.image(&assets, "book/seal"), base_color: Color::srgb(0.62, 0.55, 0.5), alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.85, ..default() });
    commands.spawn((
        Seal,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(0.34, 0.166))),
        MeshMaterial3d(seal_mat),
        Transform::from_xyz(GUTTER + PAGE_W - 0.17, TOP + 0.003, PAGE_H * 0.37).with_rotation(Quat::from_rotation_y(PI)),
        bevy::light::NotShadowCaster,
        bevy::light::NotShadowReceiver,
        Visibility::Hidden,
    ));

    // the candle at the top right, and its light
    let wax = materials.add(StandardMaterial { base_color: Color::srgb(0.86, 0.78, 0.6), perceptual_roughness: 0.5, ..default() });
    let candle_at = Vec3::new(1.3, 0.0, -0.95);
    // its own light sits right over it: a shadow from the wax would black out the table
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.09, 0.55))),
        MeshMaterial3d(wax),
        Transform::from_translation(candle_at + Vec3::Y * 0.275),
        bevy::light::NotShadowCaster,
    ));
    let flame_mat = materials.add(StandardMaterial {
        base_color_texture: art.image(&assets, "book/flame"),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    commands.spawn((Flame, Mesh3d(meshes.add(Rectangle::new(0.16, 0.16))), MeshMaterial3d(flame_mat), Transform::from_translation(candle_at + Vec3::Y * 0.62)));
    commands.spawn((
        Candle { base: 90_000.0 },
        PointLight { color: Color::srgb(1.0, 0.72, 0.42), intensity: 90_000.0, range: 12.0, shadow_maps_enabled: true, ..default() },
        Transform::from_translation(candle_at + Vec3::Y * 0.66),
    ));

    // the other readers' hands: a pale wisp each, in their hero's colour, hidden till heard of
    let wisp = meshes.add(Rectangle::new(0.14, 0.14));
    for seat in 0..4 {
        let glow = materials.add(StandardMaterial {
            base_color_texture: art.image(&assets, "book/flame"),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
        commands.spawn((GhostHand(seat), Mesh3d(wisp.clone()), MeshMaterial3d(glow), Transform::default(), Visibility::Hidden, bevy::light::NotShadowCaster));
    }

    // the painters: one camera and one UI tree per face of paper
    for (face, tex) in FACES.into_iter().zip(textures) {
        let cam = commands
            .spawn((PageCamera, Camera2d, Camera { order: -20, is_active: false, clear_color: ClearColorConfig::Custom(Color::srgb(0.93, 0.88, 0.76)), ..default() }, RenderTarget::Image(tex.into())))
            .id();
        commands.spawn((PageCanvas(face), Node { width: px(PAGE_PX.x as f32), height: px(PAGE_PX.y as f32), ..default() }, UiTargetCamera(cam)));
    }

    commands.insert_resource(Book::new(view, leaf, (front, back), fringe));
}

fn play(commands: &mut Commands, art: &Art, assets: &AssetServer, mixer: &Mixer, name: &str) {
    if let Some(path) = art.find(&format!("sfx/{name}"), "wav", "flow") {
        commands.spawn((AudioPlayer::new(assets.load::<AudioSource>(path)), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(mixer.sfx))));
    }
}

/// The seal binds the leaves not yet free, so it lies on the last page that can be reached, and
/// only there: uncovered as the leaf above is lifted off it, covered as one comes back over it.
fn keep_seal(book: Res<Book>, mut seal: Query<&mut Visibility, With<Seal>>) {
    let on = book.wanted && !book.ending && book.right_page_is_last();
    for mut vis in &mut seal {
        let want = if on { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
}

/// Spreads out the book's left half: nothing until the cover has passed upright, then as it falls.
fn spread_left(left: &mut Query<(&LeftHalf, &mut Transform, &mut Visibility), Without<Cover>>, open: f32) {
    let t = open.clamp(0.0, 1.0);
    let s = ((t * t * (3.0 - 2.0 * t) - 0.5) * 2.0).clamp(0.0, 1.0);
    for (half, mut tf, mut vis) in left {
        tf.scale.x = s.max(1e-3);
        tf.translation.x = -half.0 * s / 2.0;
        let want = if s > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
}

/// Wakes the scene while a tale is read, and swings the cover open on its spine.
fn open_cover(
    mut commands: Commands,
    time: Res<Time>,
    mut book: ResMut<Book>,
    mut cams: Query<&mut Camera, Or<(With<BookCamera>, With<PageCamera>)>>,
    mut cover: Query<&mut Transform, With<Cover>>,
    mut left: Query<(&LeftHalf, &mut Transform, &mut Visibility), Without<Cover>>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    mixer: Res<Mixer>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    for mut cam in &mut cams {
        if cam.is_active != book.wanted {
            cam.is_active = book.wanted;
        }
    }
    if !book.wanted {
        book.open = 0.0;
        book.heard = false;
        book.lifted = false;
        // put away shut: left swung open, it would be seen open next time, and snap shut at a touch
        for mut tf in &mut cover {
            tf.rotation = cover_turn(0.0);
        }
        spread_left(&mut left, 0.0);
        return;
    }
    // a book found already open lies so from the first frame
    spread_left(&mut left, book.open);
    // shut on the table until the reader opens it (an unattended dev run opens it at once)
    if !book.lifted {
        let unattended = cfg!(feature = "dev") && std::env::var("TROPA_SHOT").is_ok();
        if unattended || mouse.just_pressed(MouseButton::Left) || keys.any_just_pressed([KeyCode::Space, KeyCode::Enter, KeyCode::ArrowRight]) {
            book.lifted = true;
        } else {
            return;
        }
    }
    if !book.heard {
        book.heard = true;
        play(&mut commands, &art, &assets, &mixer, "book_clasp");
        play(&mut commands, &art, &assets, &mixer, "book_open");
    }
    // a heavy board: slow to lift, quicker as it falls over, and it lies flat open
    book.open = (book.open + time.delta_secs() / 1.4).min(1.0);
    for mut tf in &mut cover {
        tf.rotation = cover_turn(book.open);
    }
}

/// Keeps the paper in step with the story: a new spread turns the leaf over to it.
fn follow_story(mut commands: Commands, mut book: ResMut<Book>, art: Res<Art>, assets: Res<AssetServer>, mixer: Res<Mixer>) {
    if book.follow() {
        play(&mut commands, &art, &assets, &mixer, "book_flip");
    }
}

/// The point of the leaf the hand holds, or the one nearest it.
fn hand_point(leaf: &Leaf, hand: Vec3) -> usize {
    leaf.grab.map(|(p, _)| p).unwrap_or_else(|| leaf.nearest(hand))
}

/// Where the cursor's ray meets the level `y` in the scene.
fn cursor_on(camera: &Camera, at: &GlobalTransform, cursor: Vec2, y: f32) -> Option<Vec3> {
    let ray = camera.viewport_to_world(at, cursor).ok()?;
    let t = (y - ray.origin.y) / ray.direction.y;
    (t > 0.0).then(|| ray.origin + ray.direction * t)
}

/// The reader takes hold of the right-hand page, carries it, and lets go. The mouse is read as
/// picking sees it, so the dev robot's mouse turns pages too.
fn hold_leaf(
    mut commands: Commands,
    mut book: ResMut<Book>,
    pointers: Query<(&PointerId, &PointerLocation, &PointerPressState)>,
    // the mouse button a frame ago, and whether a hand still down waits for a leaf to land
    mut state: Local<(bool, bool)>,
    mut since_told: Local<f32>,
    time: Res<Time>,
    window: Single<&Window>,
    scale: Res<UiScale>,
    camera: Query<(&Camera, &GlobalTransform), With<BookCamera>>,
    mut turned: MessageWriter<Turned>,
    mut turned_back: MessageWriter<TurnedBack>,
    mut found: MessageWriter<Found>,
    mut scraps: Query<(Entity, &mut ScrapLeaf)>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    mixer: Res<Mixer>,
) {
    let Ok((camera, at)) = camera.single() else { return };
    let mouse = pointers.iter().find(|(id, ..)| id.is_mouse());
    let cursor = mouse.and_then(|(_, loc, _)| loc.location()).map(|l| crate::stage_point(&window, scale.0, l.position));
    let down = mouse.is_some_and(|(.., press)| press.is_primary_pressed());
    let pressed = down && !state.0;
    state.0 = down;
    let book = &mut *book;
    book.over_glue = false;
    // over a page read together the others see where the hand is, a few times a second
    *since_told += time.delta_secs();
    let mut tell_news = |book: &mut Book, news: HandNews, now: bool| {
        if book.shared && (now || *since_told >= 1.0 / HAND_RATE) {
            *since_told = 0.0;
            book.news.push(news);
        }
    };
    let hand_news = |at: Vec3, hold: Option<u16>| HandNews::Hand { at: at.to_array(), hold };
    // torn paper carried: `piece` 0 the leaf in play, else a scrap's age
    let carry_news = |piece: u32, point: Option<usize>, at: Vec3| HandNews::Carry { piece, point: point.map(|p| p as u16), at: at.to_array() };
    // a scrap of an older leaf in the hand: it goes where the hand goes, and falls when let go
    if let Some((_, mut scrap)) = scraps.iter_mut().find(|(_, s)| s.held && !s.theirs) {
        match cursor.and_then(|c| cursor_on(camera, at, c, LEAF_FLOOR.right + LIFT)) {
            Some(hand) if down => {
                if let Some((p, _)) = scrap.leaf.grab {
                    scrap.leaf.grab = Some((p, hand));
                    tell_news(book, carry_news(scrap.age, Some(p), hand), false);
                }
            }
            _ => {
                let hand = scrap.leaf.grab.map(|(_, h)| h).unwrap_or_default();
                tell_news(book, carry_news(scrap.age, None, hand), true);
                scrap.held = false;
                scrap.leaf.grab = None;
                scrap.active = SETTLE;
            }
        }
        return;
    }
    // paper torn loose is picked up again where it lies, on the book or on the table: first a
    // piece of the leaf in play, then the newest scrap under the hand
    if pressed && book.open >= 1.0 {
        if let Some(c) = cursor.filter(|c| c.y <= crate::STAGE_H - BUTTON_BAND) {
            let free = match book.turn.as_ref() {
                None => book.kept,
                Some(t) => matches!(t.hand, Hand::Gone),
            };
            let sight = |y: f32| cursor_on(camera, at, c, y);
            if let Some(p) = free.then(|| book.leaf.pick(sight, LOOSE_REACH, true, TOP)).flatten() {
                match book.turn.as_mut() {
                    Some(turn) => {
                        turn.hand = Hand::Loose;
                        turn.loose = 0.0;
                    }
                    None => {
                        let Some(shown) = book.shown.clone() else { return };
                        book.turn = Some(Turning { from: shown.clone(), to: shown.clone(), story_to: shown, committed: false, hand: Hand::Loose, loose: 0.0, mine: true, peel: None, back: false, own: false });
                    }
                }
                book.leaf.grab = Some((p, book.leaf.points[p]));
                play(&mut commands, &art, &assets, &mixer, "book_rustle");
                return;
            }
            let newest = scraps.iter().filter(|(_, s)| !s.held).filter_map(|(e, s)| s.leaf.pick(sight, LOOSE_REACH, false, TOP).map(|p| (s.age, e, p))).max_by_key(|(age, ..)| *age);
            if let Some((_, e, p)) = newest {
                if let Ok((_, mut scrap)) = scraps.get_mut(e) {
                    let from = scrap.leaf.points[p];
                    scrap.leaf.grab = Some((p, from));
                    scrap.held = true;
                    play(&mut commands, &art, &assets, &mixer, "book_rustle");
                    return;
                }
            }
        }
    }
    // a leaf in the air is caught where it is, if the hand reaches it. If not it flies on, and the
    // hand still down when it lands takes what it reached for
    if pressed && book.open >= 1.0 {
        if let Some(c) = cursor.filter(|c| c.y <= crate::STAGE_H - BUTTON_BAND) {
            let flying = book.turn.as_ref().is_some_and(|t| matches!(t.hand, Hand::Gone | Hand::Unseen { .. }));
            if flying && book.drag_ok {
                let caught = book.leaf.pick(|y| cursor_on(camera, at, c, y), CATCH_REACH, false, TOP).filter(|_| !book.nearly_down());
                if let Some(point) = caught {
                    if book.catch(point) {
                        let back = book.turn.as_ref().is_some_and(|t| t.back);
                        let hand = book.leaf.points[point];
                        tell_news(book, hand_news(hand, (!back).then_some(point as u16)), true);
                        play(&mut commands, &art, &assets, &mixer, "book_rustle");
                        return;
                    }
                }
                if book.nearly_down() {
                    if let Some(settled) = book.land_now() {
                        if let Some((act, whole, mine)) = settled.found {
                            found.write(Found { act, whole, mine });
                        }
                    }
                }
                state.1 = book.turn.is_some();
            }
        }
    }
    if state.1 && !down {
        state.1 = false;
    }
    let pressed = if state.1 && book.turn.is_none() {
        state.1 = false;
        true
    } else {
        pressed
    };
    // take hold: of a page lying still, with a page after it to uncover
    if pressed && book.turn.is_none() && book.drag_ok && book.open >= 1.0 {
        let Some(c) = cursor else { return };
        if c.y > crate::STAGE_H - BUTTON_BAND {
            return;
        }
        let Some(hit) = cursor_on(camera, at, c, LEAF_FLOOR.right) else { return };
        // a nail to the corner of a glued leaf
        if book.glued_act().is_some() && at_corner(hit) {
            if let Some(point) = book.start_nail(hit) {
                tell_news(book, hand_news(hit, Some(point as u16 | TUG_BIT)), true);
                play(&mut commands, &art, &assets, &mixer, "book_rustle");
            }
            return;
        }
        if hit.z.abs() > PAGE_H / 2.0 {
            return;
        }
        // the left-hand page turns back to the spread before, when the tale has one
        let taken = if (GUTTER..GUTTER + PAGE_W).contains(&hit.x) {
            book.take_up(Hand::Reader)
        } else if (-(GUTTER + PAGE_W)..-GUTTER).contains(&hit.x) {
            book.take_up_back(Hand::Reader)
        } else {
            false
        };
        if !taken {
            return;
        }
        // a glued leaf is taken by its border, to be peeled
        let peeling = book.turn.as_ref().is_some_and(|t| t.peel.is_some());
        let point = if peeling { book.leaf.nearest_border(hit) } else { book.leaf.nearest(hit) };
        book.leaf.grab = Some((point, book.leaf.points[point]));
        if peeling {
            book.easing = EASE;
        }
        tell_news(book, hand_news(hit, Some(point as u16 | if peeling { PEEL_BIT } else { 0 })), true);
        play(&mut commands, &art, &assets, &mixer, "book_rustle");
        return;
    }
    // a nail at a glued corner: alone it rubs the corner loose stroke by stroke, in a party it
    // holds on while another reader pulls at the opposite corner
    if matches!(book.turn.as_ref().map(|t| &t.hand), Some(Hand::Nail)) {
        book.over_glue = true;
        let Some((point, held)) = book.leaf.grab else { return };
        if !down {
            tell_news(book, hand_news(held, None), true);
            book.leaf.grab = None;
            if let Some(turn) = book.turn.as_mut() {
                turn.hand = Hand::Gone;
            }
            return;
        }
        let Some(hit) = cursor.and_then(|c| cursor_on(camera, at, c, LEAF_FLOOR.right)) else { return };
        let partner = book.other_tug(point);
        if book.shared {
            if partner.is_some() {
                book.worked += time.delta_secs() / TUG_TIME;
            }
        } else if book.rub.to(hit) {
            book.worked += 1.0 / STROKES;
            play(&mut commands, &art, &assets, &mixer, "book_scrap");
        }
        // the corner curls up as the glue under it gives
        let flat = Vec3::new(GUTTER + PAGE_W, LEAF_FLOOR.right, if hit.z < 0.0 { -PAGE_H / 2.0 } else { PAGE_H / 2.0 });
        let rest = Vec3::new(flat.x, flat.y, held.z);
        book.leaf.grab = Some((point, rest + Vec3::Y * CURL * book.worked.min(1.0)));
        tell_news(book, hand_news(hit, Some(point as u16 | TUG_BIT)), false);
        let act = book.turn.as_ref().and_then(|t| t.peel);
        if let (true, Some(act)) = (book.worked >= 1.0, act) {
            book.loosen(act, partner);
            play(&mut commands, &art, &assets, &mixer, "book_peel");
        }
        return;
    }
    // a piece torn loose, carried: lifted a little over whatever it is carried across
    if matches!(book.turn.as_ref().map(|t| &t.hand), Some(Hand::Loose)) {
        match cursor.and_then(|c| cursor_on(camera, at, c, LEAF_FLOOR.right + LIFT)) {
            Some(hand) if down => {
                if let Some((p, _)) = book.leaf.grab {
                    book.leaf.grab = Some((p, hand));
                    tell_news(book, carry_news(0, Some(p), hand), false);
                }
            }
            _ => {
                let hand = book.leaf.grab.map(|(_, h)| h).unwrap_or_default();
                tell_news(book, carry_news(0, None, hand), true);
                book.leaf.grab = None;
                if let Some(turn) = book.turn.as_mut() {
                    turn.hand = Hand::Gone;
                }
            }
        }
        return;
    }
    let holding = matches!(book.turn.as_ref().map(|t| &t.hand), Some(Hand::Reader));
    if !holding {
        // an empty hand hovering over the book
        if let Some(hit) = cursor.and_then(|c| cursor_on(camera, at, c, LEAF_FLOOR.right)) {
            book.over_glue = book.turn.is_none() && book.open >= 1.0 && book.glued_act().is_some() && at_corner(hit);
            tell_news(book, hand_news(hit, None), false);
        }
        return;
    }
    if !down {
        // let go: past the spine the leaf is turned, and the story reads on
        let hand = book.leaf.grab.map(|(_, h)| h).unwrap_or_default();
        let point = hand_point(&book.leaf, hand);
        tell_news(book, hand_news(hand, None), true);
        book.leaf.grab = None;
        let back = book.turn.as_ref().is_some_and(|t| t.back);
        // a torn leaf goes where the hand took it: what is left at the spine may hardly stir
        let side = if book.leaf.torn() { (hand.x / PAGE_W).clamp(-1.0, 1.0) } else { book.leaf.side() };
        // the way the hand was going when it let go decides; with the hand at rest, where it is
        let swing = std::mem::take(&mut book.swing);
        book.last_side = None;
        let over = if !book.leaf.torn() && swing.abs() > FLICK {
            if back { swing > 0.0 } else { swing < 0.0 }
        } else if back {
            side > -0.15
        } else {
            side < 0.15
        };
        let Some(turn) = book.turn.as_mut() else { return };
        turn.hand = Hand::Gone;
        let told = turn.committed;
        // let go past the spine, the leaf is carried on over by the book's own hand, so it lands
        // where the story now is: let go short of it, it falls back and the story stays
        let carry_on = |turn: &mut Turning, leaf: &mut Leaf| {
            // how far over it already is, as the unseen hand's arc would have it
            let s = side.clamp(-1.0, 1.0).acos() / PI;
            let t = if turn.back { 1.0 - s } else { s };
            turn.hand = Hand::Unseen { t, point, z: hand.z };
            // the reader let go: what tears now is no hand's doing
            leaf.tearing = false;
        };
        // caught out of the air after the story had read on: let go past the spine it goes on
        // over; carried back past it, the hand has changed its mind and the story goes back too
        if told {
            let changed_mind = !over && turn.peel.is_none() && !book.leaf.torn();
            if !changed_mind {
                carry_on(turn, &mut book.leaf);
                return;
            }
            let went_back = !back;
            let tell = book.reverse();
            let Some(turn) = book.turn.as_mut() else { return };
            carry_on(turn, &mut book.leaf);
            if tell {
                if went_back {
                    turned_back.write(TurnedBack);
                } else {
                    turned.write(Turned);
                }
            }
            play(&mut commands, &art, &assets, &mixer, "book_flip");
            return;
        }
        if back {
            if over {
                turn.committed = true;
                carry_on(turn, &mut book.leaf);
                if !turn.own {
                    turned_back.write(TurnedBack);
                }
                play(&mut commands, &art, &assets, &mixer, "book_flip");
            }
            return;
        }
        // a leaf peeled off a secret falls where it will; the secret itself, once open, is turned
        // like any page, though the story does not read on
        if over && turn.peel.is_none() {
            turn.committed = true;
            carry_on(turn, &mut book.leaf);
            if !turn.own {
                turned.write(Turned);
            }
            play(&mut commands, &art, &assets, &mixer, "book_flip");
        }
        return;
    }
    // which way the leaf in the hand is swinging, smoothed over a few frames
    let side_now = book.leaf.side();
    if let Some(was) = book.last_side {
        let speed = (side_now - was) / time.delta_secs().max(1e-3);
        book.swing += (speed - book.swing) * 0.25;
    }
    book.last_side = Some(side_now);
    // carry: the point held keeps its distance from the spine, so it rides a dome over the book;
    // a hand that pulls past the dome pulls the paper with it, and paper pulled too far tears
    let (Some(c), Some((point, hand))) = (cursor, book.leaf.grab) else { return };
    // the cursor read at the height the hand holds the paper, so a raised page does not slip
    // away from it in perspective
    let Some(hit) = cursor_on(camera, at, c, hand.y) else { return };
    let (i, j) = book.leaf.home(point);
    let reach = GUTTER + PAGE_W * i as f32 / (NX - 1) as f32;
    let x = hit.x.clamp(-reach, reach);
    let rest_z = book.leaf.height() * (j as f32 / (NZ - 1) as f32 - 0.5);
    let y = LEAF_FLOOR.right + (reach * reach - x * x).max(0.0).sqrt() * 0.75;
    let z = rest_z + (hit.z - rest_z) * 0.35;
    // only a pull well out past where the paper reaches from the spine strains it: a page taken
    // by its middle and carried across the spread is only being turned. A mouse has little room,
    // so a pull counts for more
    let out = Vec2::new(hit.x, hit.z - rest_z);
    let beyond = (out.length() - reach - SLACK).max(0.0);
    // paper being led about never tears of itself: only a real pull, or a jerk at a glued leaf
    let peeling = book.turn.as_ref().is_some_and(|t| t.peel.is_some());
    book.leaf.tearing = peeling || beyond > 0.0;
    let pull = out.normalize_or_zero() * beyond * PULL;
    let aim = Vec3::new(x + pull.x, y, z + pull.y);
    // a hand that took the paper off to one side of the cursor comes to it gently: a jump of the
    // hand counts as a jerk, and would tear what it only just got loose
    let hand = if book.easing > 0.0 {
        book.easing -= time.delta_secs();
        hand.lerp(aim, 0.06)
    } else {
        aim
    };
    book.leaf.grab = Some((point, hand));
    // a page turned back is told the others whole when it lands, not hand by hand: there the
    // hand would take up the right-hand page instead
    let (back, peeling) = book.turn.as_ref().map_or((false, false), |t| (t.back, t.peel.is_some()));
    tell_news(book, hand_news(hand, (!back).then_some(point as u16 | if peeling { PEEL_BIT } else { 0 })), false);
}

/// Dev: `TROPA_GHOST=1` plays a second reader, as their machine would tell of them: a hand that
/// hovers over the book, takes hold of the page's corner and carries it over, then lets go.
#[cfg(feature = "dev")]
fn dev_ghost(time: Res<Time>, mut book: ResMut<Book>, mut since: Local<f32>, mut told: Local<f32>) {
    if std::env::var("TROPA_GHOST").is_err() || !book.wanted {
        return;
    }
    *since += time.delta_secs();
    *told += time.delta_secs();
    // their machine speaks a few times a second, not every frame
    if *told < 1.0 / HAND_RATE {
        return;
    }
    *told = 0.0;
    // `TROPA_GHOST=tug`: they stand pulling at the head corner of a glued leaf
    if std::env::var("TROPA_GHOST").as_deref() == Ok("tug") {
        let color = Color::srgb(0.35, 0.6, 0.95);
        book.remote_hand(1, color, [GUTTER + PAGE_W, LEAF_FLOOR.right, -PAGE_H / 2.0], Some(leaf::at(NX - 1, 0) as u16 | TUG_BIT));
        return;
    }
    let t = *since - 2.0;
    let corner = leaf::at(NX - 1, NZ - 4) as u16;
    let z = PAGE_H * ((NZ - 4) as f32 / (NZ - 1) as f32 - 0.5);
    let color = Color::srgb(0.35, 0.6, 0.95);
    if t < 0.0 {
        book.remote_hand(1, color, [1.2 + t * 0.2, LEAF_FLOOR.right, z + 0.2], None);
    } else if t < 1.4 {
        let a = PI * (t / 1.4).min(1.0);
        book.remote_hand(1, color, [a.cos(), LEAF_FLOOR.right + a.sin() * 0.6, z], Some(corner));
    } else if t < 3.0 {
        book.remote_hand(1, color, [-0.9, LEAF_FLOOR.left + 0.15, z], None);
    }
}

/// The other readers' hands: one that takes hold of the page turns the leaf here as there, and
/// each is shown as a wisp in its hero's colour where it hovers.
fn follow_ghosts(
    time: Res<Time>,
    mut book: ResMut<Book>,
    mut wisps: Query<(&GhostHand, &mut Transform, &mut Visibility, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera: Query<&Transform, (With<BookCamera>, Without<GhostHand>)>,
) {
    let book = &mut *book;
    let dt = time.delta_secs();
    for ghost in book.ghosts.iter_mut() {
        if let Some(g) = ghost {
            g.quiet += dt;
            if g.quiet > GHOST_LINGER || !book.wanted {
                *ghost = None;
            }
        }
    }
    // another reader takes hold of the page lying still
    if book.turn.is_none() && book.open >= 1.0 {
        let holder = book.ghosts.iter().enumerate().find_map(|(seat, g)| g.as_ref().and_then(|g| g.hold.map(|p| (seat as u8, p, g.at, g.peeling))));
        if let Some((seat, point, at, peeling)) = holder {
            // they peel what they and another worked loose, even if this reader had no hand in it
            if let Some(act) = book.glued_act().filter(|_| peeling) {
                book.loose.push(act);
            }
            if book.take_up(Hand::Remote(seat)) && point < book.leaf.points.len() {
                book.leaf.grab = Some((point, at));
            }
        }
    }
    // and carries it, smoothly between the few times a second their machine tells of it
    if let Some(Turning { hand: Hand::Remote(seat), .. }) = book.turn.as_ref() {
        let seat = *seat as usize;
        match book.ghosts[seat].as_ref().and_then(|g| g.hold.map(|p| (p, g.at + g.speed * g.quiet.min(2.0 / HAND_RATE)))) {
            Some((point, at)) if point < book.leaf.points.len() => {
                let from = book.leaf.grab.map(|(_, h)| h).unwrap_or(at);
                book.leaf.grab = Some((point, from.lerp(at, (dt * 20.0).min(1.0))));
            }
            _ => {
                book.leaf.grab = None;
                if let Some(turn) = book.turn.as_mut() {
                    turn.hand = Hand::Gone;
                }
            }
        }
    }
    let Ok(cam) = camera.single() else { return };
    for (wisp, mut tf, mut vis, material) in &mut wisps {
        let ghost = book.ghosts[wisp.0 as usize].as_ref();
        let want = if ghost.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
        let Some(g) = ghost else { continue };
        let to = cam.translation - g.at;
        tf.translation = tf.translation.lerp(g.at + Vec3::Y * 0.03, (dt * 12.0).min(1.0));
        tf.rotation = Quat::from_rotation_y(to.x.atan2(to.z)) * Quat::from_rotation_x(-0.5);
        // a hand that holds the paper burns brighter, and fades as it falls quiet
        let strength = if g.hold.is_some() { 1.6 } else { 1.0 } * (1.0 - g.quiet / GHOST_LINGER).max(0.0);
        tf.scale = Vec3::splat(if g.hold.is_some() { 1.3 } else { 1.0 });
        if let Some(mut m) = materials.get_mut(&material.0) {
            let tint = (g.color.to_linear() * strength).with_alpha(1.0);
            if m.base_color != Color::from(tint) {
                m.base_color = tint.into();
            }
        }
    }
}

/// Runs the leaf's paper, moves the unseen hand, and lays the leaf down when it comes to rest.
fn move_leaf(
    mut commands: Commands,
    time: Res<Time>,
    mut book: ResMut<Book>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut shown: Query<&mut Visibility, With<LeafMesh>>,
    mut torn: MessageWriter<Torn>,
    mut found: MessageWriter<Found>,
    mut turned: MessageWriter<Turned>,
    mut heard_tear: Local<f32>,
    art: Res<Art>,
    assets: Res<AssetServer>,
    mixer: Res<Mixer>,
) {
    let book = &mut *book;
    let seen = book.turn.is_some() || book.kept;
    for mut v in &mut shown {
        let want = if seen { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
    }
    let Some(turn) = book.turn.as_mut() else { return };
    let dt = time.delta_secs();
    if let Hand::Unseen { t, point, z } = &mut turn.hand {
        *t = (*t + dt / AUTO_TURN).min(1.0);
        // lifted at the corner, carried over in an arc, a little ahead of the paper; a page
        // turned back is carried the other way, from the left over to the right
        let eased = *t * *t * (3.0 - 2.0 * *t);
        let a = PI * if turn.back { 1.0 - eased } else { eased };
        let reach = GUTTER + PAGE_W * book.leaf.home(*point).0 as f32 / (NX - 1) as f32;
        book.leaf.grab = Some((*point, Vec3::new(reach * a.cos(), LEAF_FLOOR.right + reach * a.sin() * 0.6, *z)));
        if *t >= 1.0 {
            turn.hand = Hand::Gone;
            book.leaf.grab = None;
        }
    }
    let (cuts, lost) = (book.leaf.cuts(), book.leaf.lost());
    // paper runs at a steady sixty steps a second, whatever the frame rate: how fast glue gives
    // and a tear runs are counted in steps
    book.leaf_clock = (book.leaf_clock + dt).min(4.0 / LEAF_RATE);
    while book.leaf_clock >= 1.0 / LEAF_RATE {
        book.leaf_clock -= 1.0 / LEAF_RATE;
        book.leaf.step(1.0 / LEAF_RATE);
    }
    // a leaf torn here is torn the same way on the others' machines
    let fresh = book.leaf.take_fresh();
    if book.shared && book.leaf.tearing {
        book.news.extend(fresh.into_iter().map(|e| HandNews::Tear(e as u16)));
    }
    // the sound of a tear: a rip as it runs, a long one when a great piece comes away
    *heard_tear -= dt;
    if book.leaf.cuts() > cuts && *heard_tear <= 0.0 {
        let big = book.leaf.lost() - lost > 0.2;
        play(&mut commands, &art, &assets, &mixer, if big { "book_tear_big" } else { "book_tear_small" });
        *heard_tear = if big { 1.2 } else { 0.35 };
    }
    for (handle, back) in [(&book.leaf_meshes.0, false), (&book.leaf_meshes.1, true)] {
        if let Some(mut mesh) = meshes.get_mut(handle) {
            shape_leaf(&mut mesh, &book.leaf, back);
        }
    }
    if let Some(mut mesh) = meshes.get_mut(&book.fringe) {
        shape_fringe(&mut mesh, &book.leaf);
    }
    if !matches!(turn.hand, Hand::Gone) {
        return;
    }
    turn.loose += dt;
    // a whole leaf come down flat on either side is taken as at rest almost at once, so the
    // reader may take it up again (or turn it back) without waiting for the paper to stop
    // stirring; torn paper is left to fall where it will
    let landed = !book.leaf.torn() && turn.loose > LANDED && book.leaf.side().abs() > 0.97;
    if !book.leaf.still() && turn.loose < SETTLE && !landed {
        return;
    }
    let settled = book.settle();
    if let Some((act, whole, mine)) = settled.found {
        found.write(Found { act, whole, mine });
    }
    if let Some((lost, mine)) = settled.torn {
        torn.write(Torn { lost, mine });
    }
    if settled.read_on {
        turned.write(Turned);
    }
}

/// Everything a scrap of torn paper is made of: its meshes, its painters and their pictures.
#[derive(Component)]
struct ScrapPart {
    /// Scraps in the order they were set aside, so the oldest goes first.
    age: u32,
}

/// The most scraps of torn paper kept lying about; each costs two pictures of a page.
const SCRAPS: u32 = 4;

/// Torn leaves set aside stay where they lie, over the pages and on the table, until the book is
/// shut: each a still copy of the leaf with its two faces printed on their own pictures.
#[allow(clippy::too_many_arguments)]
fn keep_scraps(
    mut commands: Commands,
    mut book: ResMut<Book>,
    parts: Query<(Entity, &ScrapPart)>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut count: Local<u32>,
    fonts: Res<Fonts>,
    art: Res<Art>,
    assets: Res<AssetServer>,
) {
    if !book.wanted {
        for (e, _) in &parts {
            commands.entity(e).despawn();
        }
        book.scraps_out.clear();
        return;
    }
    let layer = book.leaf_layer;
    for (leaf, front, back) in std::mem::take(&mut book.scraps_out) {
        *count += 1;
        let age = *count;
        // the oldest scrap makes room
        if age > SCRAPS {
            for (e, part) in &parts {
                if part.age + SCRAPS <= age {
                    commands.entity(e).despawn();
                }
            }
        }
        let part = ScrapPart { age };
        // a hair lower than a leaf lying where it lay, so the next leaf settles over it cleanly
        let place = Transform::from_xyz(0.0, -0.0015, 0.0);
        let shapes = [meshes.add(leaf_mesh(&leaf, false)), meshes.add(leaf_mesh(&leaf, true))];
        for (((spread, half), back_side), shape) in [(front, false), (back, true)].into_iter().zip(shapes.clone()) {
            let tex = target(&mut images, PAGE_PX);
            let cam = commands
                .spawn((ScrapPart { age }, PageCamera, Camera2d, Camera { order: -20, clear_color: ClearColorConfig::Custom(Color::srgb(0.93, 0.88, 0.76)), ..default() }, RenderTarget::Image(tex.clone().into())))
                .id();
            let canvas = commands.spawn((ScrapPart { age }, Node { width: px(PAGE_PX.x as f32), height: px(PAGE_PX.y as f32), ..default() }, UiTargetCamera(cam))).id();
            print_page(&mut commands, canvas, &spread, half, &fonts, &art, &assets);
            let material = materials.add(StandardMaterial { base_color_texture: Some(tex), perceptual_roughness: 0.95, ..default() });
            let mut mesh = commands.spawn((ScrapPart { age }, Mesh3d(shape), MeshMaterial3d(material), place, bevy::light::NotShadowReceiver));
            if back_side {
                mesh.insert(bevy::light::NotShadowCaster);
            }
        }
        let mut fringe = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        shape_fringe(&mut fringe, &leaf);
        let fibres = materials.add(StandardMaterial {
            base_color_texture: Some(images.add(fringe_picture())),
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            perceptual_roughness: 0.95,
            ..default()
        });
        let fringe = meshes.add(fringe);
        commands.spawn((part, Mesh3d(fringe.clone()), MeshMaterial3d(fibres), place, bevy::light::NotShadowCaster, bevy::light::NotShadowReceiver));
        let [front, back] = shapes;
        commands.spawn((ScrapPart { age }, ScrapLeaf { age, leaf, meshes: [front, back, fringe], held: false, theirs: false, active: 0.0, layer, was_held: false }));
    }
}

/// Torn paper another reader carries is carried here too: the leaf torn loose or a scrap follows
/// their hand smoothly between the few times a second their machine tells of it, and falls when
/// they let it go (or when their machine falls silent).
fn carry_remote(time: Res<Time>, mut book: ResMut<Book>, mut scraps: Query<&mut ScrapLeaf>) {
    let book = &mut *book;
    let dt = time.delta_secs();
    let follow = (dt * 20.0).min(1.0);
    for piece in std::mem::take(&mut book.dropped) {
        if piece == 0 {
            if let Some(turn) = book.turn.as_mut().filter(|t| matches!(t.hand, Hand::Carried(_))) {
                turn.hand = Hand::Gone;
                turn.loose = 0.0;
                book.leaf.grab = None;
            }
        } else if let Some(mut s) = scraps.iter_mut().find(|s| s.age == piece && s.theirs) {
            s.held = false;
            s.theirs = false;
            s.leaf.grab = None;
            s.active = SETTLE;
        }
    }
    let Some((seat, piece, point, at, quiet)) = book.carried.as_mut() else { return };
    *quiet += dt;
    let (seat, piece, point, at) = (*seat, *piece, *point as usize, *at);
    if *quiet > GHOST_LINGER {
        book.carried = None;
        book.dropped.push(piece);
        return;
    }
    if piece == 0 {
        // only paper torn loose and lying free, or already in their hand
        let free = match book.turn.as_ref() {
            None => book.kept,
            // paper in one reader's hand is not snatched by another
            Some(t) => matches!(t.hand, Hand::Gone) || matches!(t.hand, Hand::Carried(s) if s == seat),
        };
        if !free || point >= book.leaf.points.len() || book.open < 1.0 {
            return;
        }
        match book.turn.as_mut() {
            Some(turn) => {
                turn.hand = Hand::Carried(seat);
                turn.loose = 0.0;
            }
            None => {
                let Some(shown) = book.shown.clone() else { return };
                book.turn = Some(Turning { from: shown.clone(), to: shown.clone(), story_to: shown, committed: false, hand: Hand::Carried(seat), loose: 0.0, mine: false, peel: None, back: false, own: false });
            }
        }
        // their machine tears it; here it only follows, and is told of the tears
        book.leaf.tearing = false;
        let from = book.leaf.grab.map(|(_, h)| h).unwrap_or(book.leaf.points[point]);
        book.leaf.grab = Some((point, from.lerp(at, follow)));
    } else if let Some(mut s) = scraps.iter_mut().find(|s| s.age == piece && (!s.held || s.theirs)) {
        if point >= s.leaf.points.len() {
            return;
        }
        s.held = true;
        s.theirs = true;
        let from = s.leaf.grab.map(|(_, h)| h).unwrap_or(s.leaf.points[point]);
        s.leaf.grab = Some((point, from.lerp(at, follow)));
    }
}

/// A scrap's paper: still, until a hand takes it up, then run until it lies still again.
#[derive(Component)]
struct ScrapLeaf {
    /// Its place in the order paper came to lie: it lies over those before it.
    layer: u32,
    was_held: bool,
    age: u32,
    leaf: Leaf,
    /// Its two faces and the fibres along its tears.
    meshes: [Handle<Mesh>; 3],
    held: bool,
    /// The hand holding it is another reader's.
    theirs: bool,
    /// Seconds it may yet stir after being let go.
    active: f32,
}

/// Runs the scraps that are in the hand or falling, and lays those under a page that has come
/// down over them out of sight.
fn run_scraps(time: Res<Time>, mut book: ResMut<Book>, mut scraps: Query<(Entity, &mut ScrapLeaf)>, mut meshes: ResMut<Assets<Mesh>>, mut was_turning: Local<bool>) {
    let covered = std::mem::take(&mut book.covered);
    let dt = time.delta_secs();
    let steps = ((dt * LEAF_RATE).round() as usize).clamp(1, 4);
    // paper taken up comes down on top of all that lies
    let turning = book.turn.is_some();
    if turning && !*was_turning {
        book.paper_tick += 1;
        book.leaf_layer = book.paper_tick;
    }
    *was_turning = turning;
    for (_, mut scrap) in &mut scraps {
        if scrap.held && !scrap.was_held {
            book.paper_tick += 1;
            scrap.layer = book.paper_tick;
        }
        scrap.was_held = scrap.held;
    }
    // what moves must not pass through what lies under it, and what lay on it must settle anew
    let stirring = scraps.iter().filter(|(_, s)| s.held || s.active > 0.0).map(|(_, s)| s.layer).min();
    if let Some(from) = stirring {
        for (_, mut scrap) in &mut scraps {
            if scrap.layer > from && !scrap.held {
                scrap.active = scrap.active.max(0.2);
            }
        }
    }
    let under_scraps: Vec<(Entity, crate::book::leaf::Heights)> = scraps
        .iter()
        .filter(|(_, s)| s.held || s.active > 0.0)
        .map(|(e, s)| {
            let mut map = crate::book::leaf::Heights::new(&LEAF_FLOOR);
            for (_, other) in scraps.iter().filter(|(o, other)| *o != e && other.layer < s.layer) {
                map.lay(&other.leaf);
            }
            if !turning && book.leaf_layer < s.layer {
                map.lay(&book.leaf);
            }
            (e, map)
        })
        .collect();
    let mut leaf_map = None;
    for (_, other) in scraps.iter().filter(|(_, other)| other.layer < book.leaf_layer || turning) {
        leaf_map.get_or_insert_with(|| crate::book::leaf::Heights::new(&LEAF_FLOOR)).lay(&other.leaf);
    }
    book.leaf.under = leaf_map;
    for (e, map) in under_scraps {
        if let Ok((_, mut scrap)) = scraps.get_mut(e) {
            scrap.leaf.under = Some(map);
        }
    }
    for (_, mut scrap) in &mut scraps {
        let scrap = &mut *scrap;
        let mut changed = false;
        for half in &covered {
            let on = move |p: Vec3| {
                let x = match half {
                    Half::Left => -p.x,
                    Half::Right => p.x,
                };
                x > 0.0 && x < GUTTER + PAGE_W + 0.01 && p.z.abs() < PAGE_H / 2.0 + 0.01
            };
            changed |= scrap.leaf.sink(on, TOP - 0.003);
        }
        if scrap.held || scrap.active > 0.0 {
            for _ in 0..steps {
                scrap.leaf.step(1.0 / LEAF_RATE);
            }
            if !scrap.held {
                scrap.active -= dt;
                if scrap.leaf.still() {
                    scrap.active = 0.0;
                }
            }
            changed = true;
        }
        if changed {
            let [front, back, fringe] = &scrap.meshes;
            for (handle, back_side) in [(front, false), (back, true)] {
                if let Some(mut mesh) = meshes.get_mut(handle) {
                    shape_leaf(&mut mesh, &scrap.leaf, back_side);
                }
            }
            if let Some(mut mesh) = meshes.get_mut(fringe) {
                shape_fringe(&mut mesh, &scrap.leaf);
            }
        }
    }
}

/// Prints whatever each face of paper is to wear, when that changes.
fn redraw_pages(mut commands: Commands, mut book: ResMut<Book>, canvases: Query<(Entity, &PageCanvas)>, fonts: Res<Fonts>, art: Res<Art>, assets: Res<AssetServer>) {
    for (entity, canvas) in &canvases {
        let f = canvas.0 as usize;
        if book.faces[f] == book.drawn[f] {
            continue;
        }
        let Some((spread, half)) = book.faces[f].clone() else { continue };
        let number = match half {
            Half::Left => spread.numbers.0,
            Half::Right => spread.numbers.1,
        };
        if book.gone.contains(&number) {
            print_stub(&mut commands, entity, half, &art, &assets);
        } else {
            print_page(&mut commands, entity, &spread, half, &fonts, &art, &assets);
        }
        book.drawn[f] = Some((spread, half));
    }
}

/// Where a leaf was torn out: bare paper, nothing printed, and the ragged stub left at the spine.
fn print_stub(commands: &mut Commands, entity: Entity, half: Half, art: &Art, assets: &AssetServer) {
    commands.entity(entity).despawn_children();
    let mut page = commands.entity(entity);
    if let Some(p) = art.image(assets, "book/parchment") {
        page.insert(ImageNode::new(p).with_mode(NodeImageMode::Stretch).with_color(Color::srgb(0.78, 0.72, 0.62)));
    }
    let bare = art.find("book/bare", "png", "cut").map(|f| assets.load::<Image>(f));
    let (w, h) = (PAGE_PX.x as f32, PAGE_PX.y as f32);
    let band = 150.0;
    page.with_children(|p| {
        // down the side of the spine, patch over patch, so its edge is never straight
        for k in 0..7 {
            let y = (k as f32 - 0.5) * h / 6.0;
            let wide = band * (0.7 + 0.6 * hash(k * 13 + 5));
            let left = match half {
                Half::Left => w - wide * 0.7,
                Half::Right => -wide * 0.3,
            };
            let mut node = p.spawn(Node { position_type: PositionType::Absolute, left: px(left), top: px(y), width: px(wide), height: px(h / 4.0), ..default() });
            match &bare {
                Some(b) => node.insert(ImageNode::new(b.clone()).with_mode(NodeImageMode::Stretch).with_color(Color::WHITE.with_alpha(0.95))),
                None => node.insert(BackgroundColor(Color::srgba(0.95, 0.92, 0.85, 0.6))),
            };
        }
    });
}

fn print_page(commands: &mut Commands, entity: Entity, spread: &Spread, half: Half, fonts: &Fonts, art: &Art, assets: &AssetServer) {
    let parchment = art.image(assets, "book/parchment");
    commands.entity(entity).despawn_children();
    let mut page = commands.entity(entity);
    if let Some(p) = &parchment {
        page.insert(ImageNode::new(p.clone()).with_mode(NodeImageMode::Stretch));
    }
    let number = match half {
        Half::Left => spread.numbers.0,
        Half::Right => spread.numbers.1,
    };
    let ink = Color::srgb(0.17, 0.11, 0.07);
    page.with_children(|p| {
        print_content(p, spread, half, fonts, art, assets, 1.0);
        if spread.peeled {
            print_peeled(p, spread, half, fonts, art, assets);
        }
        if number > 0 {
            p.spawn((
                Node { position_type: PositionType::Absolute, left: px(0), bottom: px(56), width: px(PAGE_PX.x as f32), justify_content: JustifyContent::Center, ..default() },
                children![crate::theme::text(number.to_string(), &fonts.num, 40.0, ink.with_alpha(0.8))],
            ));
        }
    });
}

/// A page that was glued face to face with its neighbour and torn apart: in patches its own ink
/// went with the other page and bare fibrous paper shows, and in other patches the other page's
/// ink came with it, mirrored, as it lay against it.
fn print_peeled(p: &mut ChildSpawnerCommands, spread: &Spread, half: Half, fonts: &Fonts, art: &Art, assets: &AssetServer) {
    let other = match half {
        Half::Left => Half::Right,
        Half::Right => Half::Left,
    };
    let (w, h) = (PAGE_PX.x as f32, PAGE_PX.y as f32);
    // where the glue held hardest, decided by the page itself so both machines agree
    let seed = spread.numbers.0 * 31 + if matches!(half, Half::Left) { 0 } else { 7 };
    let patch = |k: u32| {
        let r = |n: u32| hash(seed * 1000 + k * 17 + n);
        let size = Vec2::new(120.0 + r(1) * 260.0, 80.0 + r(2) * 200.0);
        (Vec2::new(r(3) * (w - size.x), 120.0 + r(4) * (h - 300.0 - size.y)), size)
    };
    // the bared paper, where this page's own ink left with the other: a ragged fibrous patch
    // (assets/book/bare-cut.png), turned this way or that so no two look alike
    let bare = art.find("book/bare", "png", "cut").map(|f| assets.load::<Image>(f));
    for k in 0..5 {
        let (at, size) = patch(k);
        // the patch fills only the middle of its picture
        let size = size * 1.6;
        let flip = Vec2::new(if hash(seed + k * 3) < 0.5 { -1.0 } else { 1.0 }, if hash(seed + k * 5) < 0.5 { -1.0 } else { 1.0 });
        let mut node = p.spawn((
            Node { position_type: PositionType::Absolute, left: px(at.x - size.x * 0.2), top: px(at.y - size.y * 0.2), width: px(size.x), height: px(size.y), ..default() },
            UiTransform { scale: flip, ..default() },
        ));
        match &bare {
            Some(b) => node.insert(ImageNode::new(b.clone()).with_mode(NodeImageMode::Stretch).with_color(Color::WHITE.with_alpha(0.85))),
            None => node.insert(BackgroundColor(Color::srgba(0.95, 0.92, 0.85, 0.45))),
        };
    }
    // and the other page's ink, mirrored, where it came away onto this one
    for k in 5..11 {
        let (at, size) = patch(k);
        p.spawn(Node { position_type: PositionType::Absolute, left: px(at.x), top: px(at.y), width: px(size.x), height: px(size.y), overflow: Overflow::clip(), ..default() }).with_children(|clip| {
            clip.spawn((
                Node { position_type: PositionType::Absolute, left: px(-at.x), top: px(-at.y), width: px(w), height: px(h), ..default() },
                UiTransform { scale: Vec2::new(-1.0, 1.0), ..default() },
            ))
            .with_children(|m| print_content(m, spread, other, fonts, art, assets, 0.3));
        });
    }
}

/// The printed matter of one side of a spread: on the left the painting in its frame and the
/// chapter's name, on the right the headpiece, a red initial and the text. `alpha` fades it all.
fn print_content(p: &mut ChildSpawnerCommands, spread: &Spread, half: Half, fonts: &Fonts, art: &Art, assets: &AssetServer, alpha: f32) {
    // ornaments with the paper cut out from under them (tools/inkcut.py), printed onto the page
    let cut = |stem: &str| art.find(stem, "png", "cut").map(|p| assets.load::<Image>(p));
    let ink = Color::srgb(0.17, 0.11, 0.07).with_alpha(alpha);
    let cinnabar = Color::srgb(0.62, 0.13, 0.08).with_alpha(alpha);
    let faded = Color::WHITE.with_alpha(alpha);
    match half {
        Half::Left => {
            // the frame is sliced so its corners keep their knots and its sides repeat along a
            // picture of any shape
            const FRAME_W: f32 = 900.0;
            const BAND: f32 = 84.0;
            let inner_w = FRAME_W - 2.0 * BAND;
            let frame_h = inner_w / art.aspect(&spread.picture).unwrap_or(16.0 / 9.0) + 2.0 * BAND;
            let frame = cut("book/frame").map(|h| {
                ImageNode::new(h)
                    .with_mode(NodeImageMode::Sliced(TextureSlicer {
                        border: BorderRect::all(FRAME_SLICE),
                        center_scale_mode: SliceScaleMode::Stretch,
                        sides_scale_mode: SliceScaleMode::Tile { stretch_value: BAND / FRAME_SLICE },
                        max_corner_scale: BAND / FRAME_SLICE,
                    }))
                    .with_color(faded)
            });
            let picture = match art.image(assets, &spread.picture) {
                Some(h) => ImageNode::new(h).with_mode(NodeImageMode::Stretch).with_color(faded),
                None => ImageNode::solid_color(Color::BLACK.with_alpha(alpha)),
            };
            let left = (PAGE_PX.x as f32 - FRAME_W) / 2.0;
            // the picture runs a little under the frame's inner line, so no paper shows between
            // them; the frame is printed after it, on top
            let tuck = BAND - 6.0;
            p.spawn((
                Node { position_type: PositionType::Absolute, left: px(left + tuck), top: px(150.0 + tuck), width: px(FRAME_W - 2.0 * tuck), height: px(frame_h - 2.0 * tuck), ..default() },
                picture,
            ));
            if let Some(frame) = frame {
                p.spawn((Node { position_type: PositionType::Absolute, left: px(left), top: px(150), width: px(FRAME_W), height: px(frame_h), ..default() }, frame));
            }
            if let Some(title) = &spread.title {
                p.spawn((
                    Node { position_type: PositionType::Absolute, left: px(0), top: px(150.0 + frame_h + 48.0), width: px(PAGE_PX.x as f32), justify_content: JustifyContent::Center, ..default() },
                    children![crate::theme::text(title.clone(), &fonts.title, 64.0, cinnabar)],
                ));
            }
        }
        Half::Right => {
            let mut chars = spread.text.chars();
            let initial: String = chars.next().map(|c| c.to_string()).unwrap_or_default();
            let rest: String = chars.collect();
            if let Some(h) = cut("book/headpiece") {
                // only the band of the square sheet the ornament is painted on
                let band = Rect::new(0.0, 184.0, 1024.0, 758.0);
                let w = 680.0;
                p.spawn((
                    Node { position_type: PositionType::Absolute, left: px((PAGE_PX.x as f32 - w) / 2.0), top: px(70), width: px(w), height: px(w * band.height() / band.width()), ..default() },
                    ImageNode { rect: Some(band), ..ImageNode::new(h).with_mode(NodeImageMode::Stretch).with_color(faded) },
                ));
            }
            p.spawn((
                Node { position_type: PositionType::Absolute, left: px(110), top: px(470), width: px(804), column_gap: px(14), ..default() },
                children![
                    crate::theme::text(initial, &fonts.title, 150.0, cinnabar),
                    (Node { flex_grow: 1.0, padding: UiRect::top(px(10)), ..default() }, children![crate::theme::text(rest, &fonts.body, 44.0, ink)]),
                ],
            ));
        }
    }
}
/// The candle's light breathes, and its flame turns to face the camera.
fn flicker(time: Res<Time>, mut lights: Query<(&Candle, &mut PointLight)>, mut flames: Query<&mut Transform, With<Flame>>, cam: Query<&Transform, (With<BookCamera>, Without<Flame>)>) {
    let t = time.elapsed_secs();
    let wobble = (t * 7.3).sin() * 0.06 + (t * 13.1).sin() * 0.04 + (t * 2.1).sin() * 0.05;
    for (candle, mut light) in &mut lights {
        light.intensity = candle.base * (1.0 + wobble);
    }
    let Ok(cam) = cam.single() else { return };
    for mut tf in &mut flames {
        let to = cam.translation - tf.translation;
        tf.rotation = Quat::from_rotation_y(to.x.atan2(to.z)) * Quat::from_rotation_z(wobble * 0.6);
        tf.scale = Vec3::new(1.0, 1.0 + wobble * 1.5, 1.0);
    }
}

/// The camera leans a little after the cursor, as if the reader bent over the book.
fn sway_camera(window: Single<&Window>, scale: Res<UiScale>, mut cam: Query<&mut Transform, With<BookCamera>>, time: Res<Time>) {
    let Some(c) = crate::stage_cursor(&window, scale.0) else { return };
    let off = (c / Vec2::new(crate::STAGE_W, crate::STAGE_H) - 0.5) * Vec2::new(0.22, 0.12);
    let want = EYE + Vec3::new(off.x, 0.0, off.y);
    for mut tf in &mut cam {
        tf.translation = tf.translation.lerp(want, (time.delta_secs() * 3.0).min(1.0));
        *tf = tf.looking_at(LOOK, Vec3::Y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spread(n: u32, secret: Option<(u8, Spread)>) -> Spread {
        Spread { picture: format!("p{n}"), title: None, text: format!("t{n}"), numbers: (n, n + 1), secret: secret.map(|(a, s)| (a, Box::new(s))), peeled: false }
    }

    fn secret(n: u32) -> Spread {
        Spread { peeled: true, ..spread(n, None) }
    }

    fn book_at(main: Spread) -> Book {
        let mut b = Book::new(Handle::default(), Leaf::new(GUTTER, PAGE_W, PAGE_H, LEAF_FLOOR), (Handle::default(), Handle::default()), Handle::default());
        b.wanted = true;
        b.lay_open();
        b.spread = main;
        b.follow();
        b
    }

    /// Carries the held leaf slowly over the spine, lets go, and lets it come to rest.
    fn carry_over(b: &mut Book, point: usize, back: bool) -> Settled {
        let z = b.leaf.points[point].z;
        let reach = GUTTER + PAGE_W;
        for k in 0..=360 {
            let a = PI * k as f32 / 360.0;
            let a = if back { PI - a } else { a };
            b.leaf.grab = Some((point, Vec3::new(reach * a.cos(), LEAF_FLOOR.right + reach * a.sin() * 0.6, z)));
            b.leaf.step(1.0 / 60.0);
        }
        b.leaf.grab = None;
        for _ in 0..240 {
            b.leaf.step(1.0 / 60.0);
        }
        b.settle()
    }

    fn corner() -> usize {
        leaf::at(leaf::NX - 1, leaf::NZ - 1)
    }

    #[test]
    fn a_glued_pair_taken_up_turns_as_one_past_its_secret() {
        let mut b = book_at(spread(2, Some((1, secret(4)))));
        b.next = Some(spread(6, None));
        assert!(b.take_up(Hand::Reader));
        let t = b.turn.as_ref().unwrap();
        assert_eq!(t.to.numbers.0, 6, "went on to the next page, not to the secret");
        assert!(t.peel.is_none() && !t.own);
    }

    #[test]
    fn a_glued_last_page_does_not_give_way_to_a_plain_pull() {
        let mut b = book_at(spread(2, Some((1, secret(4)))));
        assert!(!b.take_up(Hand::Reader), "nothing to turn to, and the glue holds");
        assert!(b.turn.is_none());
    }

    #[test]
    fn a_nail_needs_several_strokes_back_and_forth() {
        let mut r = Rub::default();
        // one long drag is no rubbing
        assert!((0..50).all(|k| !r.to(Vec3::new(k as f32 * 0.01, 0.0, 0.0))));
        let mut strokes = 0;
        for k in 0..12 {
            let x = if k % 2 == 0 { 0.0 } else { 0.05 };
            for s in 0..5 {
                let v = if k % 2 == 0 { 0.05 - x * 0.0 - 0.01 * s as f32 } else { 0.01 * s as f32 };
                strokes += r.to(Vec3::new(v, 0.0, 0.0)) as i32;
            }
        }
        assert!(strokes >= STROKES as i32, "{strokes}");
    }

    #[test]
    fn only_corners_take_a_nail() {
        assert!(at_corner(Vec3::new(GUTTER + PAGE_W * 0.9, 0.0, PAGE_H * 0.45)));
        assert!(at_corner(Vec3::new(GUTTER + PAGE_W * 0.9, 0.0, -PAGE_H * 0.45)));
        assert!(!at_corner(Vec3::new(GUTTER + PAGE_W * 0.5, 0.0, 0.0)));
        assert!(!at_corner(Vec3::new(GUTTER + PAGE_W * 0.9, 0.0, 0.0)));
        assert!(!at_corner(Vec3::new(-(GUTTER + PAGE_W * 0.9), 0.0, PAGE_H * 0.45)));
    }

    #[test]
    fn the_seal_lies_only_on_the_last_page_that_can_be_reached() {
        let mut b = book_at(spread(2, None));
        b.next = Some(spread(4, None));
        b.pages_ahead = 1;
        assert!(!b.right_page_is_last(), "pages still to come");
        // the leaf lifted off the page before the last uncovers the seal
        assert!(b.take_up(Hand::Reader));
        assert!(b.right_page_is_last());
        // the story has read on while it is in the air
        b.spread = spread(4, None);
        b.next = None;
        b.pages_ahead = 0;
        assert!(b.right_page_is_last());
        b.turn = None;
        b.shown = Some(spread(4, None));
        assert!(b.right_page_is_last());
    }

    #[test]
    fn a_secret_opened_takes_the_seal_from_its_glued_page() {
        let glued = spread(2, Some((1, secret(4))));
        let mut b = book_at(glued.clone());
        b.pages_ahead = 0;
        assert!(b.right_page_is_last(), "unopened, nothing gives it away");
        b.opened.push(1);
        assert!(!b.right_page_is_last(), "the reader knows there is more");
        b.shown = Some(secret(4));
        assert!(b.right_page_is_last(), "the secret is the last page");
        // with pages after it, neither is
        b.pages_ahead = 2;
        assert!(!b.right_page_is_last());
    }

    #[test]
    fn a_leaf_is_caught_out_of_the_air_where_it_is_and_nothing_moves() {
        let mut b = book_at(spread(2, None));
        b.next = Some(spread(4, None));
        assert!(b.take_up(Hand::Reader));
        // lifted and let go: it is falling
        let point = b.leaf.nearest(Vec3::new(GUTTER + PAGE_W, LEAF_FLOOR.right, 0.0));
        b.leaf.grab = Some((point, Vec3::new(0.3, 0.8, 0.0)));
        for _ in 0..40 {
            b.leaf.step(1.0 / 60.0);
        }
        b.leaf.grab = None;
        b.turn.as_mut().unwrap().hand = Hand::Gone;
        let before = b.leaf.points.clone();
        let (shown, faces) = (b.shown.clone(), b.faces.clone());
        assert!(!b.nearly_down());
        assert!(b.catch(point));
        assert_eq!(b.leaf.points, before, "not a point of it has jumped");
        assert_eq!((b.shown.clone(), b.faces.clone()), (shown, faces));
        assert!(matches!(b.turn.as_ref().unwrap().hand, Hand::Reader));
        assert_eq!(b.leaf.grab.map(|(p, h)| (p, h)), Some((point, before[point])));
        // a hand already on it cannot catch it again
        assert!(!b.catch(point));
    }

    #[test]
    fn worked_loose_the_leaf_peels_slowly_onto_its_secret_and_back() {
        let main = spread(2, Some((1, secret(4))));
        let mut b = book_at(main.clone());
        b.next = Some(spread(6, None));
        assert_eq!(b.glued_act(), Some(1));
        // the nail goes in, and the corner comes loose under it: the same hand may pull on
        let point = b.start_nail(Vec3::new(0.9, 0.0, 0.6)).unwrap();
        b.loosen(1, None);
        assert!(matches!(b.turn.as_ref().unwrap().hand, Hand::Reader));
        let s = carry_over(&mut b, point, false);
        assert_eq!(s.found, Some((1, true, true)), "peeled whole, and it is ours");
        assert!(b.on_secret());
        assert_eq!(b.shown.as_ref().unwrap().numbers.0, 4);
        assert_eq!(b.spread, main, "the story has not moved");
        // and back from the secret to the glued page, a turn of the book's own
        assert!(b.take_up_back(Hand::Reader));
        assert!(b.turn.as_ref().unwrap().own);
        let s = carry_over(&mut b, corner(), true);
        assert_eq!(s, Settled::default());
        assert_eq!(b.shown.as_ref(), Some(&main));
        // forward again: no glue left to pull, the secret opens like a page
        assert!(b.take_up(Hand::Reader));
        let t = b.turn.as_ref().unwrap();
        assert!(t.peel.is_none() && t.own && t.to.peeled);
        let s = carry_over(&mut b, corner(), false);
        assert_eq!(s.found, None, "found only once");
        assert!(b.on_secret());
        // on from the secret to the next page: that one the story reads on to
        assert!(b.take_up(Hand::Reader));
        let t = b.turn.as_ref().unwrap();
        assert!(!t.own);
        assert_eq!(t.to.numbers.0, 6);
    }

    #[test]
    fn let_go_short_a_peel_falls_back_and_nothing_is_found() {
        let main = spread(2, Some((1, secret(4))));
        let mut b = book_at(main.clone());
        let point = b.start_nail(Vec3::new(0.9, 0.0, 0.6)).unwrap();
        b.leaf.grab = None;
        for _ in 0..120 {
            b.leaf.step(1.0 / 60.0);
        }
        assert_eq!(b.leaf.home(point).0, leaf::NX - 1);
        assert_eq!(b.settle(), Settled::default());
        assert_eq!(b.shown.as_ref(), Some(&main));
        assert_eq!(b.glued_act(), Some(1), "still glued");
    }

    #[test]
    fn turning_back_goes_to_the_story_page_before_and_lands_there() {
        let (one, two) = (spread(2, None), spread(4, None));
        let mut b = book_at(two.clone());
        b.prev = Some(one.clone());
        assert!(b.take_up_back(Hand::Reader));
        let t = b.turn.as_ref().unwrap();
        assert!(!t.own);
        assert_eq!(t.to, one);
        // the reader carries it over, the story goes back: the book follows, no page is doubled
        b.turn.as_mut().unwrap().committed = true;
        b.spread = one.clone();
        b.prev = None;
        assert!(!b.follow(), "the leaf in the hand is the turn, the unseen hand has no part");
        assert_eq!(b.shown.as_ref(), Some(&one));
        let s = carry_over(&mut b, corner(), true);
        assert_eq!(s, Settled::default());
        assert_eq!(b.faces[Face::Right as usize], Some((one.clone(), Half::Right)));
        assert_eq!(b.faces[Face::Left as usize], Some((one, Half::Left)));
        assert!(!b.take_up_back(Hand::Reader), "the first page has nothing before it");
    }

    #[test]
    fn a_leaf_torn_out_stays_gone_from_the_book() {
        let (one, two) = (spread(2, None), spread(4, None));
        let mut b = book_at(one.clone());
        b.next = Some(two);
        assert!(b.take_up(Hand::Reader));
        tear_off(&mut b);
        b.settle();
        assert!(b.gone.contains(&one.numbers.1), "its right page is gone");
        assert!(b.gone.contains(&4), "and the page on its back");
        b.forget_run();
        assert!(b.gone.is_empty(), "a new run, a new reading");
    }

    #[test]
    fn a_caught_leaf_carried_back_takes_the_story_back_with_it() {
        let (one, two) = (spread(2, None), spread(4, None));
        let mut b = book_at(two.clone());
        b.prev = Some(one.clone());
        assert!(b.take_up_back(Hand::Reader));
        // let go over the spine: the story has gone back a page
        b.turn.as_mut().unwrap().committed = true;
        b.spread = one.clone();
        b.prev = None;
        b.next = Some(two.clone());
        assert!(!b.follow());
        assert_eq!(b.shown.as_ref(), Some(&one));
        // caught in the air and carried back the other way
        assert!(b.reverse(), "the story must be told");
        let t = b.turn.as_ref().unwrap();
        assert!(!t.back);
        assert_eq!((&t.from, &t.to), (&one, &two));
        // the story steps forward again, and the leaf is that turn: no page changes under it
        b.spread = two.clone();
        b.prev = Some(one);
        b.next = None;
        assert!(!b.follow(), "the leaf in the hand is the turn");
        assert_eq!(b.shown.as_ref(), Some(&two));
        b.leaf = Leaf::new(GUTTER, PAGE_W, PAGE_H, LEAF_FLOOR);
        b.leaf.lay_left();
        b.settle();
        assert_eq!(b.faces[Face::Left as usize], Some((two, Half::Left)));
    }

    #[test]
    fn let_go_short_a_turn_back_falls_back() {
        let (one, two) = (spread(2, None), spread(4, None));
        let mut b = book_at(two.clone());
        b.prev = Some(one);
        assert!(b.take_up_back(Hand::Reader));
        for _ in 0..120 {
            b.leaf.step(1.0 / 60.0);
        }
        assert_eq!(b.settle(), Settled::default());
        assert_eq!(b.shown.as_ref(), Some(&two));
        assert_eq!(b.faces[Face::Left as usize], Some((two, Half::Left)));
    }

    #[test]
    fn turning_back_past_an_opened_secret_lands_on_the_secret() {
        let main = spread(2, Some((1, secret(4))));
        let next = spread(6, None);
        let mut b = book_at(next);
        b.opened.push(1);
        b.prev = Some(main.clone());
        assert!(b.take_up_back(Hand::Reader));
        b.turn.as_mut().unwrap().committed = true;
        b.spread = main;
        b.prev = None;
        b.follow();
        assert!(b.on_secret(), "what lay between is what the leaf uncovers");
    }

    #[test]
    fn the_unseen_hand_turns_a_secret_back() {
        let main = spread(2, Some((1, secret(4))));
        let mut b = book_at(main.clone());
        b.opened.push(1);
        b.shown = main.secret.as_ref().map(|(_, s)| (**s).clone());
        assert!(b.turn_back_unseen());
        let t = b.turn.as_ref().unwrap();
        assert!(t.own && t.back && matches!(t.hand, Hand::Unseen { .. }));
    }

    #[test]
    fn the_story_reading_on_turns_a_leaf_and_a_new_book_starts_clean() {
        let mut b = book_at(spread(2, Some((1, secret(4)))));
        b.spread = spread(6, None);
        assert!(b.follow(), "the unseen hand carries the glued pair over");
        assert!(b.turn.as_ref().unwrap().peel.is_none());
        b.opened.push(1);
        b.loose.push(1);
        b.wanted = false;
        b.follow();
        assert!(b.shown.is_none() && b.turn.is_none());
        assert_eq!(b.opened, vec![1], "a secret opened stays open for the rest of the run");
        b.forget_run();
        assert!(b.opened.is_empty() && b.loose.is_empty());
    }

    #[test]
    fn a_corner_two_readers_pull_at() {
        let mut b = book_at(spread(2, Some((1, secret(4)))));
        let point = b.start_nail(Vec3::new(0.9, 0.0, 0.6)).unwrap();
        let color = Color::WHITE;
        assert_eq!(b.other_tug(point), None, "alone in a party nothing gives");
        // a hand at the same corner is no help; one at the opposite corner is
        b.remote_hand(1, color, [0.0; 3], Some(point as u16 | TUG_BIT));
        assert_eq!(b.other_tug(point), None);
        b.remote_hand(1, color, [0.0; 3], Some(leaf::at(leaf::NX - 1, 0) as u16 | TUG_BIT));
        assert_eq!(b.other_tug(point), Some(1));
        assert!(b.ghosts[1].as_ref().unwrap().hold.is_none(), "a tug is not a hand turning the leaf");
        // the lower seat peels, the other lets go and watches
        b.seat = 0;
        b.loosen(1, Some(1));
        assert!(matches!(b.turn.as_ref().unwrap().hand, Hand::Reader));
        let mut c = book_at(spread(2, Some((1, secret(4)))));
        c.start_nail(Vec3::new(0.9, 0.0, 0.6));
        c.seat = 2;
        c.loosen(1, Some(1));
        assert!(matches!(c.turn.as_ref().unwrap().hand, Hand::Gone));
        assert!(c.helped, "whoever helped shares what is found");
    }

    #[test]
    fn a_peeling_hand_is_told_as_such() {
        let mut b = book_at(spread(2, Some((1, secret(4)))));
        b.remote_hand(1, Color::WHITE, [0.0; 3], Some(corner() as u16 | PEEL_BIT));
        let g = b.ghosts[1].as_ref().unwrap();
        assert_eq!(g.hold, Some(corner()));
        assert!(g.peeling);
    }

    #[test]
    fn a_falling_leaf_lands_at_once_when_reached_for() {
        let (one, two, three) = (spread(2, None), spread(4, None), spread(6, None));
        let mut b = book_at(one.clone());
        b.next = Some(two.clone());
        assert!(b.take_up(Hand::Reader));
        // turned, let go, and the story has read on: the leaf is in the air
        let t = b.turn.as_mut().unwrap();
        t.committed = true;
        t.hand = Hand::Gone;
        b.spread = two.clone();
        b.follow();
        b.next = Some(three.clone());
        b.prev = Some(one.clone());
        assert!(b.land_now().is_some());
        assert!(b.turn.is_none());
        assert_eq!(b.faces[Face::Left as usize], Some((two.clone(), Half::Left)));
        assert!(b.take_up(Hand::Reader), "and the next page is there to take");
        assert_eq!(b.turn.as_ref().unwrap().to, three);
    }

    #[test]
    fn a_leaf_let_go_short_lands_where_it_was_and_the_story_stays() {
        let one = spread(2, None);
        let mut b = book_at(one.clone());
        b.next = Some(spread(4, None));
        assert!(b.take_up(Hand::Reader));
        b.turn.as_mut().unwrap().hand = Hand::Gone;
        assert!(b.land_now().is_some());
        assert_eq!(b.shown, Some(one.clone()));
        assert_eq!(b.faces[Face::Right as usize], Some((one, Half::Right)));
    }

    #[test]
    fn a_hand_does_not_land_what_the_story_has_yet_to_catch_up_with() {
        let mut b = book_at(spread(2, None));
        b.next = Some(spread(4, None));
        assert!(b.take_up(Hand::Reader));
        let t = b.turn.as_mut().unwrap();
        t.committed = true;
        t.hand = Hand::Gone;
        assert!(b.land_now().is_none());
    }

    /// Tears the leaf off along the spine, as a hard yank does.
    fn tear_off(b: &mut Book) {
        for e in 0..2000 {
            if b.leaf.lost() > 0.9 {
                break;
            }
            b.leaf.tear_edge(e);
        }
        assert!(b.leaf.lost() > 0.5);
    }

    #[test]
    fn a_page_torn_clean_out_reads_on_and_leaves_one_whole_spread() {
        let (one, two) = (spread(2, None), spread(4, None));
        let mut b = book_at(one);
        b.next = Some(two.clone());
        assert!(b.take_up(Hand::Reader));
        tear_off(&mut b);
        let s = b.settle();
        assert!(s.read_on && s.torn.is_some());
        assert_eq!(b.faces[Face::Left as usize], Some((two.clone(), Half::Left)));
        assert_eq!(b.faces[Face::Right as usize], Some((two.clone(), Half::Right)));
        assert!(!b.kept && !b.leaf.torn() && b.scraps_out.len() == 1, "the paper is a scrap now");
        // the story catches up, with no second page turning over
        b.spread = two.clone();
        assert!(!b.follow());
        assert!(b.turn.is_none());
        assert_eq!(b.shown, Some(two));
    }

    #[test]
    fn a_torn_leaf_turned_back_and_left_hanging_shows_one_spread_not_two_halves() {
        let (one, two) = (spread(2, None), spread(4, None));
        let mut b = book_at(two);
        b.prev = Some(one.clone());
        assert!(b.take_up_back(Hand::Reader));
        b.turn.as_mut().unwrap().committed = true;
        b.spread = one.clone();
        b.prev = None;
        b.follow();
        tear_off(&mut b);
        b.settle();
        assert_eq!(b.faces[Face::Left as usize], Some((one.clone(), Half::Left)));
        assert_eq!(b.faces[Face::Right as usize], Some((one, Half::Right)));
    }

    #[test]
    fn the_last_page_torn_out_has_nothing_to_read_on_to() {
        let main = spread(2, None);
        let mut b = book_at(main.clone());
        b.next = Some(spread(4, None));
        assert!(b.take_up(Hand::Reader));
        b.next = None;
        tear_off(&mut b);
        let s = b.settle();
        assert!(!s.read_on);
        assert_eq!(b.shown, Some(main));
    }

    #[test]
    fn the_cover_lies_shut_when_not_open_and_a_laid_open_book_stays_open() {
        assert!(cover_turn(0.0).angle_between(Quat::IDENTITY) < 1e-3);
        assert!((cover_turn(1.0).angle_between(Quat::IDENTITY) - PI).abs() < 1e-3);
        let b = book_at(spread(2, None));
        assert!(b.lifted && b.open >= 1.0 && b.heard);
    }

}
