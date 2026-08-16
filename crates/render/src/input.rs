//! Framework-neutral input snapshot.
//!
//! The application maps this into commands. Keeping it a plain struct with no
//! backend types in it means input can be synthesised in a test without a
//! window, which is the only way the command-mapping logic is testable.

use crate::Vec2;

/// The mouse buttons the game uses.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    const ALL: [MouseButton; 3] = [MouseButton::Left, MouseButton::Right, MouseButton::Middle];

    const fn index(self) -> usize {
        match self {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
        }
    }
}

/// The keys the game uses.
///
/// A closed enum rather than a passthrough of the backend's key type: it keeps
/// backends interchangeable, and an unmapped key is a compile error in `app`
/// rather than a silently dead binding.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Key {
    W,
    A,
    S,
    D,
    Q,
    E,
    Tab,
    Escape,
    Space,
    Digit1,
    Digit2,
}

/// One frame of input.
///
/// Buttons and keys are reported at two granularities. `down` is the level: true
/// for as long as it is held, which is what camera panning wants. `pressed` and
/// `released` are the edges of that level, which is what selection and hotkeys
/// want. Collapsing them would make a click drag a selection box open forever.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct InputState {
    /// Cursor position in screen pixels.
    pub cursor: Vec2,
    /// Accumulated scroll wheel movement this frame; positive is zoom in.
    pub scroll: f32,
    mouse_down: [bool; 3],
    mouse_pressed: [bool; 3],
    mouse_released: [bool; 3],
    keys_down: Vec<Key>,
    keys_pressed: Vec<Key>,
}

impl InputState {
    pub fn is_down(&self, button: MouseButton) -> bool {
        self.mouse_down[button.index()]
    }

    pub fn is_pressed(&self, button: MouseButton) -> bool {
        self.mouse_pressed[button.index()]
    }

    pub fn is_released(&self, button: MouseButton) -> bool {
        self.mouse_released[button.index()]
    }

    pub fn key_down(&self, key: Key) -> bool {
        self.keys_down.contains(&key)
    }

    pub fn key_pressed(&self, key: Key) -> bool {
        self.keys_pressed.contains(&key)
    }

    /// Records a button going down this frame.
    pub fn press(&mut self, button: MouseButton) {
        self.mouse_down[button.index()] = true;
        self.mouse_pressed[button.index()] = true;
    }

    /// Records a button coming up this frame.
    pub fn release(&mut self, button: MouseButton) {
        self.mouse_down[button.index()] = false;
        self.mouse_released[button.index()] = true;
    }

    /// Records a key going down this frame.
    pub fn press_key(&mut self, key: Key) {
        self.hold_key(key);
        if !self.keys_pressed.contains(&key) {
            self.keys_pressed.push(key);
        }
    }

    /// Records a key held from an earlier frame.
    pub fn hold_key(&mut self, key: Key) {
        if !self.keys_down.contains(&key) {
            self.keys_down.push(key);
        }
    }

    /// Drops the per-frame edges and scroll, keeping what is still held.
    ///
    /// A backend polls by reusing one `InputState` and calling this first, so
    /// that levels persist across frames while edges do not.
    pub fn begin_frame(&mut self) {
        self.scroll = 0.0;
        self.mouse_pressed = [false; 3];
        self.mouse_released = [false; 3];
        self.keys_pressed.clear();
    }

    /// Iterates the mouse buttons currently held.
    pub fn buttons_down(&self) -> impl Iterator<Item = MouseButton> + '_ {
        MouseButton::ALL.into_iter().filter(|b| self.is_down(*b))
    }
}

/// A source of input frames.
pub trait InputSource {
    fn poll(&mut self) -> InputState;
}
