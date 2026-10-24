//! Content validation: numbers that would break the game are refused once, at
//! load time, with the name of the field at fault.
//!
//! Each test breaks exactly one field of the default content, so a rejection
//! can only come from the rule under test.

use sim::{Content, ContentError, Fx};

/// Breaks one field of the default content and returns the validation error.
fn reject(break_it: impl FnOnce(&mut Content)) -> ContentError {
    let mut content = Content::default();
    break_it(&mut content);
    content
        .validate()
        .expect_err("broken content passed validation")
}

fn assert_rejects(break_it: impl FnOnce(&mut Content), field: &str, problem: &str) {
    let error = reject(break_it);
    assert_eq!((error.field.as_str(), error.problem), (field, problem));
}

const POSITIVE: &str = "must be positive";
const NOT_NEGATIVE: &str = "must not be negative";

#[test]
fn the_default_content_is_valid() {
    assert_eq!(Content::default().validate(), Ok(()));
}

#[test]
fn an_unarmed_unit_needs_no_attack_cooldown() {
    // The default villager has zero damage and zero cooldown.
    let content = Content::default();
    assert_eq!(content.villager.damage, 0);
    assert_eq!(content.villager.attack_cooldown, 0);
    assert_eq!(content.validate(), Ok(()));
}

// --- units ----------------------------------------------------------------

#[test]
fn a_unit_needs_positive_hit_points() {
    assert_rejects(|c| c.soldier.max_hp = 0, "soldier.max_hp", POSITIVE);
}

#[test]
fn a_unit_needs_positive_speed() {
    assert_rejects(|c| c.villager.speed = Fx::ZERO, "villager.speed", POSITIVE);
}

#[test]
fn a_unit_cannot_move_more_than_half_a_tile_per_tick() {
    // Faster steps overshoot the arrival window, so the unit never arrives,
    // and past one tile a step can hop a one-tile wall.
    assert_rejects(
        |c| c.soldier.speed = Fx::ONE,
        "soldier.speed",
        "must be at most 1/2 tile per tick",
    );
}

#[test]
fn half_a_tile_per_tick_is_the_fastest_allowed_speed() {
    let mut content = Content::default();
    content.soldier.speed = Fx::from_frac(1, 2);
    assert_eq!(content.validate(), Ok(()));
}

#[test]
fn a_unit_needs_a_positive_radius() {
    assert_rejects(
        |c| c.soldier.radius = Fx::from_int(-1),
        "soldier.radius",
        POSITIVE,
    );
}

#[test]
fn a_unit_needs_a_positive_training_time() {
    assert_rejects(
        |c| c.villager.train_ticks = 0,
        "villager.train_ticks",
        POSITIVE,
    );
}

#[test]
fn damage_cannot_be_negative() {
    assert_rejects(|c| c.soldier.damage = -1, "soldier.damage", NOT_NEGATIVE);
}

#[test]
fn range_cannot_be_negative() {
    assert_rejects(
        |c| c.soldier.range = Fx::from_int(-1),
        "soldier.range",
        NOT_NEGATIVE,
    );
}

#[test]
fn cost_cannot_be_negative() {
    // A negative cost would hand the player gold for queueing a unit.
    assert_rejects(|c| c.villager.cost = -50, "villager.cost", NOT_NEGATIVE);
}

#[test]
fn an_armed_unit_needs_a_positive_attack_cooldown() {
    // Zero would mean a hit every tick with no limit on the rate.
    assert_rejects(
        |c| c.soldier.attack_cooldown = 0,
        "soldier.attack_cooldown",
        "must be positive when damage is positive",
    );
}

// --- buildings ------------------------------------------------------------

#[test]
fn a_building_needs_positive_hit_points() {
    assert_rejects(|c| c.town_center.max_hp = 0, "town_center.max_hp", POSITIVE);
    assert_rejects(|c| c.barracks.max_hp = -5, "barracks.max_hp", POSITIVE);
}

// --- gathering ------------------------------------------------------------

#[test]
fn carry_capacity_must_be_positive() {
    assert_rejects(
        |c| c.gather.carry_capacity = 0,
        "gather.carry_capacity",
        POSITIVE,
    );
}

#[test]
fn ticks_per_gold_must_be_positive() {
    assert_rejects(
        |c| c.gather.ticks_per_gold = 0,
        "gather.ticks_per_gold",
        POSITIVE,
    );
}

// --- the message ----------------------------------------------------------

#[test]
fn the_error_message_names_the_field() {
    let error = reject(|c| c.soldier.max_hp = 0);
    assert_eq!(error.to_string(), "soldier.max_hp: must be positive");
}
