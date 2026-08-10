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

    /// Largest representable value, roughly 32767.99998.
    pub const MAX: Fx = Fx(i32::MAX);

    /// Smallest representable value, roughly -32768.
    ///
    /// Note this is `-i32::MAX` rather than `i32::MIN`, so that the range is
    /// symmetric and `-MIN` is representable. An asymmetric range would make
    /// negation a special case that silently saturates.
    pub const MIN: Fx = Fx(-i32::MAX);

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
        Fx::from_wide((self.0 as i64 * other.0 as i64) >> SHIFT)
    }

    /// Narrows a widened intermediate back into range, saturating at the
    /// endpoints.
    ///
    /// Saturating rather than wrapping or panicking is a determinism
    /// requirement: `+` panics on overflow in debug builds and wraps in
    /// release builds, so the same inputs would produce different state
    /// depending on the build profile.
    const fn from_wide(v: i64) -> Fx {
        if v > Fx::MAX.0 as i64 {
            Fx::MAX
        } else if v < Fx::MIN.0 as i64 {
            Fx::MIN
        } else {
            Fx(v as i32)
        }
    }

    /// Divides one fixed-point value by another.
    ///
    /// Panics if `other` is zero. A silent wrong answer here would corrupt
    /// simulation state on one machine only, which is exactly the class of
    /// bug that is hardest to find later.
    pub const fn div(self, other: Fx) -> Fx {
        let scaled = (self.0 as i64) << SHIFT;
        Fx::from_wide(scaled / other.0 as i64)
    }

    /// Returns the whole part, rounding towards negative infinity.
    ///
    /// Flooring rather than truncating towards zero means the result is
    /// always the index of the tile containing the value, including on the
    /// negative side of the origin.
    pub const fn to_int(self) -> i32 {
        self.0 >> SHIFT
    }

    /// Absolute value.
    ///
    /// Total because the range is symmetric: unlike `i32::abs`, this cannot
    /// overflow, since `MIN` is `-MAX` rather than `i32::MIN`.
    pub const fn abs(self) -> Fx {
        Fx(self.0.abs())
    }

    /// Square root, rounded down.
    ///
    /// Panics on a negative input, which is always a caller bug rather than a
    /// recoverable condition.
    ///
    /// Widening by another `SHIFT` before taking the integer root is what
    /// keeps the scale right: for a value `v = V / 2^16`, the fixed-point
    /// result is `sqrt(V << 16)`.
    pub const fn sqrt(self) -> Fx {
        assert!(self.0 >= 0, "sqrt of a negative Fx");
        Fx((((self.0 as u64) << SHIFT).isqrt()) as i32)
    }
}

impl core::ops::Add for Fx {
    type Output = Fx;

    fn add(self, other: Fx) -> Fx {
        Fx::from_wide(self.0 as i64 + other.0 as i64)
    }
}

impl core::ops::Sub for Fx {
    type Output = Fx;

    fn sub(self, other: Fx) -> Fx {
        Fx::from_wide(self.0 as i64 - other.0 as i64)
    }
}

impl core::ops::Neg for Fx {
    type Output = Fx;

    fn neg(self) -> Fx {
        Fx::from_wide(-(self.0 as i64))
    }
}
