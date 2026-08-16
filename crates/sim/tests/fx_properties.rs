//! Property tests for fixed-point arithmetic.
//!
//! The hand-written unit tests cover the edges that are known to be
//! interesting. These cover the range in between, where a scaling mistake
//! would only show up on values nobody thought to write down.

use proptest::prelude::*;
use sim::Fx;

/// A range wide enough to be interesting but narrow enough that a product of
/// two values stays inside `Fx`, so a failure means a real scaling bug rather
/// than saturation. `2^23 * 2^23 >> 16` is `2^30`, comfortably in range.
const LIMIT: i32 = 1 << 23;

prop_compose! {
    fn any_fx()(bits in -LIMIT..LIMIT) -> Fx {
        Fx::from_bits(bits)
    }
}

prop_compose! {
    /// Divisors of magnitude at least one.
    ///
    /// Dividing by a fraction amplifies the rounding error left by `mul` in
    /// proportion to `1 / b`, so a round-trip through a very small `b` is
    /// legitimately inexact. Multiplying by one or more cannot lose more than
    /// a single unit in the last place.
    fn nonzero_fx()(bits in Fx::ONE.to_bits()..LIMIT, negative in any::<bool>()) -> Fx {
        Fx::from_bits(if negative { -bits } else { bits })
    }
}

proptest! {
    /// The property that matters for movement integration: dividing out a
    /// factor recovers the original value.
    #[test]
    fn multiplication_then_division_round_trips(a in any_fx(), b in nonzero_fx()) {
        let round_tripped = a.mul(b).div(b);
        let error = (round_tripped.to_bits() as i64 - a.to_bits() as i64).abs();
        prop_assert!(error <= 1, "{a:?} * {b:?} / {b:?} = {round_tripped:?}");
    }

    #[test]
    fn multiplication_is_commutative(a in any_fx(), b in any_fx()) {
        prop_assert_eq!(a.mul(b), b.mul(a));
    }

    #[test]
    fn multiplying_by_one_is_the_identity(a in any_fx()) {
        prop_assert_eq!(a.mul(Fx::ONE), a);
    }

    #[test]
    fn addition_is_reversible(a in any_fx(), b in any_fx()) {
        prop_assert_eq!(a + b - b, a);
    }

    #[test]
    fn negation_is_its_own_inverse(a in any_fx()) {
        prop_assert_eq!(-(-a), a);
    }

    #[test]
    fn abs_is_never_negative(a in any_fx()) {
        prop_assert!(a.abs() >= Fx::ZERO);
    }

    /// `sqrt` rounds down, so squaring the root brackets the input: the root
    /// squared never exceeds it, and the next representable root does.
    ///
    /// The bracket is checked in exact `i64` rather than through `Fx::mul`,
    /// because `mul` also rounds down and would hide the upper bound being off
    /// by one.
    #[test]
    fn sqrt_is_the_floor_of_the_true_root(bits in 0..i32::MAX) {
        let value = Fx::from_bits(bits);
        let root = value.sqrt().to_bits() as i64;
        let scaled = (bits as i64) << 16;
        prop_assert!(root * root <= scaled);
        prop_assert!((root + 1) * (root + 1) > scaled);
    }

    /// Ordering on the fixed-point wrapper must agree with ordering on the
    /// numbers it represents, because comparisons drive target selection and a
    /// wrong answer there is a gameplay bug, not a rounding one.
    #[test]
    fn ordering_follows_the_underlying_bits(a in any_fx(), b in any_fx()) {
        prop_assert_eq!(a < b, a.to_bits() < b.to_bits());
    }

    /// Saturation, not overflow: arithmetic that leaves the range clamps to an
    /// endpoint on every platform and in every build profile.
    #[test]
    fn arithmetic_stays_in_range(a in any::<i32>(), b in any::<i32>()) {
        let a = Fx::from_bits(a);
        let b = Fx::from_bits(b);
        for result in [a + b, a - b, a.mul(b)] {
            prop_assert!(result >= Fx::MIN && result <= Fx::MAX);
        }
    }
}
