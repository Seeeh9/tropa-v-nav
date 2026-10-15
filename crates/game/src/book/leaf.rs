//! One leaf of paper as a grid of particles: a Verlet cloth held at the spine.
//!
//! Points lie in columns `i` running from the spine (0) to the free edge and rows `j` running from
//! the head of the page (0, far from the reader) to its foot. Stretch links keep the paper from
//! growing, shear links keep its corners square, and links that skip a point give it the stiffness
//! of paper against bending. The column at the spine is glued.

use bevy::math::Vec3;

pub const NX: usize = 16;
pub const NZ: usize = 22;

/// Pull of the earth on the leaf, in scene units per second squared (a page is one unit wide).
const GRAVITY: f32 = 5.5;
/// Share of its speed a point keeps from one step to the next: air on a sheet of paper.
const KEEP: f32 = 0.985;
const ITERATIONS: usize = 20;
/// How strongly the skip-one links resist bending, 0..1: paper holds its shape but does give.
const BEND: f32 = 0.6;

struct Link {
    a: usize,
    b: usize,
    rest: f32,
    stiffness: f32,
}

pub struct Leaf {
    pub points: Vec<Vec3>,
    prev: Vec<Vec3>,
    links: Vec<Link>,
    /// The point a hand holds, and where the hand is.
    pub grab: Option<(usize, Vec3)>,
    width: f32,
    height: f32,
    /// Height of the paper the leaf lies on, right of the spine and left of it.
    floor: (f32, f32),
}

pub fn at(i: usize, j: usize) -> usize {
    j * NX + i
}

impl Leaf {
    /// A flat leaf over the right-hand page: `gap` from the spine, `width` by `height`, at `floor.0`.
    pub fn new(gap: f32, width: f32, height: f32, floor: (f32, f32)) -> Self {
        let mut points = Vec::with_capacity(NX * NZ);
        for j in 0..NZ {
            for i in 0..NX {
                let x = gap + width * i as f32 / (NX - 1) as f32;
                let z = height * (j as f32 / (NZ - 1) as f32 - 0.5);
                points.push(Vec3::new(x, floor.0, z));
            }
        }
        let mut links = Vec::new();
        let mut link = |a: usize, b: usize, stiffness: f32, points: &[Vec3]| {
            links.push(Link { a, b, rest: points[a].distance(points[b]), stiffness });
        };
        for j in 0..NZ {
            for i in 0..NX {
                let p = at(i, j);
                if i + 1 < NX {
                    link(p, at(i + 1, j), 1.0, &points);
                }
                if j + 1 < NZ {
                    link(p, at(i, j + 1), 1.0, &points);
                }
                if i + 1 < NX && j + 1 < NZ {
                    link(p, at(i + 1, j + 1), 0.8, &points);
                    link(at(i + 1, j), at(i, j + 1), 0.8, &points);
                }
                // stiffness against bending, over a short and a longer reach: the short one
                // smooths a curl, the long one keeps a crease from settling in
                for (reach, stiffness) in [(2, BEND), (4, BEND * 0.5)] {
                    if i + reach < NX {
                        link(p, at(i + reach, j), stiffness, &points);
                    }
                    if j + reach < NZ {
                        link(p, at(i, j + reach), stiffness, &points);
                    }
                    if i + reach < NX && j + reach < NZ {
                        link(p, at(i + reach, j + reach), stiffness * 0.5, &points);
                        link(at(i + reach, j), at(i, j + reach), stiffness * 0.5, &points);
                    }
                }
            }
        }
        Leaf { prev: points.clone(), points, links, grab: None, width, height, floor }
    }

    pub fn height(&self) -> f32 {
        self.height
    }

    fn pinned(p: usize) -> bool {
        p % NX == 0
    }

    pub fn step(&mut self, dt: f32) {
        let dt = dt.min(1.0 / 30.0);
        for p in 0..self.points.len() {
            if Self::pinned(p) {
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
                match (Self::pinned(l.a), Self::pinned(l.b)) {
                    (true, true) => {}
                    (true, false) => self.points[l.b] -= fix,
                    (false, true) => self.points[l.a] += fix,
                    (false, false) => {
                        self.points[l.a] += fix * 0.5;
                        self.points[l.b] -= fix * 0.5;
                    }
                }
            }
            // the leaf lies on paper: it cannot sink into the block on either side of the spine
            let floor = self.floor;
            for q in &mut self.points {
                q.y = q.y.max(if q.x >= 0.0 { floor.0 } else { floor.1 });
            }
        }
        // paper on paper: a little friction, so a landed leaf does not slide about, but not so
        // much that a bulge cannot spread flat
        for p in 0..self.points.len() {
            let q = self.points[p];
            if q.y <= self.floor_at(q.x) + 1e-4 {
                let prev = &mut self.prev[p];
                prev.x = q.x + (prev.x - q.x) * 0.9;
                prev.z = q.z + (prev.z - q.z) * 0.9;
            }
        }
    }

    fn floor_at(&self, x: f32) -> f32 {
        if x >= 0.0 { self.floor.0 } else { self.floor.1 }
    }

    /// How far over the leaf has gone: 1 lying on the right, -1 lying on the left.
    pub fn side(&self) -> f32 {
        let edge: f32 = (0..NZ).map(|j| self.points[at(NX - 1, j)].x).sum::<f32>() / NZ as f32;
        (edge / self.width).clamp(-1.0, 1.0)
    }

    /// Has the leaf come to rest?
    pub fn still(&self) -> bool {
        self.grab.is_none() && self.points.iter().zip(&self.prev).all(|(p, q)| p.distance_squared(*q) < 1e-8)
    }

    /// The point nearest to `to`, for a hand to take hold of.
    pub fn nearest(&self, to: Vec3) -> usize {
        (0..self.points.len()).filter(|p| !Self::pinned(*p)).min_by(|a, b| self.points[*a].distance_squared(to).total_cmp(&self.points[*b].distance_squared(to))).unwrap_or(NX - 1)
    }

    /// The surface normal at every point, facing up while the leaf lies on the right.
    pub fn normals(&self) -> Vec<Vec3> {
        let mut out = vec![Vec3::ZERO; self.points.len()];
        for j in 0..NZ {
            for i in 0..NX {
                let p = |i: usize, j: usize| self.points[at(i, j)];
                let dx = p((i + 1).min(NX - 1), j) - p(i.saturating_sub(1), j);
                let dz = p(i, (j + 1).min(NZ - 1)) - p(i, j.saturating_sub(1));
                out[at(i, j)] = dz.cross(dx).normalize_or(Vec3::Y);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf() -> Leaf {
        Leaf::new(0.005, 1.0, 1.37, (0.09, 0.095))
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
    fn carried_over_the_spine_it_falls_to_the_left() {
        let mut l = leaf();
        let corner = at(NX - 1, NZ - 1);
        let z = l.points[corner].z;
        // a hand lifts the corner up and over in an arc, then lets go
        for k in 0..=60 {
            let a = std::f32::consts::PI * k as f32 / 60.0;
            l.grab = Some((corner, Vec3::new(a.cos() * 0.95, 0.1 + a.sin() * 0.5, z)));
            l.step(1.0 / 60.0);
        }
        l.grab = None;
        for _ in 0..240 {
            l.step(1.0 / 60.0);
        }
        assert!(l.side() < -0.85, "side {}", l.side());
        // and paper does not stretch on the way
        let w = l.points[at(0, NZ / 2)].distance(l.points[at(NX - 1, NZ / 2)]);
        assert!(w < 1.02, "width {w}");
    }

    #[test]
    fn let_go_early_it_falls_back() {
        let mut l = leaf();
        let corner = at(NX - 1, NZ - 1);
        let z = l.points[corner].z;
        for k in 0..=20 {
            let a = std::f32::consts::PI * 0.3 * k as f32 / 20.0;
            l.grab = Some((corner, Vec3::new(a.cos() * 0.95, 0.1 + a.sin() * 0.5, z)));
            l.step(1.0 / 60.0);
        }
        l.grab = None;
        for _ in 0..300 {
            l.step(1.0 / 60.0);
        }
        assert!(l.side() > 0.85, "side {}", l.side());
    }
}
