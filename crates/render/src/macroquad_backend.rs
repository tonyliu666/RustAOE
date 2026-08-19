//! macroquad implementations of [`Renderer`] and [`InputSource`].
//!
//! Nothing else in the crate knows this module exists: a Bevy backend later is
//! a sibling module behind its own feature, and `app` is untouched. Verified by
//! eye rather than by test; the assertable draw logic lives in `app` and is
//! tested against [`crate::RecordingRenderer`].

use macroquad::prelude as mq;

use crate::{Camera, Color, InputSource, InputState, Key, MouseButton, Renderer, Vec2};

/// Draws through macroquad's immediate-mode API.
#[derive(Default)]
pub struct MacroquadRenderer {
    /// The camera of the frame in progress, kept so that text can be drawn in
    /// screen space and the world camera restored afterwards.
    camera: Option<Camera>,
}

impl MacroquadRenderer {
    pub fn new() -> MacroquadRenderer {
        MacroquadRenderer::default()
    }

    /// The current window size, for keeping the camera viewport in step.
    pub fn viewport() -> Vec2 {
        Vec2::new(mq::screen_width(), mq::screen_height())
    }

    fn apply_camera(camera: &Camera) {
        // The y component of the zoom is negated so that world y increases
        // downwards, matching tile coordinates in `sim`.
        mq::set_camera(&mq::Camera2D {
            target: mq::vec2(camera.center.x, camera.center.y),
            zoom: mq::vec2(
                2.0 * camera.zoom / camera.viewport.x,
                -2.0 * camera.zoom / camera.viewport.y,
            ),
            ..Default::default()
        });
    }
}

fn to_mq_color(color: Color) -> mq::Color {
    mq::Color::new(color.r, color.g, color.b, color.a)
}

impl Renderer for MacroquadRenderer {
    fn begin_frame(&mut self, cam: &Camera) {
        self.camera = Some(*cam);
        mq::clear_background(mq::BLACK);
        MacroquadRenderer::apply_camera(cam);
    }

    fn quad(&mut self, pos: Vec2, size: Vec2, color: Color) {
        mq::draw_rectangle(pos.x, pos.y, size.x, size.y, to_mq_color(color));
    }

    fn line(&mut self, a: Vec2, b: Vec2, color: Color) {
        // One screen pixel wide at any zoom, so debug lines stay hairlines
        // rather than growing into bars as the player zooms in.
        let thickness = self.camera.map_or(1.0, |cam| 1.0 / cam.zoom);
        mq::draw_line(a.x, a.y, b.x, b.y, thickness, to_mq_color(color));
    }

    fn text(&mut self, s: &str, pos: Vec2, px: f32, color: Color) {
        // `px` is in screen pixels, so text is drawn in screen space and the
        // world camera put back afterwards.
        let Some(camera) = self.camera else { return };
        let screen = camera.world_to_screen(pos);
        mq::set_default_camera();
        mq::draw_text(s, screen.x, screen.y, px, to_mq_color(color));
        MacroquadRenderer::apply_camera(&camera);
    }

    fn end_frame(&mut self) {
        mq::set_default_camera();
        self.camera = None;
    }
}

/// Reads macroquad's input state into an [`InputState`].
#[derive(Default)]
pub struct MacroquadInput {
    state: InputState,
}

impl MacroquadInput {
    pub fn new() -> MacroquadInput {
        MacroquadInput::default()
    }
}

const BUTTONS: [(MouseButton, mq::MouseButton); 3] = [
    (MouseButton::Left, mq::MouseButton::Left),
    (MouseButton::Right, mq::MouseButton::Right),
    (MouseButton::Middle, mq::MouseButton::Middle),
];

const KEYS: [(Key, mq::KeyCode); 11] = [
    (Key::W, mq::KeyCode::W),
    (Key::A, mq::KeyCode::A),
    (Key::S, mq::KeyCode::S),
    (Key::D, mq::KeyCode::D),
    (Key::Q, mq::KeyCode::Q),
    (Key::E, mq::KeyCode::E),
    (Key::Tab, mq::KeyCode::Tab),
    (Key::Escape, mq::KeyCode::Escape),
    (Key::Space, mq::KeyCode::Space),
    (Key::Digit1, mq::KeyCode::Key1),
    (Key::Digit2, mq::KeyCode::Key2),
];

impl InputSource for MacroquadInput {
    fn poll(&mut self) -> InputState {
        self.state.begin_frame();

        let (x, y) = mq::mouse_position();
        self.state.cursor = Vec2::new(x, y);
        // macroquad reports the wheel as (x, y); only vertical movement zooms,
        // and it is scaled to roughly one notch per event across platforms.
        let (_, wheel) = mq::mouse_wheel();
        self.state.scroll = wheel.signum() * (wheel.abs().min(1.0));

        for (button, mq_button) in BUTTONS {
            if mq::is_mouse_button_pressed(mq_button) {
                self.state.press(button);
            } else if mq::is_mouse_button_released(mq_button) {
                self.state.release(button);
            }
        }

        for (key, code) in KEYS {
            if mq::is_key_pressed(code) {
                self.state.press_key(key);
            } else if mq::is_key_down(code) {
                self.state.hold_key(key);
            } else {
                self.state.release_key(key);
            }
        }

        self.state.clone()
    }
}
