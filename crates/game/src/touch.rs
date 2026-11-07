//! Fingers on a touch screen. Picking already takes a touch for a press of the pointer, so every
//! button, card and node works by finger as it does by mouse. What reads the mouse itself (the
//! book's leaves, the map's pencil, a card turned over to read its upgrade) is fed here:
//! - one finger down is the left button, held where it touches;
//! - a finger held still for a moment is the right button instead (the pencil, a card turned);
//! - a finger drawn over the map or an open pile rolls it, as the wheel does;
//! - the system's Back is Escape.
//!
//! On a phone this is always on; on a desktop with `TROPA_TOUCH=1`, to try it.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel};
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::input::ButtonState;
use bevy::prelude::*;
use std::sync::Mutex;

/// How long a still finger is held before it counts as the right button.
const LONG_PRESS: f32 = 0.45;
/// How far (logical pixels) a finger may wander and still be held still.
const STILL: f32 = 14.0;

/// Where the finger is, in window logical pixels: for code that asks the window for its cursor,
/// which a touch screen does not have.
static FINGER: Mutex<Option<Vec2>> = Mutex::new(None);

pub fn finger() -> Option<Vec2> {
    FINGER.lock().ok().and_then(|f| *f)
}

pub fn enabled() -> bool {
    crate::platform::TOUCH || std::env::var("TROPA_TOUCH").is_ok_and(|v| v == "1")
}

pub fn plugin(app: &mut App) {
    if enabled() {
        app.init_resource::<Hold>().add_systems(PreUpdate, fingers.before(bevy::input::InputSystems));
    }
}

/// The finger that stands for the mouse, and what it has been taken for.
#[derive(Resource, Default)]
struct Hold {
    id: Option<u64>,
    start: Vec2,
    at: Vec2,
    held: f32,
    /// Taken for the right button: held still long enough.
    right: bool,
    /// Moved too far to become the right button.
    moved: bool,
}

#[allow(clippy::too_many_arguments)]
fn fingers(
    mut touches: MessageReader<TouchInput>,
    held_keys: Res<ButtonInput<KeyCode>>,
    mut buttons: MessageWriter<MouseButtonInput>,
    mut wheel: MessageWriter<MouseWheel>,
    mut keys: MessageWriter<KeyboardInput>,
    mut hold: ResMut<Hold>,
    time: Res<Time>,
    window: Single<Entity, With<bevy::window::PrimaryWindow>>,
    state: Res<State<crate::AppState>>,
    piles: Query<(), With<crate::battle::piles::PileView>>,
) {
    let window = *window;
    let press = |buttons: &mut MessageWriter<MouseButtonInput>, button, down: bool| {
        buttons.write(MouseButtonInput { button, state: if down { ButtonState::Pressed } else { ButtonState::Released }, window });
    };
    // the map and an open pile roll under a drawn finger
    let rolls = (*state.get() == crate::AppState::Map || !piles.is_empty()) && !hold.right;
    for t in touches.read() {
        match t.phase {
            TouchPhase::Started if hold.id.is_none() => {
                *hold = Hold { id: Some(t.id), start: t.position, at: t.position, ..default() };
                press(&mut buttons, MouseButton::Left, true);
            }
            TouchPhase::Moved if hold.id == Some(t.id) => {
                let step = t.position - hold.at;
                hold.at = t.position;
                if hold.start.distance(t.position) > STILL {
                    hold.moved = true;
                }
                if rolls && hold.moved && step.y != 0.0 {
                    wheel.write(MouseWheel { unit: MouseScrollUnit::Pixel, x: 0.0, y: step.y, window, phase: TouchPhase::Moved });
                }
            }
            TouchPhase::Ended | TouchPhase::Canceled if hold.id == Some(t.id) => {
                press(&mut buttons, if hold.right { MouseButton::Right } else { MouseButton::Left }, false);
                *hold = Hold::default();
            }
            _ => {}
        }
    }
    if hold.id.is_some() && !hold.right && !hold.moved {
        hold.held += time.delta_secs();
        if hold.held >= LONG_PRESS {
            // the left press is let go, so whatever it began is not finished as a click
            hold.right = true;
            press(&mut buttons, MouseButton::Left, false);
            press(&mut buttons, MouseButton::Right, true);
        }
    }
    if let Ok(mut f) = FINGER.lock() {
        *f = hold.id.map(|_| hold.at);
    }
    // Back on the phone is Escape: it closes what is open, or pauses
    // (seen a frame late: the keys are read after this runs)
    if held_keys.just_pressed(KeyCode::BrowserBack) {
        for state in [ButtonState::Pressed, ButtonState::Released] {
            keys.write(KeyboardInput { key_code: KeyCode::Escape, logical_key: Key::Escape, state, text: None, repeat: false, window });
        }
    }
}
