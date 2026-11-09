//! Fingers on a touch screen. The first finger is made the mouse: the cursor goes where it
//! touches and stays where it last touched (so a tap on a relic, a card, a hero or a foe hovers it
//! and shows what it is, as the mouse does), and its presses are the mouse's buttons. Picking's own
//! touch pointers are turned off, so nothing is pressed twice. So:
//! - a tap is a left click where it lands;
//! - a finger held still for a moment is the right button instead (a card turned over to read its
//!   upgrade, the pencil's eraser);
//! - a finger moved is the left button held down from where it touched: a card dragged and aimed,
//!   the book's leaf pulled; over the map or an open pile it rolls them, as the wheel does;
//! - further fingers are extra hands for the book (`more_fingers`): one reader may pull a leaf
//!   at several points at once;
//! - the system's Back is Escape.
//!
//! On a phone this is always on; on a desktop with `TROPA_TOUCH=1`, to try it.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel};
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::window::{CursorMoved, WindowEvent};
use std::sync::Mutex;

/// How long a still finger is held before it counts as the right button.
const LONG_PRESS: f32 = 0.45;
/// How far (logical pixels) a finger may wander and still be held still.
const STILL: f32 = 14.0;

/// Where the first finger is or last was, in window logical pixels: for code that asks the window
/// for its cursor, which a touch screen does not have.
static FINGER: Mutex<Option<Vec2>> = Mutex::new(None);
/// Every other finger down, in window logical pixels.
static MORE: Mutex<Vec<Vec2>> = Mutex::new(Vec::new());

pub fn finger() -> Option<Vec2> {
    FINGER.lock().ok().and_then(|f| *f)
}

/// The fingers down besides the first, in window logical pixels.
pub fn more_fingers() -> Vec<Vec2> {
    MORE.lock().map(|m| m.clone()).unwrap_or_default()
}

pub fn enabled() -> bool {
    crate::platform::TOUCH || std::env::var("TROPA_TOUCH").is_ok_and(|v| v == "1")
}

pub fn plugin(app: &mut App) {
    if enabled() {
        app.insert_resource(bevy::picking::input::PointerInputSettings { is_touch_enabled: false, is_mouse_enabled: true })
            .init_resource::<Hold>()
            .add_systems(PreUpdate, fingers.before(bevy::input::InputSystems));
    }
}

/// The finger that stands for the mouse, and what it has been taken for.
#[derive(Resource, Default)]
struct Hold {
    id: Option<u64>,
    start: Vec2,
    at: Vec2,
    held: f32,
    /// The left button is down: the finger moved while touching.
    left: bool,
    /// Taken for the right button: held still long enough.
    right: bool,
    /// Moved too far to become the right button.
    moved: bool,
    /// The other fingers down, by id.
    more: Vec<(u64, Vec2)>,
}

/// What the mouse is told, both as the window's events (which picking reads) and as the input
/// messages (which `ButtonInput<MouseButton>` is built from).
struct Mouse<'a, 'e, 'b, 'm> {
    window: Entity,
    events: &'a mut MessageWriter<'e, WindowEvent>,
    buttons: &'a mut MessageWriter<'b, MouseButtonInput>,
    moved: &'a mut MessageWriter<'m, CursorMoved>,
}

impl Mouse<'_, '_, '_, '_> {
    fn go(&mut self, to: Vec2) {
        let m = CursorMoved { window: self.window, position: to, delta: None };
        self.events.write(WindowEvent::CursorMoved(m.clone()));
        self.moved.write(m);
    }

    fn press(&mut self, button: MouseButton, down: bool) {
        let b = MouseButtonInput { button, state: if down { ButtonState::Pressed } else { ButtonState::Released }, window: self.window };
        self.events.write(WindowEvent::MouseButtonInput(b));
        self.buttons.write(b);
    }
}

#[allow(clippy::too_many_arguments)]
fn fingers(
    mut touches: MessageReader<TouchInput>,
    held_keys: Res<ButtonInput<KeyCode>>,
    mut events: MessageWriter<WindowEvent>,
    mut buttons: MessageWriter<MouseButtonInput>,
    mut moved: MessageWriter<CursorMoved>,
    mut wheel: MessageWriter<MouseWheel>,
    mut keys: MessageWriter<KeyboardInput>,
    mut hold: ResMut<Hold>,
    time: Res<Time>,
    window: Single<Entity, With<bevy::window::PrimaryWindow>>,
    state: Res<State<crate::AppState>>,
    piles: Query<(), With<crate::battle::piles::PileView>>,
) {
    let window = *window;
    let mut mouse = Mouse { window, events: &mut events, buttons: &mut buttons, moved: &mut moved };
    // the map and an open pile roll under a drawn finger
    let rolls = (*state.get() == crate::AppState::Map || !piles.is_empty()) && !hold.right;
    for t in touches.read() {
        match t.phase {
            TouchPhase::Started if hold.id.is_none() => {
                let more = std::mem::take(&mut hold.more);
                *hold = Hold { id: Some(t.id), start: t.position, at: t.position, more, ..default() };
                // the cursor comes to the finger; what it is pressed for is known once it moves,
                // lets go or has been held still
                mouse.go(t.position);
            }
            TouchPhase::Started => hold.more.push((t.id, t.position)),
            TouchPhase::Moved if hold.id == Some(t.id) => {
                let step = t.position - hold.at;
                hold.at = t.position;
                if !hold.moved && hold.start.distance(t.position) > STILL {
                    hold.moved = true;
                    if !hold.right && !rolls {
                        // pressed where it touched, then drawn to where it is
                        hold.left = true;
                        mouse.press(MouseButton::Left, true);
                    }
                }
                mouse.go(t.position);
                if rolls && hold.moved && step.y != 0.0 {
                    wheel.write(MouseWheel { unit: MouseScrollUnit::Pixel, x: 0.0, y: step.y, window, phase: TouchPhase::Moved });
                }
            }
            TouchPhase::Moved => {
                if let Some(f) = hold.more.iter_mut().find(|(id, _)| *id == t.id) {
                    f.1 = t.position;
                }
            }
            TouchPhase::Ended | TouchPhase::Canceled if hold.id == Some(t.id) => {
                if hold.right {
                    mouse.press(MouseButton::Right, false);
                } else if hold.left {
                    mouse.press(MouseButton::Left, false);
                } else if t.phase == TouchPhase::Ended && !(rolls && hold.moved) {
                    // a tap: a click where it landed
                    mouse.press(MouseButton::Left, true);
                    mouse.press(MouseButton::Left, false);
                }
                let more = std::mem::take(&mut hold.more);
                *hold = Hold { more, ..default() };
            }
            TouchPhase::Ended | TouchPhase::Canceled => hold.more.retain(|(id, _)| *id != t.id),
        }
    }
    if hold.id.is_some() && !hold.right && !hold.moved && !hold.left {
        hold.held += time.delta_secs();
        if hold.held >= LONG_PRESS {
            hold.right = true;
            mouse.press(MouseButton::Right, true);
        }
    }
    if let Ok(mut f) = FINGER.lock() {
        // the cursor stays where the finger last was
        if hold.id.is_some() {
            *f = Some(hold.at);
        }
    }
    if let Ok(mut m) = MORE.lock() {
        *m = hold.more.iter().map(|(_, at)| *at).collect();
    }
    // Back on the phone is Escape: it closes what is open, or pauses
    // (seen a frame late: the keys are read after this runs)
    if held_keys.just_pressed(KeyCode::BrowserBack) {
        for state in [ButtonState::Pressed, ButtonState::Released] {
            keys.write(KeyboardInput { key_code: KeyCode::Escape, logical_key: Key::Escape, state, text: None, repeat: false, window });
        }
    }
}
