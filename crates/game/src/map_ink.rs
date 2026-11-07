//! Pencil marks on the path map, as in Slay the Spire 2: the right mouse button held down draws a
//! line on the sheet, Shift with it rubs out the marks under the pointer, and Delete wipes them
//! all. The marks are this player's own, kept until the act ends, and never press a node: the
//! nodes take the left button only.

use crate::map_party::{MapScroll, MapSheet, SHEET_H};
use crate::net::Net;
use crate::AppState;
use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.init_resource::<MapInk>().add_systems(Update, (redraw_ink, draw_ink).chain().run_if(in_state(AppState::Map)));
}

/// Strokes in sheet coordinates, and the act they were drawn in.
#[derive(Resource, Default)]
pub struct MapInk {
    act: u8,
    strokes: Vec<Vec<Vec2>>,
    /// A stroke is being drawn now.
    drawing: bool,
}

/// One dot of a pencil stroke, child of the sheet.
#[derive(Component)]
struct InkDot;

/// Pencil grey, a little warm, so it reads on parchment and is not taken for the map's own ink.
const PENCIL: Color = Color::srgba(0.24, 0.20, 0.30, 0.85);
const DOT: f32 = 7.0;
/// How far apart the dots of a stroke are laid.
const STEP: f32 = 4.0;
/// How near the pointer has to come to a stroke to rub it out.
const RUB: f32 = 26.0;

fn dot(commands: &mut Commands, sheet: Entity, p: Vec2) {
    commands.spawn((
        InkDot,
        Node { position_type: PositionType::Absolute, left: px(p.x - DOT / 2.0), top: px(p.y - DOT / 2.0), width: px(DOT), height: px(DOT), border_radius: BorderRadius::MAX, ..default() },
        BackgroundColor(PENCIL),
        GlobalZIndex(5),
        Pickable::IGNORE,
        ChildOf(sheet),
    ));
}

/// The map is drawn anew whenever the run changes: the marks are put back on each new sheet, and
/// forgotten when a new act begins.
fn redraw_ink(mut commands: Commands, mut ink: ResMut<MapInk>, net: Option<Res<Net>>, sheets: Query<Entity, Added<MapSheet>>) {
    let act = net.as_ref().and_then(|n| n.node.state.as_ref()).map(|r| r.act).unwrap_or(0);
    if act != ink.act {
        ink.act = act;
        ink.strokes.clear();
    }
    for sheet in &sheets {
        for stroke in &ink.strokes {
            for p in stroke {
                dot(&mut commands, sheet, *p);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_ink(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window>,
    scale: Res<UiScale>,
    scroll: Res<MapScroll>,
    sheet: Query<Entity, With<MapSheet>>,
    dots: Query<Entity, With<InkDot>>,
    mut ink: ResMut<MapInk>,
) {
    let Ok(sheet) = sheet.single() else { return };
    let respawn = |commands: &mut Commands, ink: &MapInk| {
        for e in &dots {
            commands.entity(e).despawn();
        }
        for stroke in &ink.strokes {
            for p in stroke {
                dot(commands, sheet, *p);
            }
        }
    };
    if keys.just_pressed(KeyCode::Delete) && !ink.strokes.is_empty() {
        ink.strokes.clear();
        respawn(&mut commands, &ink);
        return;
    }
    if !mouse.pressed(MouseButton::Right) {
        ink.drawing = false;
        return;
    }
    let Some(stage) = crate::stage_cursor(&window, scale.0) else { return };
    // below the top bar only, where the sheet shows
    if stage.y < crate::between::BAR_H {
        return;
    }
    let at = stage - scroll.to_stage(Vec2::ZERO);
    if at.y < 0.0 || at.y > SHEET_H {
        return;
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if shift {
        let before = ink.strokes.len();
        ink.strokes.retain(|s| s.iter().all(|p| p.distance(at) > RUB));
        if ink.strokes.len() != before {
            respawn(&mut commands, &ink);
        }
        ink.drawing = false;
        return;
    }
    if !ink.drawing || mouse.just_pressed(MouseButton::Right) {
        ink.drawing = true;
        ink.strokes.push(vec![at]);
        dot(&mut commands, sheet, at);
        return;
    }
    let Some(stroke) = ink.strokes.last_mut() else { return };
    let last = *stroke.last().unwrap_or(&at);
    let gap = last.distance(at);
    if gap < STEP {
        return;
    }
    // fill in between, so a quick stroke is a line and not a row of beads
    let n = (gap / STEP).floor() as i32;
    for k in 1..=n {
        let p = last.lerp(at, k as f32 / n as f32);
        stroke.push(p);
        dot(&mut commands, sheet, p);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_stroke_near_the_pointer_is_rubbed_out_and_the_rest_stay() {
        use bevy::math::Vec2;
        let mut strokes = vec![vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)], vec![Vec2::new(500.0, 500.0)]];
        let at = Vec2::new(12.0, 5.0);
        strokes.retain(|s| s.iter().all(|p| p.distance(at) > super::RUB));
        assert_eq!(strokes.len(), 1);
    }
}
