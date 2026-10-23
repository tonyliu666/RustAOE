//! A two-dimensional vector in fixed point.
//!
//! World positions live here rather than in `f32`, so that two machines
//! stepping the same commands land on bit-identical coordinates. One unit is
//! one tile.

use crate::Fx;
use serde::{Deserialize, Serialize};

/// A position or offset in world space, in tiles.
#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct FxVec2 {
    pub x: Fx,
    pub y: Fx,
}

impl FxVec2 {
    pub const ZERO: FxVec2 = FxVec2::new(Fx::ZERO, Fx::ZERO);

    pub const fn new(x: Fx, y: Fx) -> FxVec2 {
        FxVec2 { x, y }
    }

    /// The centre of a tile, which is where entities stand.
    ///
    /// Centres rather than corners so that a unit sitting on a tile is
    /// symmetric about it, and so that the tile a position falls in is a plain
    /// floor of the coordinate with no half-open-interval edge cases.
    pub const fn tile_center(pos: crate::TilePos) -> FxVec2 {
        FxVec2::new(
            Fx::from_frac(pos.x * 2 + 1, 2),
            Fx::from_frac(pos.y * 2 + 1, 2),
        )
    }

    /// The tile containing this position.
    pub const fn tile(self) -> crate::TilePos {
        crate::TilePos::new(self.x.to_int(), self.y.to_int())
    }

    /// Squared length in `i64` raw units.
    ///
    /// Squared rather than actual length, and widened, because comparing
    /// distances is the only thing the simulation ever needs and squaring
    /// avoids both `sqrt` and the saturation that squaring in `Fx` would hit.
    pub const fn length_squared(self) -> i64 {
        let x = self.x.to_bits() as i64;
        let y = self.y.to_bits() as i64;
        x * x + y * y
    }

    /// Length, saturating like all `Fx` arithmetic.
    pub fn length(self) -> Fx {
        Fx::from_bits((self.length_squared() as u64).isqrt() as i32)
    }

    /// Rescales to `len`, or returns zero if this vector has no direction.
    ///
    /// Returning zero for a zero input rather than panicking: two units on
    /// exactly the same tile is a normal, reachable state, and the separation
    /// force between them simply has no direction to push along.
    pub fn with_length(self, len: Fx) -> FxVec2 {
        let current = self.length();
        if current == Fx::ZERO {
            return FxVec2::ZERO;
        }
        let scale = len.div(current);
        FxVec2::new(self.x.mul(scale), self.y.mul(scale))
    }

    /// Shortens to at most `len`, leaving shorter vectors alone.
    pub fn clamp_length(self, len: Fx) -> FxVec2 {
        let limit = len.to_bits() as i64;
        if self.length_squared() <= limit * limit {
            self
        } else {
            self.with_length(len)
        }
    }
}

impl core::ops::Add for FxVec2 {
    type Output = FxVec2;

    fn add(self, other: FxVec2) -> FxVec2 {
        FxVec2::new(self.x + other.x, self.y + other.y)
    }
}

impl core::ops::Sub for FxVec2 {
    type Output = FxVec2;

    fn sub(self, other: FxVec2) -> FxVec2 {
        FxVec2::new(self.x - other.x, self.y - other.y)
    }
}

impl core::ops::Mul<Fx> for FxVec2 {
    type Output = FxVec2;

    fn mul(self, scale: Fx) -> FxVec2 {
        FxVec2::new(self.x.mul(scale), self.y.mul(scale))
    }
}
