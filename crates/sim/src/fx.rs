//! 16.16 fixed-point arithmetic.
//!
//! One tile equals `Fx::ONE`. Floating point is banned in this crate because
//! `f32` results differ across platforms and would desync a lockstep game.

/// A 16.16 signed fixed-point number.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Fx(i32);

/// Number of fractional bits in the representation.
const SHIFT: u32 = 16;

impl Fx {
    /// One whole unit, which is one tile in world space.
    pub const ONE: Fx = Fx::from_int(1);

    /// Zero.
    pub const ZERO: Fx = Fx(0);

    /// Converts a whole number into fixed point.
    pub const fn from_int(n: i32) -> Fx {
        Fx(n << SHIFT)
    }

    /// Builds the fraction `num / den`.
    ///
    /// Panics if `den` is zero.
    pub const fn from_frac(num: i32, den: i32) -> Fx {
        Fx((((num as i64) << SHIFT) / den as i64) as i32)
    }

    /// Multiplies two fixed-point values.
    ///
    /// The intermediate product is widened to `i64` because two 16.16 values
    /// multiplied together need 64 bits before the scale is shifted back out.
    pub const fn mul(self, other: Fx) -> Fx {
        Fx(((self.0 as i64 * other.0 as i64) >> SHIFT) as i32)
    }

    /// Divides one fixed-point value by another.
    ///
    /// Panics if `other` is zero. A silent wrong answer here would corrupt
    /// simulation state on one machine only, which is exactly the class of
    /// bug that is hardest to find later.
    pub const fn div(self, other: Fx) -> Fx {
        let scaled = (self.0 as i64) << SHIFT;
        Fx((scaled / other.0 as i64) as i32)
    }

    /// Returns the whole part, rounding towards negative infinity.
    ///
    /// Flooring rather than truncating towards zero means the result is
    /// always the index of the tile containing the value, including on the
    /// negative side of the origin.
    pub const fn to_int(self) -> i32 {
        self.0 >> SHIFT
    }
}
