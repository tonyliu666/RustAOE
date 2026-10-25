//! Each conversion rule on its own, with values chosen in the test.

use assets::dat::Cost;
use datconv::{building_size, cost, damage, fraction, speed_per_tick, ticks, ticks_per_gold};

#[test]
fn a_whole_number_of_ticks_is_exact() {
    // Seconds times 20 ticks a second.
    for (seconds, expected) in [(25.0, 500), (2.0, 40), (21.0, 420)] {
        let converted = ticks(seconds, false);
        assert_eq!(converted.value, expected);
        assert!(!converted.was_rounded(), "{seconds} s");
    }
}

#[test]
fn a_time_between_ticks_rounds_to_the_nearest_and_says_so() {
    let converted = ticks(1.52, false);
    assert_eq!(converted.value, 30);
    assert!(converted.was_rounded());
    // Halves round away from zero: 0.025 s is half a tick.
    assert_eq!(ticks(0.025, false).value, 1);
}

#[test]
fn a_time_that_must_happen_is_at_least_one_tick() {
    assert_eq!(ticks(0.01, true).value, 1);
    assert_eq!(ticks(0.0, true).value, 1);
    assert_eq!(ticks(0.0, false).value, 0);
}

#[test]
fn speed_in_tiles_per_second_becomes_tiles_per_tick() {
    // 2.5 tiles a second is exactly an eighth of a tile a tick.
    let converted = speed_per_tick(2.5);
    assert_eq!(converted.value, (1, 8));
    assert!(!converted.was_rounded());
    // 1 tile a second is 1/20 of a tile, 3276.8 in 65536ths: rounded.
    let converted = speed_per_tick(1.0);
    assert_eq!(converted.value, (3277, 65536));
    assert!(converted.was_rounded());
}

#[test]
fn a_distance_is_written_as_a_fraction_in_lowest_terms() {
    assert_eq!(fraction(0.25).value, (1, 4));
    assert_eq!(fraction(1.5).value, (3, 2));
    assert_eq!(fraction(0.0).value, (0, 1));
    assert!(!fraction(1.5).was_rounded());
    // 0.2 has no exact 16.16 form.
    assert_eq!(fraction(0.2).value, (13107, 65536));
    assert!(fraction(0.2).was_rounded());
}

#[test]
fn damage_is_the_base_melee_or_pierce_attack() {
    // Class 4 is melee, 3 pierce; bonus classes such as 11 and 13 wait for
    // Phase 6.
    assert_eq!(damage(&[(11, 3), (4, 3), (13, 6)]), 3);
    assert_eq!(damage(&[(3, 5)]), 5);
    assert_eq!(damage(&[(4, 4), (3, 2)]), 6);
    assert_eq!(damage(&[(29, 7)]), 0);
    assert_eq!(damage(&[]), 0);
}

#[test]
fn costs_map_by_resource_name_not_position() {
    // The original orders food, wood, stone, gold; ours food, wood, gold,
    // stone. Population (4) and unused parts are left out.
    let part = |resource, amount, used| Cost {
        resource,
        amount,
        used,
    };
    let militia = [part(0, 60, true), part(3, 20, true), part(4, 1, false)];
    assert_eq!(cost(&militia), [60, 0, 20, 0]);
    let stone_first = [part(2, 100, true), part(1, 25, true), part(0, 9, false)];
    assert_eq!(cost(&stone_first), [0, 25, 0, 100]);
}

#[test]
fn a_buildings_size_is_twice_its_half_width_rounded() {
    assert_eq!(building_size(2.0).value, 4);
    assert_eq!(building_size(1.5).value, 3);
    assert_eq!(building_size(0.5).value, 1);
    assert!(!building_size(1.5).was_rounded());
    // 2.5 tiles rounds, halves away from zero, to 3.
    assert_eq!(building_size(1.25).value, 3);
    assert!(building_size(1.25).was_rounded());
}

#[test]
fn the_gather_rate_becomes_ticks_per_unit_gathered() {
    // One gold a second is 20 ticks a gold; 0.379 a second is 52.77 ticks.
    assert_eq!(ticks_per_gold(1.0).value, 20);
    assert!(!ticks_per_gold(1.0).was_rounded());
    assert_eq!(ticks_per_gold(0.379).value, 53);
    assert!(ticks_per_gold(0.379).was_rounded());
}
