// A unit's painted figure, bent as if it had a body: the feet stay planted and everything above
// them moves more the higher it is, as a spine would carry it. The node is drawn larger than the
// picture (`pad`) so that a lean or a sway is never cut off at the picture's edge.
//
// Every warp is applied backwards (the pixel looks up where its paint came from), which is exact
// enough for the small bends a fight needs.

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

struct Rig {
    // time (s), lean (+ toward the foe, in picture widths at the top), idle sway, cloth wave
    motion: vec4<f32>,
    // chest breath, vertical squash (1 = none), flash amount, fade (alpha)
    body: vec4<f32>,
    tint: vec4<f32>,
    flash: vec4<f32>,
    // share of the node padded on the left, on the right and on top of the picture; facing (+1/-1)
    pad: vec4<f32>,
    // a limb's joint on the picture, its turn (radians, clockwise on screen), picture width/height
    limb: vec4<f32>,
    // above zero: seen with no eyes, the figure is only its white outline
    sight: vec4<f32>,
}

@group(1) @binding(0)
var<uniform> rig: Rig;
@group(1) @binding(1)
var picture: texture_2d<f32>;
@group(1) @binding(2)
var picture_sampler: sampler;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let t = rig.motion.x;
    let face = rig.pad.w;
    // from the node to the picture
    let span = vec2<f32>(1.0 - rig.pad.x - rig.pad.y, 1.0 - rig.pad.z);
    var p = (in.uv - vec2<f32>(rig.pad.x, rig.pad.z)) / span;
    // height above the feet: 0 at the bottom edge, 1 at the top
    let h = clamp(1.0 - p.y, 0.0, 1.0);
    // the knees take the first of a bend, the hips and shoulders the most
    let carry = smoothstep(0.04, 0.9, h);
    // squash and stretch, from the feet
    // (not the clamped height: above the top of the picture must stay above it, or a stretch
    // repeats one row of paint up the node)
    let hs = (1.0 - p.y) / max(rig.body.y, 0.2);
    p.y = 1.0 - hs;
    // lean and idle sway: a curve up the spine
    var dx = (rig.motion.y * face) * carry * carry;
    dx += rig.motion.z * sin(t * 1.1) * pow(carry, 1.6);
    // cloth, hair and sleeves ripple, the more the further from the middle of the figure
    let side = abs(p.x - 0.5) * 2.0;
    dx += rig.motion.w * sin(t * 2.7 + h * 8.0) * carry * (0.35 + side);
    p.x -= dx;
    // breath swells the chest a little
    let chest = exp(-pow((h - 0.62) / 0.16, 2.0));
    p.x = 0.5 + (p.x - 0.5) / (1.0 + rig.body.x * chest * (0.5 + 0.5 * sin(t * 2.5)));
    // a limb turns about its joint: look up where its paint was before the turn, measured in
    // true proportions so the turn does not shear the picture
    let k = vec2<f32>(rig.limb.w, 1.0);
    let d = (p - rig.limb.xy) * k;
    let ca = cos(rig.limb.z);
    let sa = sin(rig.limb.z);
    p = rig.limb.xy + vec2<f32>(ca * d.x + sa * d.y, -sa * d.x + ca * d.y) / k;
    // sampled everywhere (a texture is read in uniform control flow), dropped outside the picture
    let c = textureSample(picture, picture_sampler, clamp(p, vec2<f32>(0.0), vec2<f32>(1.0)));
    let inside = step(0.0, p.x) * step(p.x, 1.0) * step(0.0, p.y) * step(p.y, 1.0);
    // what is around this pixel, for the outline
    let e = vec2<f32>(0.004, 0.003);
    let left = textureSample(picture, picture_sampler, clamp(p - vec2<f32>(e.x, 0.0), vec2<f32>(0.0), vec2<f32>(1.0)));
    let right = textureSample(picture, picture_sampler, clamp(p + vec2<f32>(e.x, 0.0), vec2<f32>(0.0), vec2<f32>(1.0)));
    let up = textureSample(picture, picture_sampler, clamp(p - vec2<f32>(0.0, e.y), vec2<f32>(0.0), vec2<f32>(1.0)));
    let down = textureSample(picture, picture_sampler, clamp(p + vec2<f32>(0.0, e.y), vec2<f32>(0.0), vec2<f32>(1.0)));
    if rig.sight.x > 0.5 {
        let shape = length(vec2<f32>(right.a - left.a, down.a - up.a));
        let lum = vec3<f32>(0.3, 0.6, 0.1);
        let ink = length(vec2<f32>(dot(right.rgb, lum) - dot(left.rgb, lum), dot(down.rgb, lum) - dot(up.rgb, lum))) * c.a;
        let edge = clamp(shape * 1.6 + smoothstep(0.10, 0.28, ink) * 0.8, 0.0, 1.0);
        return vec4<f32>(1.0, 1.0, 1.0, edge * rig.body.w * inside);
    }
    var rgb = c.rgb * rig.tint.rgb;
    // the flash is laid on as a painter would, in gamma: mixed in linear light even a little of
    // it lifts every shadow and the whole figure goes pale
    let g = mix(pow(rgb, vec3<f32>(1.0 / 2.2)), pow(rig.flash.rgb, vec3<f32>(1.0 / 2.2)), rig.body.z);
    rgb = pow(g, vec3<f32>(2.2));
    return vec4<f32>(rgb, c.a * rig.tint.a * rig.body.w * inside);
}
