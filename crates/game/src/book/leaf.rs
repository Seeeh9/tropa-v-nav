//! One leaf of paper as a cloth of particles held at the spine, which can be torn.
//!
//! The leaf is a grid of cells. Grid points have a home `(i, j)`: columns `i` run from the spine
//! (0) to the free edge, rows `j` from the head of the page (0, far from the reader) to its foot.
//! At first neighbouring cells share their corner particles. Links between particles keep the
//! paper from stretching (the cells' edges), from shearing (their diagonals) and from bending
//! (links that reach across two and three cells). The column at the spine is glued.
//!
//! Paper tears where it is stretched past what its fibres bear. A torn edge no longer joins the
//! two cells beside it, and every point along the tear is split into one particle per side, so
//! the cut opens cleanly, and a piece cut loose from the spine falls where it will.

use bevy::math::{Vec2, Vec3};

pub const NX: usize = 24;
pub const NZ: usize = 33;
const CX: usize = NX - 1;
const CZ: usize = NZ - 1;

/// Pull of the earth on the leaf, in scene units per second squared (a page is one unit wide).
const GRAVITY: f32 = 5.5;
/// Share of its speed a point keeps from one step to the next: air on a sheet of paper.
const KEEP: f32 = 0.985;
const ITERATIONS: usize = 36;
/// How strongly the links across two cells resist bending, 0..1: paper holds its shape but gives.
const BEND: f32 = 0.6;
/// Stretch, as a share of its length, at which an edge of paper gives way: twice what the
/// briskest calm turn asks of it (about 0.09), so only a pull or a jerk tears.
const TEAR: f32 = 0.18;
/// Edges that may give way in one step: a tear runs, it does not burst.
const TEARS_PER_STEP: usize = 2;
/// Glue: how hard a glued point is held to the page under it each solver pass, and how far it
/// may be drawn off before it lets go. A peel runs only so fast, though: no more than
/// `GLUE_GIVE` points come away in a step, the worst drawn first. A patient pull keeps within
/// that, and the glue lets go along the front of the peel; a jerk outruns it, the paper between
/// the hand and the glue is stretched, and tears. With these, a leaf peeled over a second or more
/// comes away whole, one snatched in a quarter of a second tears.
const GLUE_HOLD: f32 = 0.2;
const GLUE_SLACK: f32 = 0.004;
const GLUE_GIVE: usize = 12;
/// Speed of the hand, in scene units a second, past which peeling a glued leaf is a jerk that
/// tears it: a calm turn carries the corner at one to three.
const JERK: f32 = 6.0;

/// A point of the leaf glued down where it lay.
struct Glue {
    point: usize,
    at: Vec3,
    /// The glue has let go.
    gone: bool,
}

/// Where a leaf lies: the book's two pages, and the table round it.
#[derive(Clone, Copy)]
pub struct Floor {
    /// Height of the right-hand page and of the left-hand one.
    pub right: f32,
    pub left: f32,
    /// Half the size of the book on the table: beyond it lies the table, at height 0.
    pub half: (f32, f32),
}

impl Floor {
    fn at(&self, x: f32, z: f32) -> f32 {
        if x.abs() > self.half.0 || z.abs() > self.half.1 {
            0.002
        } else if x >= 0.0 {
            self.right
        } else {
            self.left
        }
    }
}

/// How high the paper already lying about stands, over the book and the table: a leaf coming down
/// lies on it and bends over its edges instead of passing through.
pub struct Heights {
    half: (f32, f32),
    h: Vec<f32>,
}

const HEIGHT_COLS: usize = 128;
const HEIGHT_ROWS: usize = 96;
/// A sheet's thickness on the one under it.
const SHEET: f32 = 0.003;
/// Paper higher than this over what it lies on is in the air, not lying there.
const LYING: f32 = 0.06;

impl Heights {
    pub fn new(floor: &Floor) -> Heights {
        Heights { half: (floor.half.0 * 1.7, floor.half.1 * 1.7), h: vec![0.0; HEIGHT_COLS * HEIGHT_ROWS] }
    }

    fn cell(&self, x: f32, z: f32) -> (f32, f32) {
        ((x + self.half.0) / (2.0 * self.half.0) * (HEIGHT_COLS - 1) as f32, (z + self.half.1) / (2.0 * self.half.1) * (HEIGHT_ROWS - 1) as f32)
    }

    /// The height at a place, smoothly between cells, so paper bends over an edge.
    fn at(&self, x: f32, z: f32) -> f32 {
        let (cx, cz) = self.cell(x, z);
        if cx < 0.0 || cz < 0.0 || cx >= (HEIGHT_COLS - 1) as f32 || cz >= (HEIGHT_ROWS - 1) as f32 {
            return 0.0;
        }
        let (i, j) = (cx as usize, cz as usize);
        let (u, v) = (cx.fract(), cz.fract());
        let h = |a: usize, b: usize| self.h[b * HEIGHT_COLS + a];
        (h(i, j) * (1.0 - u) + h(i + 1, j) * u) * (1.0 - v) + (h(i, j + 1) * (1.0 - u) + h(i + 1, j + 1) * u) * v
    }

    /// Lays a leaf's paper down on the map: what it covers stands one sheet higher than it does.
    /// Paper sunk under a page or in the air counts for nothing.
    pub fn lay(&mut self, leaf: &Leaf) {
        let reach = leaf.width / (NX - 1) as f32 * 0.8;
        let across = reach / (2.0 * self.half.0) * (HEIGHT_COLS - 1) as f32;
        let down = reach / (2.0 * self.half.1) * (HEIGHT_ROWS - 1) as f32;
        for q in &leaf.points {
            let floor = leaf.floor.at(q.x, q.z);
            if q.y < floor - 1e-3 || q.y > floor + LYING {
                continue;
            }
            let (cx, cz) = self.cell(q.x, q.z);
            let (i0, i1) = ((cx - across).floor().max(0.0) as usize, ((cx + across).ceil() as usize).min(HEIGHT_COLS - 1));
            let (j0, j1) = ((cz - down).floor().max(0.0) as usize, ((cz + down).ceil() as usize).min(HEIGHT_ROWS - 1));
            for j in j0..=j1 {
                for i in i0..=i1 {
                    let slot = &mut self.h[j * HEIGHT_COLS + i];
                    *slot = slot.max(q.y + SHEET);
                }
            }
        }
    }
}

/// One side of a torn edge, for drawing its fringe of fibres: see [`Leaf::rims`].
pub struct Rim {
    pub a: Vec3,
    pub b: Vec3,
    pub into_a: Vec3,
    pub into_b: Vec3,
    pub normal: Vec3,
    pub seed: u32,
}

struct Link {
    a: usize,
    b: usize,
    rest: f32,
    stiffness: f32,
    /// The cell edge this link is, if it is one (`edge` index), and so may tear.
    edge: Option<usize>,
}

pub struct Leaf {
    pub points: Vec<Vec3>,
    prev: Vec<Vec3>,
    /// The grid point each particle stands for.
    home: Vec<(usize, usize)>,
    /// Each cell's corner particles: head-spine, head-edge, foot-spine, foot-edge.
    cells: Vec<[usize; 4]>,
    /// Torn edges, by `edge` index.
    cut: Vec<bool>,
    links: Vec<Link>,
    /// The point a hand holds, and where the hand is.
    pub grab: Option<(usize, Vec3)>,
    /// How fast the hand has lately been moving, and where it was a step ago.
    hand_speed: f32,
    last_hand: Option<Vec3>,
    /// Does the paper tear of itself when pulled too far? Not when another reader's machine
    /// holds the leaf: then its tears come from there, edge by edge, in their order.
    pub tearing: bool,
    /// Edges torn since last asked, in order.
    fresh: Vec<usize>,
    /// Points glued to the page beneath, while the glue holds.
    glue: Vec<Glue>,
    gap: f32,
    width: f32,
    height: f32,
    floor: Floor,
    /// The paper this leaf lies on, if any lies under it.
    pub under: Option<Heights>,
}

/// A repeatable hash of a number to 0..1, the same on every machine.
fn wobble(n: usize) -> f32 {
    let mut x = (n as u32).wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65535.0
}

/// The particle first standing for grid point `(i, j)`.
pub fn at(i: usize, j: usize) -> usize {
    j * NX + i
}

fn cell(ci: usize, cj: usize) -> usize {
    cj * CX + ci
}

/// Edges of the grid: the ones along rows (`i`..`i+1` on row `j`) come first, then the ones
/// down columns (`j`..`j+1` on column `i`).
fn row_edge(i: usize, j: usize) -> usize {
    j * CX + i
}

fn col_edge(i: usize, j: usize) -> usize {
    CX * NZ + i * CZ + j
}

const EDGES: usize = CX * NZ + NX * CZ;

impl Leaf {
    /// A flat leaf over the right-hand page: `gap` from the spine, `width` by `height`.
    pub fn new(gap: f32, width: f32, height: f32, floor: Floor) -> Self {
        let mut leaf = Leaf {
            points: Vec::new(),
            prev: Vec::new(),
            home: Vec::new(),
            cells: Vec::with_capacity(CX * CZ),
            cut: vec![false; EDGES],
            links: Vec::new(),
            grab: None,
            hand_speed: 0.0,
            last_hand: None,
            tearing: true,
            fresh: Vec::new(),
            glue: Vec::new(),
            gap,
            width,
            height,
            floor,
            under: None,
        };
        for j in 0..NZ {
            for i in 0..NX {
                leaf.points.push(leaf.rest(i, j) + Vec3::Y * floor.right);
                leaf.home.push((i, j));
            }
        }
        leaf.prev = leaf.points.clone();
        for cj in 0..CZ {
            for ci in 0..CX {
                leaf.cells.push([at(ci, cj), at(ci + 1, cj), at(ci, cj + 1), at(ci + 1, cj + 1)]);
            }
        }
        leaf.relink();
        leaf
    }

    /// Where grid point `(i, j)` lies on the flat leaf, at height 0. Inside the sheet the points
    /// stand a little off the square grid, each its own way, so a tear running from point to
    /// point wanders as paper does and never runs ruler-straight.
    fn rest(&self, i: usize, j: usize) -> Vec3 {
        let (mut x, mut z) = (i as f32, j as f32);
        if i > 0 && i < CX && j > 0 && j < CZ {
            x += (wobble(i * 977 + j * 131) - 0.5) * 0.6;
            z += (wobble(i * 389 + j * 1013 + 7) - 0.5) * 0.6;
        }
        Vec3::new(self.gap + self.width * x / CX as f32, 0.0, self.height * (z / CZ as f32 - 0.5))
    }

    /// Where on the printed page a particle's paper is, 0..1 across from the spine and down from
    /// the head: its place on the flat leaf.
    pub fn uv(&self, p: usize) -> [f32; 2] {
        let (i, j) = self.home[p];
        let r = self.rest(i, j);
        [(r.x - self.gap) / self.width, r.z / self.height + 0.5]
    }

    pub fn height(&self) -> f32 {
        self.height
    }

    pub fn home(&self, p: usize) -> (usize, usize) {
        self.home[p]
    }

    /// The cells' corners, for drawing: head-spine, head-edge, foot-spine, foot-edge.
    pub fn cells(&self) -> &[[usize; 4]] {
        &self.cells
    }

    fn pinned(&self, p: usize) -> bool {
        self.home[p].0 == 0
    }

    /// Rebuilds the links from the cells and the edges still whole.
    fn relink(&mut self) {
        let mut links = Vec::new();
        fn link(links: &mut Vec<Link>, leaf: &Leaf, a: usize, b: usize, stiffness: f32, edge: Option<usize>) {
            let (ha, hb) = (leaf.home[a], leaf.home[b]);
            let rest = leaf.rest(ha.0, ha.1).distance(leaf.rest(hb.0, hb.1));
            links.push(Link { a, b, rest, stiffness, edge });
        }
        let link = |links: &mut Vec<Link>, a: usize, b: usize, stiffness: f32, edge: Option<usize>| link(links, self, a, b, stiffness, edge);
        for cj in 0..CZ {
            for ci in 0..CX {
                let [hs, he, fs, fe] = self.cells[cell(ci, cj)];
                // the cell's diagonals hold it square
                link(&mut links, hs, fe, 0.8, None);
                link(&mut links, he, fs, 0.8, None);
                // its head and spine-side edges; the foot and far edges belong to the cells
                // beyond, or to this one at the border of the leaf
                link(&mut links, hs, he, 1.0, Some(row_edge(ci, cj)));
                link(&mut links, hs, fs, 1.0, Some(col_edge(ci, cj)));
                if cj + 1 == CZ {
                    link(&mut links, fs, fe, 1.0, Some(row_edge(ci, cj + 1)));
                }
                if ci + 1 == CX {
                    link(&mut links, he, fe, 1.0, Some(col_edge(ci + 1, cj)));
                }
            }
        }
        // a torn edge is not drawn together by its cells' head and spine links either
        links.retain(|l| l.edge.is_none_or(|e| !self.cut[e]));
        // stiffness against bending, across neighbouring cells that are still joined: the short
        // reach smooths a curl, the long one keeps a crease from settling in
        for cj in 0..CZ {
            for ci in 0..CX {
                let here = self.cells[cell(ci, cj)];
                for reach in [1, 2] {
                    let across = (1..=reach).all(|k| ci + k < CX && !self.cut[col_edge(ci + k, cj)]);
                    if across {
                        let there = self.cells[cell(ci + reach, cj)];
                        let s = if reach == 1 { BEND } else { BEND * 0.5 };
                        link(&mut links, here[0], there[1], s, None);
                        link(&mut links, here[2], there[3], s, None);
                    }
                    let down = (1..=reach).all(|k| cj + k < CZ && !self.cut[row_edge(ci, cj + k)]);
                    if down {
                        let there = self.cells[cell(ci, cj + reach)];
                        let s = if reach == 1 { BEND } else { BEND * 0.5 };
                        link(&mut links, here[0], there[2], s, None);
                        link(&mut links, here[1], there[3], s, None);
                    }
                }
            }
        }
        self.links = links;
    }

    /// The grid points at the ends of an edge.
    fn ends(e: usize) -> [(usize, usize); 2] {
        if e < CX * NZ {
            let (i, j) = (e % CX, e / CX);
            [(i, j), (i + 1, j)]
        } else {
            let k = e - CX * NZ;
            let (i, j) = (k / CZ, k % CZ);
            [(i, j), (i, j + 1)]
        }
    }

    /// The edges that cross an edge's line at either of its ends.
    fn crossing(e: usize) -> [Option<usize>; 4] {
        if e < CX * NZ {
            let (i, j) = (e % CX, e / CX);
            [(j > 0).then(|| col_edge(i, j - 1)), (j < CZ).then(|| col_edge(i, j)), (j > 0).then(|| col_edge(i + 1, j - 1)), (j < CZ).then(|| col_edge(i + 1, j))]
        } else {
            let k = e - CX * NZ;
            let (i, j) = (k / CZ, k % CZ);
            [(i > 0).then(|| row_edge(i - 1, j)), (i < CX).then(|| row_edge(i, j)), (i > 0).then(|| row_edge(i - 1, j + 1)), (i < CX).then(|| row_edge(i, j + 1))]
        }
    }

    /// Does this edge touch a torn one, at the tip of a running tear?
    fn at_tip(&self, e: usize) -> bool {
        Self::ends(e).into_iter().any(|p| self.touches_cut(p, e))
    }

    fn touches_cut(&self, (i, j): (usize, usize), not: usize) -> bool {
        [
            (i < CX).then(|| row_edge(i, j)),
            (i > 0).then(|| row_edge(i - 1, j)),
            (j < CZ).then(|| col_edge(i, j)),
            (j > 0).then(|| col_edge(i, j - 1)),
        ]
        .into_iter()
        .flatten()
        .any(|n| n != not && self.cut[n])
    }

    /// Can a tear reach this edge: does it touch the border of the sheet, or a torn edge? An edge
    /// along the border itself parts nothing and never tears.
    fn at_front(&self, e: usize) -> bool {
        let rim = if e < CX * NZ {
            let j = e / CX;
            j == 0 || j == CZ
        } else {
            let i = (e - CX * NZ) / CZ;
            i == 0 || i == CX
        };
        !rim && Self::ends(e).into_iter().any(|(i, j)| i == 0 || i == CX || j == 0 || j == CZ || self.touches_cut((i, j), e))
    }

    /// Tears an edge and splits the points at its ends where the paper round them has parted.
    fn tear(&mut self, e: usize) {
        self.cut[e] = true;
        self.fresh.push(e);
        for (i, j) in Self::ends(e) {
            self.split(i, j);
        }
        self.relink();
    }

    /// Gives each fan of cells still joined round grid point `(i, j)` its own particle.
    fn split(&mut self, i: usize, j: usize) {
        // the up to four cells round the point, with the corner of each that the point is,
        // in order round it: head-spine, head-edge, foot-edge, foot-spine
        let around: [(Option<usize>, usize); 4] = [
            ((i > 0 && j > 0).then(|| cell(i - 1, j - 1)), 3),
            ((i < CX && j > 0).then(|| cell(i, j - 1)), 2),
            ((i < CX && j < CZ).then(|| cell(i, j)), 0),
            ((i > 0 && j < CZ).then(|| cell(i - 1, j)), 1),
        ];
        // the edges between consecutive cells of that ring
        let between = [
            (j > 0).then(|| col_edge(i, j - 1)),
            (i < CX).then(|| row_edge(i, j)),
            (j < CZ).then(|| col_edge(i, j)),
            (i > 0).then(|| row_edge(i - 1, j)),
        ];
        // walk the ring: a whole edge keeps two neighbours in the same fan
        let mut fan = [usize::MAX; 4];
        let mut fans = 0;
        for start in 0..4 {
            if around[start].0.is_none() || fan[start] != usize::MAX {
                continue;
            }
            fan[start] = fans;
            for dir in [1usize, 3] {
                let mut k = start;
                loop {
                    let next = (k + dir) % 4;
                    let edge = if dir == 1 { between[k] } else { between[next] };
                    if around[next].0.is_none() || fan[next] != usize::MAX || edge.is_none_or(|e| self.cut[e]) {
                        break;
                    }
                    fan[next] = fans;
                    k = next;
                }
            }
            fans += 1;
        }
        // the first fan keeps the particle it has, every other fan gets one of its own
        let mut given: Vec<Option<usize>> = vec![None; fans];
        let mut used = Vec::new();
        for k in 0..4 {
            let (Some(c), corner) = around[k] else { continue };
            let f = fan[k];
            let p = self.cells[c][corner];
            let q = *given[f].get_or_insert_with(|| {
                if used.contains(&p) {
                    self.points.push(self.points[p]);
                    self.prev.push(self.prev[p]);
                    self.home.push(self.home[p]);
                    self.points.len() - 1
                } else {
                    p
                }
            });
            used.push(q);
            self.cells[c][corner] = q;
        }
    }

    pub fn step(&mut self, dt: f32) {
        let dt = dt.min(1.0 / 30.0);
        // fingers lift the corner they take: under them the glue is gone at once
        if let Some((held, _)) = self.grab {
            self.glue.retain(|g| g.point != held);
        }
        let moved = match (self.grab, self.last_hand) {
            (Some((_, to)), Some(was)) => to.distance(was) / dt,
            _ => 0.0,
        };
        self.hand_speed += (moved - self.hand_speed) * 0.3;
        self.last_hand = self.grab.map(|(_, to)| to);
        for p in 0..self.points.len() {
            if self.pinned(p) {
                continue;
            }
            let now = self.points[p];
            let moved = (now - self.prev[p]) * KEEP;
            self.prev[p] = now;
            self.points[p] = now + moved - Vec3::Y * GRAVITY * dt * dt;
        }
        for _ in 0..ITERATIONS {
            if let Some((p, to)) = self.grab {
                self.points[p] = to;

            }
            for l in &self.links {
                let (a, b) = (self.points[l.a], self.points[l.b]);
                let d = b - a;
                let len = d.length();
                if len < 1e-6 {
                    continue;
                }
                let fix = d * ((len - l.rest) / len) * l.stiffness;
                match (self.home[l.a].0 == 0, self.home[l.b].0 == 0) {
                    (true, true) => {}
                    (true, false) => self.points[l.b] -= fix,
                    (false, true) => self.points[l.a] += fix,
                    (false, false) => {
                        self.points[l.a] += fix * 0.5;
                        self.points[l.b] -= fix * 0.5;
                    }
                }
            }
            // glued points are held to the page beneath, as long as their glue holds
            for g in &self.glue {
                let q = &mut self.points[g.point];
                *q += (g.at - *q) * GLUE_HOLD;
            }
            // the leaf lies on paper or on the table: it cannot sink into either
            let floor = self.floor;
            let under = self.under.as_ref();
            for q in &mut self.points {
                q.y = q.y.max(floor.at(q.x, q.z)).max(under.map_or(0.0, |h| h.at(q.x, q.z)));
            }
        }
        // paper on paper: a little friction, so a landed leaf does not slide about, but not so
        // much that a bulge cannot spread flat
        for p in 0..self.points.len() {
            let q = self.points[p];
            let lies_at = self.floor.at(q.x, q.z).max(self.under.as_ref().map_or(0.0, |h| h.at(q.x, q.z)));
            if q.y <= lies_at + 1e-4 {
                let prev = &mut self.prev[p];
                prev.x = q.x + (prev.x - q.x) * 0.9;
                prev.z = q.z + (prev.z - q.z) * 0.9;
            }
        }
        // glue drawn off past its slack lets go, but a peel runs only so fast: of the points
        // drawn off, only the worst few come away in a step
        let mut drawn: Vec<(f32, usize)> =
            self.glue.iter().enumerate().map(|(k, g)| (self.points[g.point].distance(g.at), k)).filter(|(off, _)| *off > GLUE_SLACK).collect();
        drawn.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, k) in drawn.into_iter().take(GLUE_GIVE) {
            self.glue[k].gone = true;
        }
        self.glue.retain(|g| !g.gone);
        // what the solver could not take up, the paper's fibres must: of the edges stretched past
        // bearing, the worst gives way. Paper tears from an edge of the sheet or from the tip of
        // a tear already started, where the pull gathers, so only such edges may give
        // A crack runs across the pull, so a link stretched too far parts the paper along an edge
        // that crosses it at one of its ends
        let mut strained: Vec<(f32, usize)> = self
            .links
            .iter()
            .filter_map(|l| {
                let e = l.edge?;
                let s = self.points[l.a].distance(self.points[l.b]) / l.rest - 1.0;
                if s <= TEAR {
                    return None;
                }
                let crossing = Self::crossing(e).into_iter().flatten().filter(|c| !self.cut[*c] && self.at_front(*c));
                // the tip of a tear already running goes first, then the border of the sheet
                let best = crossing.max_by_key(|c| self.at_tip(*c))?;
                Some((s, best))
            })
            .collect();
        // while glue holds the leaf down, the paper strains wherever it is peeled; only a jerk,
        // the hand flying faster than glue can give, tears it
        let jerked = self.glue.is_empty() || self.hand_speed > JERK;
        if self.tearing && jerked && !strained.is_empty() {
            strained.sort_by(|a, b| b.0.total_cmp(&a.0));
            for (_, e) in strained.into_iter().take(TEARS_PER_STEP) {
                if !self.cut[e] {
                    self.tear(e);
                }
            }
            // a hand that held a point now torn off still holds that point
        }
    }

    /// Share of the leaf's cells that are no longer joined to the spine.
    pub fn lost(&self) -> f32 {
        let held = self.held();
        1.0 - held.iter().filter(|h| **h).count() as f32 / held.len() as f32
    }

    /// The torn edges of paper, one per torn edge and side: the edge's two ends `a`, `b` as the
    /// cell on that side has them, each end's step into that cell (`into_a`, `into_b`, unit
    /// vectors), the cell's normal, and the edge's index (to vary the fringe drawn along it).
    pub fn rims(&self) -> Vec<Rim> {
        let mut out = Vec::new();
        for e in (0..EDGES).filter(|e| self.cut[*e]) {
            // the side cells, and which of each one's corners are on the edge and which inside
            let sides: [Option<(usize, [usize; 4])>; 2] = if e < CX * NZ {
                let (i, j) = (e % CX, e / CX);
                // the cell above has the edge at its foot, the one below at its head
                [(j > 0).then(|| (cell(i, j - 1), [2, 3, 0, 1])), (j < CZ).then(|| (cell(i, j), [0, 1, 2, 3]))]
            } else {
                let k = e - CX * NZ;
                let (i, j) = (k / CZ, k % CZ);
                // the cell to the spine side has it at its far edge, the other at its spine edge
                [(i > 0).then(|| (cell(i - 1, j), [1, 3, 0, 2])), (i < CX).then(|| (cell(i, j), [0, 2, 1, 3]))]
            };
            for (c, [ea, eb, ia, ib]) in sides.into_iter().flatten() {
                let corners = self.cells[c].map(|p| self.points[p]);
                let [hs, he, fs, fe] = corners;
                let (a, b) = (corners[ea], corners[eb]);
                out.push(Rim {
                    a,
                    b,
                    into_a: (corners[ia] - a).normalize_or_zero(),
                    into_b: (corners[ib] - b).normalize_or_zero(),
                    normal: (fe - hs).cross(he - fs).normalize_or(Vec3::Y),
                    seed: e as u32,
                });
            }
        }
        out
    }

    /// Lays the leaf over on the left-hand page, as a leaf lies once turned: turned about the
    /// spine, its back up.
    pub fn lay_left(&mut self) {
        let y = self.floor.left;
        for p in 0..self.points.len() {
            let q = &mut self.points[p];
            *q = Vec3::new(-q.x, y, q.z);
            self.prev[p] = *q;
        }
    }

    /// Glues the whole leaf down where it lies, to the page beneath.
    pub fn glue_down(&mut self) {
        self.glue = (0..self.points.len()).filter(|p| !self.pinned(*p)).map(|point| Glue { point, at: self.points[point], gone: false }).collect();
    }

    /// Share of the glue still holding: 1 glued all over, 0 peeled clean away.
    #[cfg(test)]
    pub fn glued(&self) -> f32 {
        let total = (self.points.len() - NZ).max(1) as f32;
        self.glue.len() as f32 / total
    }

    /// Tears an edge torn on another machine. The same edges torn in the same order give the
    /// same particles, so a hand there and here holds the same point.
    pub fn tear_edge(&mut self, e: usize) {
        if e < EDGES && !self.cut[e] {
            self.tear(e);
        }
    }

    /// The edges torn since the last call, in the order they tore.
    pub fn take_fresh(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.fresh)
    }

    /// Has any of the leaf been torn?
    pub fn torn(&self) -> bool {
        self.cut.iter().any(|c| *c)
    }

    /// How many edges of paper have torn.
    pub fn cuts(&self) -> usize {
        self.cut.iter().filter(|c| **c).count()
    }

    /// For each cell, is it still joined to the spine through whole edges?
    pub fn held(&self) -> Vec<bool> {
        let mut held = vec![false; self.cells.len()];
        let mut todo: Vec<usize> = (0..CZ).map(|cj| cell(0, cj)).collect();
        for &c in &todo {
            held[c] = true;
        }
        while let Some(c) = todo.pop() {
            let (ci, cj) = (c % CX, c / CX);
            let near = [
                (ci + 1 < CX).then(|| (cell(ci + 1, cj), col_edge(ci + 1, cj))),
                (ci > 0).then(|| (cell(ci - 1, cj), col_edge(ci, cj))),
                (cj + 1 < CZ).then(|| (cell(ci, cj + 1), row_edge(ci, cj + 1))),
                (cj > 0).then(|| (cell(ci, cj - 1), row_edge(ci, cj))),
            ];
            for (n, e) in near.into_iter().flatten() {
                if !held[n] && !self.cut[e] {
                    held[n] = true;
                    todo.push(n);
                }
            }
        }
        held
    }

    /// How far over the leaf has gone: 1 lying on the right, -1 lying on the left. Only the part
    /// still held at the spine counts.
    pub fn side(&self) -> f32 {
        let held = self.held();
        let edge: Vec<f32> = (0..CZ).filter(|cj| held[cell(CX - 1, *cj)]).map(|cj| self.points[self.cells[cell(CX - 1, cj)][1]].x).collect();
        let x = if edge.is_empty() {
            // torn off at the spine: what is left is a stub, it lies wherever the stub lies
            let stub: Vec<f32> = (0..self.cells.len()).filter(|c| held[*c]).map(|c| self.points[self.cells[c][1]].x).collect();
            stub.iter().sum::<f32>() / stub.len().max(1) as f32
        } else {
            edge.iter().sum::<f32>() / edge.len() as f32
        };
        (x / self.width).clamp(-1.0, 1.0)
    }

    /// Has the leaf come to rest?
    pub fn still(&self) -> bool {
        self.grab.is_none() && self.points.iter().zip(&self.prev).all(|(p, q)| p.distance_squared(*q) < 1e-8)
    }

    /// The free particle nearest the hand's line of sight within `within`, for a hand picking
    /// paper up off the book or the table: `sight` gives where that line meets a level, so paper
    /// lying low on the table is found where it is seen. Of the pieces torn loose only, or
    /// anywhere on the paper, but never under a page (lower than `above`).
    pub fn pick(&self, sight: impl Fn(f32) -> Option<Vec3>, within: f32, only_loose: bool, above: f32) -> Option<usize> {
        let held = self.held();
        let mut loose = vec![!only_loose; self.points.len()];
        if only_loose {
            for (c, corners) in self.cells.iter().enumerate() {
                if !held[c] {
                    for p in corners {
                        loose[*p] = true;
                    }
                }
            }
        }
        let flat = |p: Vec3| sight(p.y).map_or(f32::MAX, |to| Vec2::new(p.x - to.x, p.z - to.z).length());
        (0..self.points.len())
            .filter(|p| loose[*p] && !self.pinned(*p) && self.points[*p].y > above)
            .min_by(|a, b| flat(self.points[*a]).total_cmp(&flat(self.points[*b])))
            .filter(|p| flat(self.points[*p]) < within)
    }

    /// Lays the paper where `covered` says a page now lies over it down under that page, out of
    /// sight, at rest.
    pub fn sink(&mut self, covered: impl Fn(Vec3) -> bool, y: f32) -> bool {
        let mut any = false;
        for p in 0..self.points.len() {
            if !self.pinned(p) && covered(self.points[p]) && self.points[p].y > y {
                self.points[p].y = y;
                self.prev[p] = self.points[p];
                any = true;
            }
        }
        any
    }

    /// The particle nearest to `to`, for a hand to take hold of.
    pub fn nearest(&self, to: Vec3) -> usize {
        (0..self.points.len()).filter(|p| !self.pinned(*p)).min_by(|a, b| self.points[*a].distance_squared(to).total_cmp(&self.points[*b].distance_squared(to))).unwrap_or(NX - 1)
    }

    /// The point on the leaf's free border (its head, foot or outer edge) nearest to `to`: a
    /// glued leaf is peeled from its border, never lifted from the middle.
    pub fn nearest_border(&self, to: Vec3) -> usize {
        (0..self.points.len())
            .filter(|p| {
                let (i, j) = self.home[*p];
                i > 0 && (i == CX || j == 0 || j == CZ)
            })
            .min_by(|a, b| self.points[*a].distance_squared(to).total_cmp(&self.points[*b].distance_squared(to)))
            .unwrap_or(NX - 1)
    }

    /// The surface normal at every particle, facing up while the leaf lies on the right.
    pub fn normals(&self) -> Vec<Vec3> {
        let mut out = vec![Vec3::ZERO; self.points.len()];
        for c in &self.cells {
            let [hs, he, fs, fe] = c.map(|p| self.points[p]);
            // across the cell's diagonals: up while the leaf lies flat on the right
            let n = (fe - hs).cross(he - fs);
            for p in c {
                out[*p] += n;
            }
        }
        out.into_iter().map(|n| n.normalize_or(Vec3::Y)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOOR: Floor = Floor { right: 0.094, left: 0.099, half: (1.06, 0.725) };

    fn leaf() -> Leaf {
        Leaf::new(0.005, 1.0, 1.37, FLOOR)
    }

    fn carry(l: &mut Leaf, point: usize, upto: f32, steps: usize) {
        let z = l.points[point].z;
        for k in 0..=steps {
            let a = std::f32::consts::PI * upto * k as f32 / steps as f32;
            l.grab = Some((point, Vec3::new(a.cos() * 0.95, 0.1 + a.sin() * 0.5, z)));
            l.step(1.0 / 60.0);
        }
        l.grab = None;
    }

    #[test]
    fn a_resting_leaf_stays_put() {
        let mut l = leaf();
        let before = l.points.clone();
        for _ in 0..120 {
            l.step(1.0 / 60.0);
        }
        let drift = l.points.iter().zip(&before).map(|(a, b)| a.distance(*b)).fold(0.0, f32::max);
        assert!(drift < 1e-3, "drifted {drift}");
        assert!(l.side() > 0.95);
    }

    #[test]
    fn carried_over_the_spine_it_falls_to_the_left_whole() {
        let mut l = leaf();
        carry(&mut l, at(NX - 1, NZ - 1), 1.0, 60);
        for _ in 0..240 {
            l.step(1.0 / 60.0);
        }
        assert!(l.side() < -0.85, "side {}", l.side());
        assert!(!l.torn(), "a calm turn tore the page");
        let w = l.points[at(0, NZ / 2)].distance(l.points[at(NX - 1, NZ / 2)]);
        assert!(w < 1.02, "width {w}");
    }

    #[test]
    fn turned_back_from_the_left_it_lands_on_the_right_whole() {
        let mut l = leaf();
        l.lay_left();
        for _ in 0..60 {
            l.step(1.0 / 60.0);
        }
        assert!(!l.torn(), "lying on the left tore it");
        let corner = at(NX - 1, NZ - 1);
        let z = l.points[corner].z;
        for k in 0..=60 {
            let a = std::f32::consts::PI * (1.0 - k as f32 / 60.0);
            l.grab = Some((corner, Vec3::new(a.cos() * 0.95, 0.1 + a.sin() * 0.5, z)));
            l.step(1.0 / 60.0);
        }
        l.grab = None;
        for _ in 0..240 {
            l.step(1.0 / 60.0);
        }
        assert!(!l.torn(), "a calm turn back tore the page");
        assert!(l.side() > 0.85, "side {}", l.side());
    }

    #[test]
    fn turned_back_by_the_middle_it_lands_whole() {
        for forward in [true, false] {
            let mut l = leaf();
            if !forward {
                l.lay_left();
            }
            let p = at(14, 23);
            let reach = 0.005 + 14.0 / CX as f32;
            let z = l.points[p].z;
            for k in 0..=120 {
                let t = k as f32 / 120.0;
                let x = if forward { reach - 2.0 * reach * t } else { -reach + 2.0 * reach * t };
                let y = 0.094 + (reach * reach - x * x).max(0.0).sqrt() * 0.75;
                l.grab = Some((p, Vec3::new(x, y, z)));
                l.step(1.0 / 60.0);
            }
            l.grab = None;
            for _ in 0..240 {
                l.step(1.0 / 60.0);
            }
            assert!(!l.torn(), "forward {forward}: tore");
        }
    }

    #[test]
    fn let_go_early_it_falls_back() {
        let mut l = leaf();
        carry(&mut l, at(NX - 1, NZ - 1), 0.3, 20);
        for _ in 0..300 {
            l.step(1.0 / 60.0);
        }
        assert!(l.side() > 0.85, "side {}", l.side());
    }

    #[test]
    fn yanked_away_from_the_spine_the_corner_tears_off() {
        let mut l = leaf();
        let corner = at(NX - 1, NZ - 1);
        let from = l.points[corner];
        // a jerk: the hand flies off the page in a few frames, and holds on out there
        for k in 1..=40 {
            l.grab = Some((corner, from + Vec3::new(0.12, 0.05, 0.12) * k.min(8) as f32));
            l.step(1.0 / 60.0);
        }
        l.grab = None;
        for _ in 0..240 {
            l.step(1.0 / 60.0);
        }
        assert!(l.torn(), "nothing tore");
        let lost = l.lost();
        assert!(lost > 0.0 && lost < 0.6, "lost {lost}");
        // the torn piece is loose: some of the paper now lies on the table
        assert!(l.points.iter().any(|p| p.y < 0.01), "nothing fell to the table");
    }

    #[test]
    fn tears_replayed_elsewhere_give_the_same_paper() {
        let mut here = leaf();
        let corner = at(NX - 1, NZ - 1);
        let from = here.points[corner];
        for k in 1..=40 {
            here.grab = Some((corner, from + Vec3::new(0.12, 0.05, 0.12) * k.min(8) as f32));
            here.step(1.0 / 60.0);
        }
        let torn = here.take_fresh();
        assert!(!torn.is_empty());
        let mut there = leaf();
        there.tearing = false;
        for e in &torn {
            there.tear_edge(*e);
        }
        assert_eq!(here.points.len(), there.points.len());
        assert_eq!(here.cells, there.cells);
    }

    /// Carries the corner of a glued leaf over the spine in `seconds`, then lets go.
    fn peel(seconds: f32) -> Leaf {
        let mut l = leaf();
        l.glue_down();
        let corner = at(NX - 1, NZ - 1);
        let z = l.points[corner].z;
        let steps = (seconds * 60.0) as usize;
        for k in 0..=steps {
            let a = std::f32::consts::PI * k as f32 / steps as f32;
            l.grab = Some((corner, Vec3::new(a.cos() * 1.0, 0.094 + a.sin() * 0.6, z)));
            l.step(1.0 / 60.0);
        }
        l.grab = None;
        for _ in 0..240 {
            l.step(1.0 / 60.0);
        }
        l
    }

    #[test]
    fn glued_leaves_peel_slowly_and_tear_when_jerked() {
        let slow = peel(4.0);
        assert!(!slow.torn(), "a patient peel tore the page");
        assert!(slow.glued() < 0.02, "glue left {}", slow.glued());
        assert!(slow.side() < -0.8, "side {}", slow.side());
        let jerk = peel(0.25);
        assert!(jerk.torn(), "a jerk peeled it clean");
    }

    #[test]
    fn glue_holds_a_leaf_left_alone() {
        let mut l = leaf();
        l.glue_down();
        for _ in 0..300 {
            l.step(1.0 / 60.0);
        }
        assert!(l.glued() > 0.99);
    }

    #[test]
    fn a_flat_leaf_faces_up() {
        let l = leaf();
        assert!(l.normals().iter().all(|n| n.distance(Vec3::Y) < 1e-4));
    }

    #[test]
    fn a_tear_splits_the_points_along_it() {
        let mut l = leaf();
        let before = l.points.len();
        // cut a line down the middle of the page, head to foot
        for j in 0..CZ {
            l.cut[col_edge(NX / 2, j)] = true;
        }
        for j in 0..NZ {
            l.split(NX / 2, j);
        }
        l.relink();
        assert_eq!(l.points.len(), before + NZ, "every point on the cut was split");
        let lost = l.lost();
        assert!((lost - (CX - NX / 2) as f32 / CX as f32).abs() < 1e-4, "lost {lost}");
    }
}
