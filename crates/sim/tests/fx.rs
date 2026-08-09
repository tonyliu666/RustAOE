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
