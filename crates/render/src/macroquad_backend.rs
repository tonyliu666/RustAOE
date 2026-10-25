//! macroquad implementations of [`Renderer`] and [`InputSource`].
//!
//! Nothing else in the crate knows this module exists: a Bevy backend later is
//! a sibling module behind its own feature, and `app` is untouched. Verified by
//! eye rather than by test; the assertable draw logic lives in `app` and is
//! tested against [`crate::RecordingRenderer`].

use macroquad::prelude as mq;

use crate::{Camera, Color, InputSource, InputState, Key, MouseButton, Renderer, TextureId, Vec2};

/// Draws through macroquad's immediate-mode API.
#[derive(Default)]
pub struct MacroquadRenderer {
    /// The camera of the frame in progress, kept so that text can be drawn in
    /// screen space and the world camera restored afterwards.
    camera: Option<Camera>,
    /// Uploaded textures, indexed by [`TextureId`].
    textures: Vec<mq::Texture2D>,
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
        mq::set_camera(&to_mq_camera(camera));
    }
}

/// The macroquad camera for one of ours.
fn to_mq_camera(camera: &Camera) -> mq::Camera2D {
    // Both zoom components positive. macroquad 0.4 already flips y when
    // drawing to the window (`invert_y` in `Camera2D::matrix`), so world y
    // increases downwards, matching tile coordinates in `sim` and the mouse.
    // Negating y here as well, as an earlier version did, drew the world
    // upside down while picking stayed the right way up.
    mq::Camera2D {
        target: mq::vec2(camera.center.x, camera.center.y),
        zoom: mq::vec2(
            2.0 * camera.zoom / camera.viewport.x,
            2.0 * camera.zoom / camera.viewport.y,
        ),
        ..Default::default()
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

    fn load_texture(&mut self, width: u32, height: u32, rgba: &[u8]) -> TextureId {
        // macroquad takes 16-bit sizes. A larger image is a caller bug (the
        // asset readers cap sprites well below this), so say so plainly
        // rather than truncating the size and failing deep in the backend.
        assert!(
            width <= u32::from(u16::MAX) && height <= u32::from(u16::MAX),
            "texture {width}x{height} is too large to upload"
        );
        assert_eq!(
            rgba.len(),
            width as usize * height as usize * 4,
            "texture data does not match its size"
        );
        let texture = mq::Texture2D::from_rgba8(width as u16, height as u16, rgba);
        // The original's sprites are pixel art: scale them without blurring.
        texture.set_filter(mq::FilterMode::Nearest);
        self.textures.push(texture);
        TextureId(self.textures.len() as u32 - 1)
    }

    fn sprite(&mut self, texture: TextureId, pos: Vec2, size: Vec2) {
        // An ID this renderer never issued is skipped rather than panicking
        // mid-frame; the recording renderer is where that bug is caught.
        let Some(texture) = self.textures.get(texture.0 as usize) else {
            return;
        };
        mq::draw_texture_ex(
            texture,
            pos.x,
            pos.y,
            mq::WHITE,
            mq::DrawTextureParams {
                dest_size: Some(mq::vec2(size.x, size.y)),
                ..Default::default()
            },
        );
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

const KEYS: [(Key, mq::KeyCode); 19] = [
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
    (Key::Digit3, mq::KeyCode::Key3),
    (Key::Digit4, mq::KeyCode::Key4),
    (Key::Digit5, mq::KeyCode::Key5),
    (Key::Digit6, mq::KeyCode::Key6),
    (Key::Digit7, mq::KeyCode::Key7),
    (Key::Digit8, mq::KeyCode::Key8),
    (Key::Digit9, mq::KeyCode::Key9),
    (Key::F10, mq::KeyCode::F10),
];

impl InputSource for MacroquadInput {
    fn poll(&mut self) -> InputState {
        self.state.begin_frame();

        let (x, y) = mq::mouse_position();
        self.state.cursor = Vec2::new(x, y);
        // Sticky: once the mouse has moved, or sits anywhere but the (0, 0)
        // macroquad reports before its first mouse event, the position is real.
        if (x, y) != (0.0, 0.0) || mq::mouse_delta_position() != mq::Vec2::ZERO {
            self.state.cursor_seen = true;
        }
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

/// Drawing and picking must agree on where a world point is on screen.
///
/// Picking uses [`Camera::world_to_screen`], which puts world y downwards.
/// These tests push world points through the matrix macroquad actually draws
/// with, and check they land on the same pixels. They need no window: the
/// matrix is plain arithmetic.
#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::camera::Camera as _;

    /// Where macroquad draws a world point, in window pixels with y down.
    ///
    /// The matrix produces normalised device coordinates, in which +1 y is
    /// the top of the window.
    fn drawn_at(camera: &Camera, world: Vec2) -> Vec2 {
        let ndc = to_mq_camera(camera)
            .matrix()
            .project_point3(mq::vec3(world.x, world.y, 0.0));
        Vec2::new(
            (ndc.x + 1.0) * 0.5 * camera.viewport.x,
            (1.0 - ndc.y) * 0.5 * camera.viewport.y,
        )
    }

    #[test]
    fn drawing_puts_world_points_where_picking_expects_them() {
        let camera = Camera::new(Vec2::new(18.0, 18.0), 16.0, Vec2::new(800.0, 600.0));

        for world in [
            Vec2::new(18.0, 18.0),
            Vec2::new(16.0, 16.0),
            Vec2::new(18.0, 21.0),
            Vec2::new(40.0, 2.0),
        ] {
            let drawn = drawn_at(&camera, world);
            let picked = camera.world_to_screen(world);
            assert!(
                (drawn.x - picked.x).abs() < 1e-3 && (drawn.y - picked.y).abs() < 1e-3,
                "world {world:?} is drawn at {drawn:?} but picked at {picked:?}"
            );
        }
    }

    #[test]
    fn a_larger_world_y_is_drawn_lower_on_screen() {
        let camera = Camera::new(Vec2::new(18.0, 18.0), 16.0, Vec2::new(800.0, 600.0));

        let above = drawn_at(&camera, Vec2::new(18.0, 16.0));
        let below = drawn_at(&camera, Vec2::new(18.0, 21.0));

        assert!(
            below.y > above.y,
            "y=21 drawn at {below:?}, y=16 at {above:?}"
        );
    }
}
