use sim::Fx;

#[test]
fn whole_numbers_survive_a_round_trip() {
    assert_eq!(Fx::from_int(0).to_int(), 0);
    assert_eq!(Fx::from_int(1).to_int(), 1);
    assert_eq!(Fx::from_int(12345).to_int(), 12345);
    assert_eq!(Fx::from_int(-7).to_int(), -7);
}

#[test]
fn one_represents_a_whole_unit() {
    assert_eq!(Fx::ONE, Fx::from_int(1));
}

#[test]
fn equivalent_fractions_are_equal() {
    assert_eq!(Fx::from_frac(1, 2), Fx::from_frac(2, 4));
    assert_eq!(Fx::from_frac(4, 2), Fx::from_int(2));
    assert_eq!(Fx::from_frac(-1, 2), Fx::from_frac(1, -2));
}

#[test]
fn to_int_floors_rather_than_truncating_towards_zero() {
    assert_eq!(Fx::from_frac(1, 2).to_int(), 0);
    assert_eq!(Fx::from_frac(3, 2).to_int(), 1);
    assert_eq!(Fx::from_frac(-1, 2).to_int(), -1);
    assert_eq!(Fx::from_frac(-3, 2).to_int(), -2);
}

#[test]
fn multiplication_preserves_the_fixed_point_scale() {
    assert_eq!(Fx::from_int(3).mul(Fx::from_int(4)), Fx::from_int(12));
    assert_eq!(Fx::from_int(-3).mul(Fx::from_int(4)), Fx::from_int(-12));
    assert_eq!(Fx::from_int(-3).mul(Fx::from_int(-4)), Fx::from_int(12));
    assert_eq!(
        Fx::from_frac(1, 2).mul(Fx::from_frac(1, 2)),
        Fx::from_frac(1, 4)
    );
    assert_eq!(Fx::from_int(5).mul(Fx::ONE), Fx::from_int(5));
    assert_eq!(Fx::from_int(5).mul(Fx::ZERO), Fx::ZERO);
}

#[test]
fn division_preserves_the_fixed_point_scale() {
    assert_eq!(Fx::from_int(12).div(Fx::from_int(4)), Fx::from_int(3));
    assert_eq!(Fx::from_int(-12).div(Fx::from_int(4)), Fx::from_int(-3));
    assert_eq!(Fx::from_int(1).div(Fx::from_int(2)), Fx::from_frac(1, 2));
    assert_eq!(Fx::from_int(5).div(Fx::ONE), Fx::from_int(5));
    assert_eq!(
        Fx::from_frac(1, 4).div(Fx::from_frac(1, 2)),
        Fx::from_frac(1, 2)
    );
}

#[test]
#[should_panic]
fn division_by_zero_panics_rather_than_returning_a_wrong_answer() {
    let _ = Fx::ONE.div(Fx::ZERO);
}

#[test]
fn addition_and_subtraction_combine_whole_and_fractional_parts() {
    assert_eq!(Fx::from_int(3) + Fx::from_int(4), Fx::from_int(7));
    assert_eq!(Fx::from_frac(1, 2) + Fx::from_frac(1, 2), Fx::ONE);
    assert_eq!(Fx::from_int(3) - Fx::from_int(4), Fx::from_int(-1));
    assert_eq!(Fx::ONE - Fx::from_frac(1, 4), Fx::from_frac(3, 4));
}

#[test]
fn negation_flips_sign() {
    assert_eq!(-Fx::from_int(5), Fx::from_int(-5));
    assert_eq!(-Fx::ZERO, Fx::ZERO);
    assert_eq!(-(-Fx::from_frac(1, 3)), Fx::from_frac(1, 3));
}

#[test]
fn arithmetic_saturates_so_debug_and_release_builds_agree() {
    assert_eq!(Fx::MAX + Fx::ONE, Fx::MAX);
    assert_eq!(Fx::MIN - Fx::ONE, Fx::MIN);
    assert_eq!(-Fx::MIN, Fx::MAX);
    assert_eq!(Fx::MAX.mul(Fx::from_int(2)), Fx::MAX);
    assert_eq!(Fx::MIN.mul(Fx::from_int(2)), Fx::MIN);
    assert_eq!(Fx::MAX.div(Fx::from_frac(1, 2)), Fx::MAX);
}

#[test]
fn sqrt_of_perfect_squares_is_exact() {
    assert_eq!(Fx::ZERO.sqrt(), Fx::ZERO);
    assert_eq!(Fx::ONE.sqrt(), Fx::ONE);
    assert_eq!(Fx::from_int(4).sqrt(), Fx::from_int(2));
    assert_eq!(Fx::from_int(144).sqrt(), Fx::from_int(12));
    assert_eq!(Fx::from_frac(1, 4).sqrt(), Fx::from_frac(1, 2));
}

#[test]
fn sqrt_of_non_squares_is_within_one_unit_in_the_last_place() {
    // sqrt(2) = 1.41421356..., which is 92681.9 in 16.16.
    let two = Fx::from_int(2).sqrt();
    let expected = Fx::from_frac(92681, 65536);
    assert!(
        (two - expected).abs() <= Fx::from_frac(1, 65536),
        "sqrt(2) was {two:?}, expected within 1 ulp of {expected:?}"
    );

    // The defining property matters more than the digits: r*r must be close
    // to the input, and never overshoot by more than rounding allows.
    for n in 1..200 {
        let x = Fx::from_int(n);
        let r = x.sqrt();
        assert!(r.mul(r) <= x, "sqrt({n}) overshot: {r:?}^2 > {x:?}");
    }
}

#[test]
#[should_panic]
fn sqrt_of_a_negative_value_panics() {
    let _ = Fx::from_int(-1).sqrt();
}
