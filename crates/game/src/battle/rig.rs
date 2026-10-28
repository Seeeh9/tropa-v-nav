//! A body for a painted figure: each unit is drawn through a shader (`assets/shaders/rig.wesl`)
//! that bends the picture up a spine, feet planted, the way a cut-out puppet would move. Idle,
//! the figure breathes, sways and its cloth and hair ripple; a blow it lands leans it into the
//! foe, a blow it takes bends it back and flashes it; a spell gathers it up and throws it forward.
//! The sprite node keeps its own transform for the larger moves (a step forward, a recoil); the
//! rig adds the bending that a flat picture cannot do by moving as a whole.

use super::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

/// Share of the skin padded around the picture on each side, and above it, so that a bend is
/// never cut off at the picture's edge.
const PAD_X: f32 = 0.12;
const PAD_TOP: f32 = 0.08;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(UiMaterialPlugin::<RigMaterial>::default());
}

#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
pub(super) struct RigMaterial {
    #[uniform(0)]
    pub rig: RigUniform,
    #[texture(1)]
    #[sampler(2)]
    pub picture: Handle<Image>,
}

#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub(super) struct RigUniform {
    /// Time, lean, idle sway, cloth wave.
    pub motion: Vec4,
    /// Chest breath, squash, flash amount, fade.
    pub body: Vec4,
    pub tint: Vec4,
    pub flash: Vec4,
    /// Padding left, right, top; facing.
    pub pad: Vec4,
}

impl UiMaterial for RigMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/rig.wesl".into()
    }
}

/// The figure's skin: the shader-drawn picture inside a sprite node, larger than it by the pads.
#[derive(Component)]
pub(super) struct RigSkin;

/// How the figure is bent this frame, worked out from its animation (see `animate_sprites`).
#[derive(Clone, Copy)]
pub(super) struct Pose {
    pub lean: f32,
    pub sway: f32,
    pub wave: f32,
    pub breath: f32,
    pub squash: f32,
    pub flash: f32,
    pub flash_color: Color,
    pub tint: Color,
    pub fade: f32,
}

impl Pose {
    /// At rest: breathing, swaying a little, cloth stirring.
    pub fn idle(tint: Color, hero: bool) -> Pose {
        Pose { lean: 0.0, sway: if hero { 0.005 } else { 0.007 }, wave: 0.0035, breath: 0.018, squash: 1.0, flash: 0.0, flash_color: Color::WHITE, tint, fade: 1.0 }
    }
}

/// The skin for a picture drawn at `draw` stage pixels, as a child of the sprite node.
pub(super) fn skin(materials: &mut Assets<RigMaterial>, picture: Option<Handle<Image>>, tint: Color, draw: Vec2, hero: bool) -> impl Bundle {
    let w = draw.x / (1.0 - 2.0 * PAD_X);
    let h = draw.y / (1.0 - PAD_TOP);
    let face = if hero { 1.0 } else { -1.0 };
    let material = materials.add(RigMaterial {
        rig: RigUniform { pad: Vec4::new(PAD_X, PAD_X, PAD_TOP, face), tint: tint.to_linear().to_vec4(), body: Vec4::new(0.0, 1.0, 0.0, 1.0), ..default() },
        picture: picture.unwrap_or_default(),
    });
    (
        RigSkin,
        MaterialNode(material),
        Node { position_type: PositionType::Absolute, left: px(-(w - draw.x) / 2.0), top: px(-(h - draw.y)), width: px(w), height: px(h), ..default() },
        Pickable::IGNORE,
    )
}

/// Writes a pose into a skin's material.
pub(super) fn pose(materials: &mut Assets<RigMaterial>, handle: &MaterialNode<RigMaterial>, t: f32, p: Pose) {
    let Some(mut m) = materials.get_mut(&handle.0) else { return };
    m.rig.motion = Vec4::new(t, p.lean, p.sway, p.wave);
    m.rig.body = Vec4::new(p.breath, p.squash, p.flash, p.fade);
    m.rig.tint = p.tint.to_linear().to_vec4();
    m.rig.flash = p.flash_color.to_linear().to_vec4();
}

/// Puts a new picture on a skin (a change of form).
pub(super) fn repaint(materials: &mut Assets<RigMaterial>, handle: &MaterialNode<RigMaterial>, picture: Handle<Image>) {
    if let Some(mut m) = materials.get_mut(&handle.0) {
        m.picture = picture;
    }
}
