//! What a blow throws off: a white slash across whoever is struck and blood spattering away from
//! it, sparks off a shield that stops it, dust kicked up by a lunge; healing and blessings rise
//! off a body as motes, curses and poison hang about it as smoke. All are small UI particles in
//! the float layer, each living a second or less.

use super::*;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Bursts>().add_systems(Update, (burst, fly).chain().after(cue_effects).run_if(in_state(AppState::Battle)));
}

/// Effects waiting to be thrown off, put here by `cue_effects`.
#[derive(Resource, Default)]
pub(super) struct Bursts(pub Vec<Burst>);

pub(super) enum Burst {
    /// A blow drawing blood at `at`, coming from the side `dir` points away from; `weight` 0..1.
    Wound { at: Vec2, dir: f32, weight: f32, blood: Color },
    /// A blow stopped by a shield at `at`.
    Sparks { at: Vec2, dir: f32, weight: f32 },
    /// Feet pushing off the ground.
    Dust { at: Vec2, dir: f32 },
    /// Something good taking hold of a body standing at `at` (its feet), in `color`.
    Motes { at: Vec2, color: Color },
    /// Something foul taking hold.
    Smoke { at: Vec2, color: Color },
}

#[derive(Component)]
struct Particle {
    at: Vec2,
    vel: Vec2,
    /// Downward pull, stage pixels a second squared (negative rises).
    fall: f32,
    /// Slowing, per second.
    drag: f32,
    life: f32,
    ttl: f32,
    size: f32,
    /// Grows (or shrinks, negative) by this share of its size over its life.
    grow: f32,
    color: Color,
    /// Drawn stretched along its flight, this many times longer than wide.
    streak: f32,
}

/// A cheap repeatable scatter, so no random source is needed.
fn scatter(seed: u32, i: u32) -> f32 {
    let x = (seed.wrapping_mul(747_796_405).wrapping_add(i.wrapping_mul(2_891_336_453))) ^ 0x9E37_79B9;
    let x = (x ^ (x >> 15)).wrapping_mul(2_246_822_519);
    ((x ^ (x >> 13)) & 0xFFFF) as f32 / 65_535.0
}

fn burst(mut commands: Commands, mut bursts: ResMut<Bursts>, layer: Single<Entity, With<FloatLayer>>, time: Res<Time>) {
    let seed = (time.elapsed_secs() * 1000.0) as u32;
    let mut spawn = |p: Particle| {
        commands.spawn((
            Node { position_type: PositionType::Absolute, left: px(p.at.x), top: px(p.at.y), width: px(p.size), height: px(p.size), border_radius: BorderRadius::MAX, ..default() },
            BackgroundColor(p.color),
            UiTransform::IDENTITY,
            GlobalZIndex(30),
            Pickable::IGNORE,
            ChildOf(*layer),
            p,
        ));
    };
    for (k, b) in bursts.0.drain(..).enumerate() {
        let r = |i: u32| scatter(seed.wrapping_add(k as u32 * 977), i);
        match b {
            Burst::Wound { at, dir, weight, blood } => {
                // the slash: one long white-hot stroke across the body
                let angle = -0.6 + 0.35 * r(0);
                spawn(Particle { at, vel: Vec2::from_angle(angle) * 900.0 * dir, fall: 0.0, drag: 9.0, life: 0.0, ttl: 0.22, size: 10.0 + 6.0 * weight, grow: -0.6, color: Color::srgb(1.0, 0.97, 0.88), streak: 24.0 });
                let n = 8 + (weight * 26.0) as u32;
                for i in 0..n {
                    let a = (r(i * 3 + 1) - 0.5) * 1.9 - 0.35;
                    let speed = 260.0 + 520.0 * r(i * 3 + 2) * (0.5 + weight);
                    spawn(Particle {
                        at: at + Vec2::new((r(i * 3 + 3) - 0.5) * 40.0, (r(i * 3 + 4) - 0.5) * 60.0),
                        vel: Vec2::new(dir * a.cos(), a.sin()) * speed,
                        fall: 1500.0,
                        drag: 1.2,
                        life: 0.0,
                        ttl: 0.45 + 0.4 * r(i * 3 + 5),
                        size: 5.0 + 9.0 * r(i * 3 + 6) * (0.6 + weight),
                        grow: -0.4,
                        color: blood.mix(&Color::BLACK, 0.3 * r(i * 3 + 7)),
                        streak: 2.2,
                    });
                }
            }
            Burst::Sparks { at, dir, weight } => {
                let n = 10 + (weight * 18.0) as u32;
                for i in 0..n {
                    let a = (r(i * 2) - 0.5) * 2.4;
                    let speed = 500.0 + 700.0 * r(i * 2 + 1);
                    spawn(Particle {
                        at,
                        vel: Vec2::new(-dir * a.cos(), a.sin() - 0.3) * speed,
                        fall: 900.0,
                        drag: 3.0,
                        life: 0.0,
                        ttl: 0.25 + 0.3 * r(i * 2 + 7),
                        size: 4.0 + 3.0 * r(i * 2 + 3),
                        grow: -0.7,
                        color: Color::srgb(0.85, 0.95, 1.0).mix(&Color::srgb(1.0, 0.85, 0.5), r(i * 2 + 5)),
                        streak: 5.0,
                    });
                }
            }
            Burst::Dust { at, dir } => {
                for i in 0..7 {
                    spawn(Particle {
                        at: at + Vec2::new((r(i) - 0.5) * 60.0, -6.0),
                        vel: Vec2::new(-dir * (60.0 + 140.0 * r(i + 9)), -30.0 - 50.0 * r(i + 17)),
                        fall: 40.0,
                        drag: 2.5,
                        life: 0.0,
                        ttl: 0.7 + 0.3 * r(i + 23),
                        size: 22.0 + 20.0 * r(i + 31),
                        grow: 1.4,
                        color: Color::srgba(0.45, 0.38, 0.28, 0.35),
                        streak: 1.0,
                    });
                }
            }
            Burst::Motes { at, color } => {
                for i in 0..12 {
                    spawn(Particle {
                        at: at + Vec2::new((r(i) - 0.5) * 150.0, -20.0 - 220.0 * r(i + 40)),
                        vel: Vec2::new((r(i + 11) - 0.5) * 40.0, -60.0 - 90.0 * r(i + 21)),
                        fall: -60.0,
                        drag: 0.6,
                        life: 0.0,
                        ttl: 0.8 + 0.5 * r(i + 31),
                        size: 6.0 + 7.0 * r(i + 51),
                        grow: -0.5,
                        color: color.with_alpha(0.9),
                        streak: 1.0,
                    });
                }
            }
            Burst::Smoke { at, color } => {
                for i in 0..8 {
                    spawn(Particle {
                        at: at + Vec2::new((r(i) - 0.5) * 120.0, -40.0 - 200.0 * r(i + 40)),
                        vel: Vec2::new((r(i + 11) - 0.5) * 50.0, -20.0 - 30.0 * r(i + 21)),
                        fall: -20.0,
                        drag: 1.0,
                        life: 0.0,
                        ttl: 0.9 + 0.4 * r(i + 31),
                        size: 30.0 + 30.0 * r(i + 51),
                        grow: 1.2,
                        color: color.with_alpha(0.32),
                        streak: 1.0,
                    });
                }
            }
        }
    }
}

fn fly(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Particle, &mut Node, &mut UiTransform, &mut BackgroundColor)>) {
    let dt = time.delta_secs();
    for (e, mut p, mut node, mut tf, mut bg) in &mut q {
        p.life += dt;
        if p.life >= p.ttl {
            commands.entity(e).despawn();
            continue;
        }
        let k = p.life / p.ttl;
        let drag = (1.0 - p.drag * dt).max(0.0);
        p.vel *= drag;
        p.vel.y += p.fall * dt;
        let step = p.vel * dt;
        p.at += step;
        let size = (p.size * (1.0 + p.grow * k)).max(0.5);
        node.left = px(p.at.x - size / 2.0);
        node.top = px(p.at.y - size / 2.0);
        node.width = px(size);
        node.height = px(size);
        // stretched along the flight, the more the faster it goes
        let speed = p.vel.length();
        let stretch = 1.0 + (p.streak - 1.0) * (speed / 600.0).min(1.0);
        tf.rotation = Rot2::radians(p.vel.y.atan2(p.vel.x));
        tf.scale = Vec2::new(stretch, 1.0 / stretch.sqrt());
        let a = p.color.alpha() * (1.0 - k * k);
        bg.0 = p.color.with_alpha(a);
    }
}
