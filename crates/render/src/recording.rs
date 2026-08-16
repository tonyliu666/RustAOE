//! A `Renderer` that logs instead of drawing.
//!
//! This is the payoff of putting drawing behind a trait: the application's
//! translation of game state into primitives is assertable in CI, with no
//! window and no GPU. The real backend is verified by eye.

use crate::{Camera, Color, Renderer, Vec2};

/// One recorded call, in the order it was made.
#[derive(Clone, PartialEq, Debug)]
pub enum DrawCall {
    BeginFrame(Camera),
    Quad {
        pos: Vec2,
        size: Vec2,
        color: Color,
    },
    Line {
        a: Vec2,
        b: Vec2,
        color: Color,
    },
    Text {
        text: String,
        pos: Vec2,
        px: f32,
        color: Color,
    },
    EndFrame,
}

/// A renderer that records every call it receives.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct RecordingRenderer {
    calls: Vec<DrawCall>,
}

impl RecordingRenderer {
    pub fn new() -> RecordingRenderer {
        RecordingRenderer::default()
    }

    /// Every call recorded so far.
    pub fn calls(&self) -> &[DrawCall] {
        &self.calls
    }

    /// The quads recorded so far, as `(pos, size, color)`.
    pub fn quads(&self) -> impl Iterator<Item = (Vec2, Vec2, Color)> + '_ {
        self.calls.iter().filter_map(|call| match call {
            DrawCall::Quad { pos, size, color } => Some((*pos, *size, *color)),
            _ => None,
        })
    }

    /// The text recorded so far, as `(text, pos)`.
    pub fn texts(&self) -> impl Iterator<Item = (&str, Vec2)> + '_ {
        self.calls.iter().filter_map(|call| match call {
            DrawCall::Text { text, pos, .. } => Some((text.as_str(), *pos)),
            _ => None,
        })
    }

    /// Number of completed frames.
    ///
    /// Counts `end_frame` rather than `begin_frame`, so a frame in progress is
    /// not counted as finished.
    pub fn frame_count(&self) -> usize {
        self.calls
            .iter()
            .filter(|call| **call == DrawCall::EndFrame)
            .count()
    }

    /// Drops the log, so a test can assert on one frame in isolation.
    pub fn clear(&mut self) {
        self.calls.clear();
    }
}

impl Renderer for RecordingRenderer {
    fn begin_frame(&mut self, cam: &Camera) {
        self.calls.push(DrawCall::BeginFrame(*cam));
    }

    fn quad(&mut self, pos: Vec2, size: Vec2, color: Color) {
        self.calls.push(DrawCall::Quad { pos, size, color });
    }

    fn line(&mut self, a: Vec2, b: Vec2, color: Color) {
        self.calls.push(DrawCall::Line { a, b, color });
    }

    fn text(&mut self, s: &str, pos: Vec2, px: f32, color: Color) {
        self.calls.push(DrawCall::Text {
            text: s.to_string(),
            pos,
            px,
            color,
        });
    }

    fn end_frame(&mut self) {
        self.calls.push(DrawCall::EndFrame);
    }
}
