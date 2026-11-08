//! A `Renderer` that logs instead of drawing.
//!
//! This is the payoff of putting drawing behind a trait: the application's
//! translation of game state into primitives is assertable in CI, with no
//! window and no GPU. The real backend is verified by eye.

use crate::{Camera, Color, Renderer, TextureId, Vec2};

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
    Sprite {
        texture: TextureId,
        pos: Vec2,
        size: Vec2,
    },
    TintedSprite {
        texture: TextureId,
        pos: Vec2,
        size: Vec2,
        tint: Color,
    },
    EndFrame,
}

/// A renderer that records every call it receives.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct RecordingRenderer {
    calls: Vec<DrawCall>,
    /// The size and pixels of each loaded texture, indexed by its ID.
    textures: Vec<(u32, u32, Vec<u8>)>,
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

    /// The sprites recorded so far, as `(texture, pos, size)`.
    pub fn sprites(&self) -> impl Iterator<Item = (TextureId, Vec2, Vec2)> + '_ {
        self.calls.iter().filter_map(|call| match call {
            DrawCall::Sprite { texture, pos, size } => Some((*texture, *pos, *size)),
            _ => None,
        })
    }

    /// The tinted sprites recorded so far, as `(texture, pos, size, tint)`.
    /// [`RecordingRenderer::sprites`] leaves them out.
    pub fn tinted_sprites(&self) -> impl Iterator<Item = (TextureId, Vec2, Vec2, Color)> + '_ {
        self.calls.iter().filter_map(|call| match call {
            DrawCall::TintedSprite {
                texture,
                pos,
                size,
                tint,
            } => Some((*texture, *pos, *size, *tint)),
            _ => None,
        })
    }

    /// The size of a texture this renderer loaded, or `None` for one it did
    /// not, which would be a drawing bug in the caller.
    pub fn texture_size(&self, texture: TextureId) -> Option<(u32, u32)> {
        self.textures
            .get(texture.0 as usize)
            .map(|(w, h, _)| (*w, *h))
    }

    /// The RGBA pixels a texture was loaded with, so a test can check what
    /// would have been drawn.
    pub fn texture_pixels(&self, texture: TextureId) -> Option<&[u8]> {
        self.textures
            .get(texture.0 as usize)
            .map(|(_, _, pixels)| pixels.as_slice())
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

    fn load_texture(&mut self, width: u32, height: u32, rgba: &[u8]) -> TextureId {
        debug_assert_eq!(rgba.len(), width as usize * height as usize * 4);
        self.textures.push((width, height, rgba.to_vec()));
        TextureId(self.textures.len() as u32 - 1)
    }

    fn sprite(&mut self, texture: TextureId, pos: Vec2, size: Vec2) {
        self.calls.push(DrawCall::Sprite { texture, pos, size });
    }

    fn sprite_tinted(&mut self, texture: TextureId, pos: Vec2, size: Vec2, tint: Color) {
        self.calls.push(DrawCall::TintedSprite {
            texture,
            pos,
            size,
            tint,
        });
    }
}
