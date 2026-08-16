//! Drawing and input abstraction.
//!
//! This crate is a dumb drawing surface: it knows about quads, lines, text, and
//! a camera, and nothing about units, buildings, or gold. It deliberately does
//! not depend on `sim`, so translating game state into primitives has to happen
//! in `app`. That keeps the backend swappable and lets the application's draw
//! logic be tested headlessly against [`RecordingRenderer`].
//!
//! Floating point is fine here. This is presentation, not simulation.
//!
//! See `docs/superpowers/specs/2026-08-04-rts-poc-design.md`.

mod camera;
mod input;
mod recording;

pub use camera::{Camera, Vec2};
pub use input::{InputSource, InputState, Key, MouseButton};
pub use recording::{DrawCall, RecordingRenderer};

/// An RGBA colour with components in `0.0..=1.0`.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);
    pub const BLACK: Color = Color::rgb(0.0, 0.0, 0.0);

    pub const fn rgb(r: f32, g: f32, b: f32) -> Color {
        Color { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color { r, g, b, a }
    }
}

/// A drawing surface.
///
/// Positions and sizes are in world units, one unit per tile; the camera passed
/// to [`Renderer::begin_frame`] decides where that lands on screen. Text is the
/// exception: its `px` size is in screen pixels, so debug overlays stay legible
/// at any zoom.
pub trait Renderer {
    fn begin_frame(&mut self, cam: &Camera);
    fn quad(&mut self, pos: Vec2, size: Vec2, color: Color);
    fn line(&mut self, a: Vec2, b: Vec2, color: Color);
    fn text(&mut self, s: &str, pos: Vec2, px: f32, color: Color);
    fn end_frame(&mut self);
}
