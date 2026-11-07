//! Pencil marks on the path map, as in Slay the Spire 2: the right mouse button held down draws a
//! line on the sheet, or rubs out the marks under the pointer when the eraser is the tool (or
//! Shift is held); the little tray at the left picks the tool and wipes this player's own marks,
//! as Delete does. The marks are this player's own, kept until the act ends, and never press a
//! node: the nodes take the left button only.

use crate::l10n::L10n;
use crate::map_party::{MapScroll, MapSheet, SHEET_H};
use crate::net::Net;
use crate::theme::{self, Fonts};
use crate::{AppState, Stage};
use bevy::prelude::*;
use bevy::ui_widgets::{observe, Activate, Button};

pub fn plugin(app: &mut App) {
    app.init_resource::<MapInk>().add_systems(Update, (redraw_ink, tray, light_tray, draw_ink).chain().run_if(in_state(AppState::Map)));
}

/// What the right button does on the map.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Tool {
    #[default]
    Pencil,
    Eraser,
}

/// Strokes in sheet coordinates, and the act they were drawn in.
#[derive(Resource, Default)]
pub struct MapInk {
    act: u8,
    strokes: Vec<Vec<Vec2>>,
    /// A stroke is being drawn now.
    drawing: bool,
    pub tool: Tool,
}

/// One piece of a pencil stroke, child of the sheet.
#[derive(Component)]
struct InkDot;

/// The tray of pencil tools, and the buttons that pick one.
#[derive(Component)]
struct InkTray;
#[derive(Component)]
struct ToolButton(Tool);

/// Pencil grey, a little warm, so it reads on parchment and is not taken for the map's own ink.
const PENCIL: Color = Color::srgba(0.24, 0.20, 0.30, 0.85);
/// The line's thickness.
const DOT: f32 = 6.0;
/// How far apart the points of a stroke are kept; the line is drawn whole between them.
const STEP: f32 = 6.0;
/// How near the pointer has to come to a stroke to rub it out.
const RUB: f32 = 26.0;
const EDGE: Color = Color::srgb(0.42, 0.33, 0.22);

/// A piece of line from `a` to `b`: a bar with round ends turned along the way, so the pieces of a
/// stroke overlap into one unbroken line (a lone point is a round dot).
fn piece(commands: &mut Commands, sheet: Entity, a: Vec2, b: Vec2) {
    let d = b - a;
    let mid = (a + b) / 2.0;
    let w = d.length() + DOT;
    commands.spawn((
        InkDot,
        Node { position_type: PositionType::Absolute, left: px(mid.x - w / 2.0), top: px(mid.y - DOT / 2.0), width: px(w), height: px(DOT), border_radius: BorderRadius::MAX, ..default() },
        UiTransform { rotation: Rot2::radians(d.y.atan2(d.x)), ..UiTransform::IDENTITY },
        BackgroundColor(PENCIL),
        GlobalZIndex(5),
        Pickable::IGNORE,
        ChildOf(sheet),
    ));
}

/// Whether the pointer at `at` touches the stroke anywhere along its line, not only at its points.
fn near(stroke: &[Vec2], at: Vec2) -> bool {
    match stroke {
        [] => false,
        [p] => p.distance(at) <= RUB,
        _ => stroke.windows(2).any(|w| {
            let (a, b) = (w[0], w[1]);
            let ab = b - a;
            let t = if ab.length_squared() > 0.0 { ((at - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
            (a + ab * t).distance(at) <= RUB
        }),
    }
}

fn lay_stroke(commands: &mut Commands, sheet: Entity, stroke: &[Vec2]) {
    match stroke {
        [] => {}
        [p] => piece(commands, sheet, *p, *p),
        _ => {
            for w in stroke.windows(2) {
                piece(commands, sheet, w[0], w[1]);
            }
        }
    }
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
            lay_stroke(&mut commands, sheet, stroke);
        }
    }
}

fn tray_button(label: String, fonts: &Fonts) -> impl Bundle {
    (
        Button,
        Node { width: px(240), padding: UiRect::axes(px(14), px(8)), border: UiRect::all(px(3)), justify_content: JustifyContent::Center, ..default() },
        BackgroundColor(theme::PEAT),
        BorderColor::all(EDGE),
        children![(theme::text(label, &fonts.title, 24.0, theme::BONE), TextLayout::justify(Justify::Center))],
    )
}

fn wipe(commands: &mut Commands, ink: &mut MapInk, dots: &Query<Entity, With<InkDot>>) {
    ink.strokes.clear();
    ink.drawing = false;
    for e in dots {
        commands.entity(e).despawn();
    }
}

/// The tray lies at the left of the map, beside the parchment, once per map screen.
fn tray(mut commands: Commands, trays: Query<(), With<InkTray>>, stage: Single<Entity, With<Stage>>, fonts: Res<Fonts>, l10n: Res<L10n>) {
    if !trays.is_empty() {
        return;
    }
    commands
        .spawn((
            InkTray,
            Node { position_type: PositionType::Absolute, left: px(28), top: px(crate::between::BAR_H + 28.0), flex_direction: FlexDirection::Column, row_gap: px(8), ..default() },
            GlobalZIndex(40),
            DespawnOnExit(AppState::Map),
            ChildOf(*stage),
        ))
        .with_children(|p| {
            p.spawn(theme::text(l10n.t("map-ink-title"), &fonts.body, 22.0, theme::BONE));
            p.spawn((tray_button(l10n.t("map-ink-pencil"), &fonts), ToolButton(Tool::Pencil), observe(|_: On<Activate>, mut ink: ResMut<MapInk>| ink.tool = Tool::Pencil)));
            p.spawn((tray_button(l10n.t("map-ink-eraser"), &fonts), ToolButton(Tool::Eraser), observe(|_: On<Activate>, mut ink: ResMut<MapInk>| ink.tool = Tool::Eraser)));
            p.spawn((
                tray_button(l10n.t("map-ink-clear"), &fonts),
                observe(|_: On<Activate>, mut commands: Commands, mut ink: ResMut<MapInk>, dots: Query<Entity, With<InkDot>>| wipe(&mut commands, &mut ink, &dots)),
            ));
        });
}

/// The tool in hand shows lit.
fn light_tray(ink: Res<MapInk>, mut buttons: Query<(&ToolButton, &mut BorderColor, &mut BackgroundColor)>) {
    for (b, mut border, mut bg) in &mut buttons {
        let on = b.0 == ink.tool;
        *border = BorderColor::all(if on { theme::CANDLE } else { EDGE });
        bg.0 = if on { Color::srgb(0.30, 0.22, 0.14) } else { theme::PEAT };
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
    mut moves: MessageReader<bevy::window::WindowEvent>,
    mut cursor: Local<Option<Vec2>>,
    sheet: Query<Entity, With<MapSheet>>,
    dots: Query<Entity, With<InkDot>>,
    mut ink: ResMut<MapInk>,
) {
    // where the pointer is, as the window's own events tell it (as the party's pointers are read)
    for ev in moves.read() {
        if let bevy::window::WindowEvent::CursorMoved(m) = ev {
            *cursor = Some(m.position);
        }
    }
    let Ok(sheet) = sheet.single() else { return };
    if keys.just_pressed(KeyCode::Delete) && !ink.strokes.is_empty() {
        wipe(&mut commands, &mut ink, &dots);
        return;
    }
    if !mouse.pressed(MouseButton::Right) {
        ink.drawing = false;
        return;
    }
    let Some(pos) = window.cursor_position().or_else(crate::touch::finger).or(*cursor) else { return };
    let stage = crate::stage_point(&window, scale.0, pos);
    // below the top bar only, where the sheet shows
    if stage.y < crate::between::BAR_H {
        return;
    }
    let at = stage - scroll.to_stage(Vec2::ZERO);
    if at.y < 0.0 || at.y > SHEET_H {
        return;
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if shift || ink.tool == Tool::Eraser {
        let before = ink.strokes.len();
        ink.strokes.retain(|s| !near(s, at));
        if ink.strokes.len() != before {
            for e in &dots {
                commands.entity(e).despawn();
            }
            for stroke in &ink.strokes {
                lay_stroke(&mut commands, sheet, stroke);
            }
        }
        ink.drawing = false;
        return;
    }
    if !ink.drawing || mouse.just_pressed(MouseButton::Right) {
        ink.drawing = true;
        ink.strokes.push(vec![at]);
        piece(&mut commands, sheet, at, at);
        return;
    }
    let Some(stroke) = ink.strokes.last_mut() else { return };
    let last = *stroke.last().unwrap_or(&at);
    if last.distance(at) < STEP {
        return;
    }
    // the piece from the last point reaches the pointer whole, however quick the stroke
    stroke.push(at);
    piece(&mut commands, sheet, last, at);
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_stroke_near_the_pointer_is_rubbed_out_and_the_rest_stay() {
        use bevy::math::Vec2;
        let mut strokes = vec![vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0)], vec![Vec2::new(500.0, 500.0)]];
        let at = Vec2::new(12.0, 5.0);
        strokes.retain(|s| !super::near(s, at));
        assert_eq!(strokes.len(), 1);
    }

    #[test]
    fn the_middle_of_a_long_piece_is_rubbed_out_too() {
        use bevy::math::Vec2;
        let stroke = vec![Vec2::new(0.0, 0.0), Vec2::new(400.0, 0.0)];
        assert!(super::near(&stroke, Vec2::new(200.0, 10.0)));
        assert!(!super::near(&stroke, Vec2::new(200.0, 60.0)));
    }
}
