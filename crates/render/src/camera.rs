//! Camera and the two-dimensional vector it works in.

/// A point or extent in either world or screen space.
///
/// Which space a given value is in is carried by the name of the parameter, not
/// by the type. A newtype per space would be more honest, but the whole crate
/// is one screenful of geometry and the conversions all live in this file.
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2::new(0.0, 0.0);

    pub const fn new(x: f32, y: f32) -> Vec2 {
        Vec2 { x, y }
    }
}

impl core::ops::Add for Vec2 {
    type Output = Vec2;

    fn add(self, other: Vec2) -> Vec2 {
        Vec2::new(self.x + other.x, self.y + other.y)
    }
}

impl core::ops::Sub for Vec2 {
    type Output = Vec2;

    fn sub(self, other: Vec2) -> Vec2 {
        Vec2::new(self.x - other.x, self.y - other.y)
    }
}

impl core::ops::Mul<f32> for Vec2 {
    type Output = Vec2;

    fn mul(self, scale: f32) -> Vec2 {
        Vec2::new(self.x * scale, self.y * scale)
    }
}

/// An orthographic 2D camera.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Camera {
    /// World position drawn at the centre of the viewport.
    pub center: Vec2,
    /// Screen pixels per world unit, so one tile is `zoom` pixels across.
    pub zoom: f32,
    /// Viewport extent in screen pixels.
    pub viewport: Vec2,
}

impl Camera {
    pub const fn new(center: Vec2, zoom: f32, viewport: Vec2) -> Camera {
        Camera {
            center,
            zoom,
            viewport,
        }
    }

    /// Converts a world position into screen pixels.
    pub fn world_to_screen(&self, world: Vec2) -> Vec2 {
        (world - self.center) * self.zoom + self.viewport * 0.5
    }

    /// Converts a screen position into world units.
    ///
    /// This is the direction the application needs to turn a mouse click into a
    /// tile, so it has to be exact enough to round-trip.
    pub fn screen_to_world(&self, screen: Vec2) -> Vec2 {
        (screen - self.viewport * 0.5) * (1.0 / self.zoom) + self.center
    }
}
