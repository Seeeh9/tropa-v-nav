//! What a killing blow leaves. A foe cut into parts (see `rig`) loses its head, and on a heavy
//! blow an arm too: each flies off, falls, bounces and rolls along the ground, bleeding as it
//! goes, while the neck and the stump spurt and then seep. The body falls over and stays where it
//! fell until the fight is over. Every drop that reaches the ground stays there as a stain, the
//! more of them the harder a thing came down; heroes bleed bright, the dead and the unclean bleed
//! rot, sap, ichor or dust.

use super::*;
use std::f32::consts::FRAC_PI_2;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Blows>().add_systems(OnEnter(AppState::Battle), |mut b: ResMut<Blows>| b.0.clear()).add_systems(Update, (fall, fly_gibs, bleed).chain().after(dying).run_if(in_state(AppState::Battle)));
}

/// The last blow each unit took: its weight (0..1) and the HP it cost.
#[derive(Resource, Default)]
pub(super) struct Blows(pub std::collections::HashMap<UnitRef, (f32, u32)>);

/// Stains on the ground, under the units.
#[derive(Component)]
pub(super) struct StainLayer;

/// A body that has fallen: which way, how far through its fall, where its feet are.
#[derive(Component)]
pub(super) struct Corpse {
    t: f32,
    side: f32,
    /// The widget's feet, from its top-left corner.
    feet: Vec2,
    /// Stage point of those feet.
    ground: Vec2,
    pub angle: f32,
}

/// A part struck off: flying, bouncing, then rolling, dripping all the while.
#[derive(Component)]
struct Gib {
    at: Vec2,
    vel: Vec2,
    angle: f32,
    spin: f32,
    ground: f32,
    radius: f32,
    weight: f32,
    blood: Color,
    /// Seconds it still bleeds.
    bleed: f32,
    /// Ground covered since the last smear.
    smear: f32,
    still: bool,
}

/// Blood coming out of a body: a spurt that pulses, weakening, and then a seep.
#[derive(Component)]
struct Bleeder {
    /// The body it comes from, and the point on that body (from its widget's top-left).
    corpse: Entity,
    local: Vec2,
    dir: Vec2,
    color: Color,
    t: f32,
    life: f32,
    due: f32,
    strength: f32,
}

/// The colour of what runs in `unit`'s veins.
pub(super) fn blood_of(comfort: &Comfort, shown: &Combat, unit: UnitRef) -> Color {
    if comfort.ink {
        return Color::srgb(0.03, 0.03, 0.05);
    }
    let UnitRef::Enemy(uid) = unit else { return Color::srgb(0.80, 0.03, 0.02) };
    let def = shown.enemy(uid).map(|e| e.def.as_str()).unwrap_or("");
    match def {
        // the drowned and the ghosts: a pale cold ichor
        "nav_soul" | "weeper" | "mara" | "drowned_maiden" | "mavka" | "koschei_shadow" | "swamp_light" | "sirin" => Color::srgb(0.50, 0.60, 0.64),
        // wood and fungus bleed sap
        "leshy" | "tree_guard" | "death_oak" | "death_oak_chest" | "mushroom_corpse" | "lesavka" => Color::srgb(0.30, 0.30, 0.07),
        // bone has only dust in it
        "kostomakha" | "koschei" => Color::srgb(0.42, 0.36, 0.26),
        // the swamp's things run green-black
        "bolotnik" | "kikimora" | "tinyak" | "shishiga" | "yrka" => Color::srgb(0.16, 0.20, 0.06),
        // old dead blood, black and thick
        "koschei_guard" | "black_rider" | "vurdalak_elder" | "volot" | "volot_giant" => Color::srgb(0.13, 0.04, 0.03),
        // the rest rot: a dark brown-red
        _ => Color::srgb(0.36, 0.06, 0.03),
    }
}

/// A stain where a drop of `size` landed at `at`, in `color`.
pub(super) fn stain(commands: &mut Commands, layer: Entity, at: Vec2, size: f32, color: Color, seed: u32) {
    let r = |i: u32| fx::scatter(seed, i);
    let w = size * (1.4 + 0.8 * r(1));
    let h = w * (0.28 + 0.12 * r(2));
    commands.spawn((
        Node { position_type: PositionType::Absolute, left: px(at.x - w / 2.0), top: px(at.y - h / 2.0), width: px(w), height: px(h), border_radius: BorderRadius::MAX, ..default() },
        BackgroundColor(color.mix(&Color::BLACK, 0.25 + 0.25 * r(3)).with_alpha(0.55 + 0.3 * r(4))),
        Pickable::IGNORE,
        ChildOf(layer),
    ));
}

/// The parts of a foe just killed fly off, and its body begins to fall.
#[allow(clippy::too_many_arguments)]
pub(super) fn strike_down(
    commands: &mut Commands,
    widget: Entity,
    origin: Vec2,
    foot: Vec2,
    draw: Vec2,
    hero: bool,
    blood: Color,
    parts: Vec<(rig::Part, Vec2, Handle<Image>, Entity)>,
    images: &Assets<Image>,
    float_layer: Entity,
    seed: u32,
    weight: f32,
) {
    let r = |i: u32| fx::scatter(seed, i);
    // a foe falls back, away from the heroes; a hero the other way
    let side = if hero { -1.0 } else { 1.0 };
    // the picture's top-left on the stage, and the widget's
    let pic = Vec2::new(foot.x - draw.x / 2.0, foot.y - draw.y);
    // the dead lie behind the living
    commands.entity(widget).insert((Corpse { t: 0.0, side, feet: foot - origin, ground: foot, angle: 0.0 }, ZIndex(0)));
    // only a heavy killing blow strikes anything off, and each part takes its own chance: a
    // light one only fells the body, a crushing one may take every part at once
    for (k, (part, joint, picture, skin)) in parts.into_iter().enumerate() {
        let k = k as u32;
        let chance = match part {
            rig::Part::Head => weight * 1.3 - 0.2,
            rig::Part::Arm => weight * 1.1 - 0.25,
            rig::Part::Shield => weight * 0.9 - 0.3,
        };
        let flies = r(70 + k) < chance;
        if !flies {
            continue;
        }
        commands.entity(skin).insert(Visibility::Hidden);
        let (centre, radius) = centre_of(images, &picture).unwrap_or((joint - Vec2::new(0.0, 0.06), 0.08));
        let at = pic + centre * draw;
        let weight = match part {
            rig::Part::Head => 1.0,
            rig::Part::Arm => 0.7,
            rig::Part::Shield => 0.85,
        };
        let up = if part == rig::Part::Head { 900.0 } else { 650.0 };
        // up and back over the body, now and then forward over the heroes' heads
        let back = if r(60 + k) < 0.75 { side } else { -side };
        let vel = Vec2::new(back * (110.0 + 200.0 * r(10 + k)), -(up + 300.0 * r(20 + k)));
        commands
            .spawn((
                Gib {
                    at,
                    vel,
                    angle: 0.0,
                    spin: side * (5.0 + 8.0 * r(30 + k)),
                    ground: foot.y + 10.0 + 50.0 * r(40 + k),
                    radius: radius * draw.y,
                    weight,
                    blood,
                    bleed: 3.5,
                    smear: 0.0,
                    still: false,
                },
                Node { position_type: PositionType::Absolute, left: px(at.x), top: px(at.y), width: px(0), height: px(0), ..default() },
                UiTransform::IDENTITY,
                Pickable::IGNORE,
                ChildOf(float_layer),
            ))
            .with_child((
                ImageNode::new(picture),
                Node { position_type: PositionType::Absolute, left: px(-centre.x * draw.x), top: px(-centre.y * draw.y), width: px(draw.x), height: px(draw.y), ..default() },
                Pickable::IGNORE,
            ));
        // and where it was cut from, the body spurts
        let wound = pic + joint * draw - origin;
        let dir = if part == rig::Part::Head { Vec2::new(side * 0.25, -1.0) } else { Vec2::new(-side * 0.6, -0.5) };
        commands.spawn(Bleeder { corpse: widget, local: wound, dir: dir.normalize(), color: blood, t: 0.0, life: 4.5, due: 0.0, strength: weight });
    }
}

/// Where a part's paint is centred on its picture (0..1), and how far it reaches.
fn centre_of(images: &Assets<Image>, picture: &Handle<Image>) -> Option<(Vec2, f32)> {
    let image = images.get(picture)?;
    let data = image.data.as_ref()?;
    let (w, h) = (image.width() as usize, image.height() as usize);
    if data.len() < w * h * 4 {
        return None;
    }
    let (mut sx, mut sy, mut n) = (0.0f64, 0.0f64, 0.0f64);
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in (0..h).step_by(4) {
        for x in (0..w).step_by(4) {
            if data[(y * w + x) * 4 + 3] > 128 {
                sx += x as f64;
                sy += y as f64;
                n += 1.0;
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    (n > 0.0).then(|| {
        let reach = ((x1 - x0) as f32 / w as f32).max((y1 - y0) as f32 / h as f32) * 0.5;
        (Vec2::new((sx / n) as f32 / w as f32, (sy / n) as f32 / h as f32), reach.min(0.2))
    })
}

/// A body topples about its feet, slowly and then all at once, and settles with a bump.
fn fall(time: Res<Time>, mut q: Query<(&mut Corpse, &mut UiTransform, &ComputedNode)>) {
    let dt = time.delta_secs();
    for (mut c, mut tf, node) in &mut q {
        c.t += dt;
        let p = ((c.t - 0.15) / 0.75).clamp(0.0, 1.0);
        // gravity: the fall gathers speed; at the end it hits and rocks a little
        let ease = p * p;
        let bump = if p >= 1.0 { 0.06 * (-(c.t - 0.9) * 7.0).exp() * ((c.t - 0.9) * 18.0).sin() } else { 0.0 };
        c.angle = c.side * (FRAC_PI_2 * 0.93 * ease - bump);
        let size = node.size() * node.inverse_scale_factor();
        // a transform turns about the middle: move the middle so the feet stay put
        let arm = size / 2.0 - c.feet;
        let turned = Vec2::from_angle(c.angle).rotate(arm);
        tf.rotation = Rot2::radians(c.angle);
        tf.translation = Val2::px(turned.x - arm.x, turned.y - arm.y);
    }
}

fn fly_gibs(
    mut commands: Commands,
    time: Res<Time>,
    stains: Option<Single<Entity, With<StainLayer>>>,
    mut gibs: Query<(&mut Gib, &mut Node, &mut UiTransform)>,
    mut seed: Local<u32>,
) {
    let dt = time.delta_secs().min(0.05);
    let Some(stains) = stains.map(|s| *s) else { return };
    for (mut g, mut node, mut tf) in &mut gibs {
        *seed = seed.wrapping_add(1);
        let r = |i: u32| fx::scatter(*seed, i);
        if !g.still {
            g.vel.y += 2400.0 * dt;
            let step = g.vel * dt;
            g.at += step;
            g.angle += g.spin * dt;
            let floor = g.ground - g.radius * 0.55;
            if g.at.y >= floor {
                g.at.y = floor;
                if g.vel.y > 140.0 {
                    // a hit on the ground: the harder and heavier, the more it splashes
                    let hit = g.vel.y * g.weight;
                    let n = (hit / 90.0).clamp(2.0, 18.0) as u32;
                    for i in 0..n {
                        let off = Vec2::new((r(i * 2) - 0.5) * g.radius * 3.0 * (hit / 800.0).min(2.0), (r(i * 2 + 1) - 0.5) * 18.0);
                        stain(&mut commands, stains, Vec2::new(g.at.x, g.ground) + off, 6.0 + 16.0 * r(i + 50) * (hit / 900.0).min(1.5), g.blood, *seed + i);
                    }
                    g.vel.y = -g.vel.y * 0.32;
                    g.vel.x *= 0.72;
                    g.spin = g.vel.x / g.radius.max(1.0);
                } else {
                    // rolling: it turns as it goes, slowing in the dirt
                    g.vel.y = 0.0;
                    g.vel.x *= (1.0 - 2.2 * dt).max(0.0);
                    g.spin = g.vel.x / g.radius.max(1.0);
                    g.smear += step.x.abs();
                    if g.bleed > 0.0 && g.smear > 16.0 {
                        g.smear = 0.0;
                        stain(&mut commands, stains, Vec2::new(g.at.x, g.ground + (r(7) - 0.5) * 6.0), 5.0 + 7.0 * g.weight * (g.bleed / 3.5), g.blood, *seed);
                    }
                    if g.vel.x.abs() < 6.0 {
                        g.still = true;
                    }
                }
            }
            // the edges of the clearing throw it back
            let (lo, hi) = (60.0, STAGE_W - 60.0);
            if (g.at.x < lo && g.vel.x < 0.0) || (g.at.x > hi && g.vel.x > 0.0) {
                g.vel.x = -g.vel.x * 0.5;
                g.spin = -g.spin * 0.5;
            }
            // come to rest, a head lies on its side or face, an arm along the ground
            if g.still {
                let lie = if g.weight < 0.9 { std::f32::consts::PI } else { FRAC_PI_2 };
                g.angle = (g.angle / lie).round() * lie;
            }
            node.left = px(g.at.x);
            node.top = px(g.at.y);
            tf.rotation = Rot2::radians(g.angle);
        }
        // a struck-off part keeps dripping where it lies, less and less
        if g.bleed > 0.0 {
            g.bleed -= dt;
            if r(99) < dt * 6.0 * (g.bleed / 3.5) {
                let drop_at = g.at + Vec2::new((r(98) - 0.5) * g.radius, 0.0);
                if g.still {
                    stain(&mut commands, stains, Vec2::new(drop_at.x, g.ground + (r(97) - 0.5) * 8.0), 5.0 + 6.0 * r(96), g.blood, *seed + 3);
                }
            }
        }
    }
}

/// A wound on a body pumps blood out in pulses, weakening, then only seeps; every drop lands as
/// a stain. The wound moves with the body as it falls.
fn bleed(
    mut commands: Commands,
    time: Res<Time>,
    mut bursts: ResMut<fx::Bursts>,
    corpses: Query<(&Corpse, &UiGlobalTransform, &ComputedNode)>,
    mut bleeders: Query<(Entity, &mut Bleeder)>,
    scale: Res<UiScale>,
) {
    let dt = time.delta_secs();
    for (e, mut b) in &mut bleeders {
        b.t += dt;
        if b.t >= b.life {
            commands.entity(e).despawn();
            continue;
        }
        let Ok((corpse, global, node)) = corpses.get(b.corpse) else {
            commands.entity(e).despawn();
            continue;
        };
        b.due -= dt;
        if b.due > 0.0 {
            continue;
        }
        // where the wound is now: the corpse's centre on the stage, and the wound turned with it
        let size = node.size() * node.inverse_scale_factor();
        let centre = global.translation / scale.0;
        let at = centre + Vec2::from_angle(corpse.angle).rotate(b.local - size / 2.0);
        let dir = Vec2::from_angle(corpse.angle).rotate(b.dir);
        // a spurt with every beat for the first two seconds, weaker each time; then a seep
        let spurting = b.t < 2.0;
        let beat = 0.42;
        let force = if spurting { b.strength * (1.0 - b.t / 2.4) } else { 0.15 * b.strength * (1.0 - (b.t - 2.0) / (b.life - 2.0)) };
        bursts.0.push(fx::Burst::Spurt { at, dir, force, blood: b.color, ground: corpse.ground.y });
        b.due = if spurting { beat } else { 0.25 };
    }
}
