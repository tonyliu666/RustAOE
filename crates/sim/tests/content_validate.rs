//! Content validation: numbers that would break the game are refused once, at
//! load time, with the name of the field at fault.
//!
//! Each test breaks exactly one field of the default content, so a rejection
//! can only come from the rule under test.

use sim::{BuildingKind, Content, ContentError, Fx, UnitKind};

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
    // An unarmed unit (damage = 0) may have zero attack cooldown.
    let mut content = Content::default();
    content.unit_mut(UnitKind::VILLAGER).damage = 0;
    content.unit_mut(UnitKind::VILLAGER).attack_cooldown = 0;
    assert_eq!(content.validate(), Ok(()));
}

// --- units ----------------------------------------------------------------

#[test]
fn a_unit_needs_positive_hit_points() {
    assert_rejects(
        |c| c.unit_mut(UnitKind::SOLDIER).max_hp = 0,
        "soldier.max_hp",
        POSITIVE,
    );
}

#[test]
fn a_unit_needs_positive_speed() {
    assert_rejects(
        |c| c.unit_mut(UnitKind::VILLAGER).speed = Fx::ZERO,
        "villager.speed",
        POSITIVE,
    );
}

#[test]
fn a_unit_cannot_move_more_than_half_a_tile_per_tick() {
    // Faster steps overshoot the arrival window, so the unit never arrives,
    // and past one tile a step can hop a one-tile wall.
    assert_rejects(
        |c| c.unit_mut(UnitKind::SOLDIER).speed = Fx::ONE,
        "soldier.speed",
        "must be at most 1/2 tile per tick",
    );
}

#[test]
fn half_a_tile_per_tick_is_the_fastest_allowed_speed() {
    let mut content = Content::default();
    content.unit_mut(UnitKind::SOLDIER).speed = Fx::from_frac(1, 2);
    assert_eq!(content.validate(), Ok(()));
}

#[test]
fn a_unit_needs_a_positive_radius() {
    assert_rejects(
        |c| c.unit_mut(UnitKind::SOLDIER).radius = Fx::from_int(-1),
        "soldier.radius",
        POSITIVE,
    );
}

#[test]
fn a_unit_needs_a_positive_training_time() {
    assert_rejects(
        |c| c.unit_mut(UnitKind::VILLAGER).train_ticks = 0,
        "villager.train_ticks",
        POSITIVE,
    );
}

#[test]
fn damage_cannot_be_negative() {
    assert_rejects(
        |c| c.unit_mut(UnitKind::SOLDIER).damage = -1,
        "soldier.damage",
        NOT_NEGATIVE,
    );
}

#[test]
fn range_cannot_be_negative() {
    assert_rejects(
        |c| c.unit_mut(UnitKind::SOLDIER).range = Fx::from_int(-1),
        "soldier.range",
        NOT_NEGATIVE,
    );
}

#[test]
fn cost_cannot_be_negative() {
    // A negative cost would hand the player gold for queueing a unit.
    assert_rejects(
        |c| c.unit_mut(UnitKind::VILLAGER).cost.gold = -50,
        "villager.cost.gold",
        NOT_NEGATIVE,
    );
}

#[test]
fn no_part_of_a_cost_can_be_negative() {
    assert_rejects(
        |c| c.unit_mut(UnitKind::VILLAGER).cost.food = -1,
        "villager.cost.food",
        NOT_NEGATIVE,
    );
    assert_rejects(
        |c| c.unit_mut(UnitKind::SOLDIER).cost.stone = -1,
        "soldier.cost.stone",
        NOT_NEGATIVE,
    );
}

#[test]
fn an_armed_unit_needs_a_positive_attack_cooldown() {
    // Zero would mean a hit every tick with no limit on the rate.
    assert_rejects(
        |c| c.unit_mut(UnitKind::SOLDIER).attack_cooldown = 0,
        "soldier.attack_cooldown",
        "must be positive when damage is positive",
    );
}

// --- buildings ------------------------------------------------------------

#[test]
fn a_building_needs_positive_hit_points() {
    assert_rejects(
        |c| c.building_mut(BuildingKind::TOWN_CENTER).max_hp = 0,
        "town_center.max_hp",
        POSITIVE,
    );
    assert_rejects(
        |c| c.building_mut(BuildingKind::BARRACKS).max_hp = -5,
        "barracks.max_hp",
        POSITIVE,
    );
}

// --- gathering ------------------------------------------------------------

#[test]
fn carry_capacity_must_be_positive() {
    assert_rejects(
        |c| c.gather.gold.carry_capacity = 0,
        "gather.gold.carry_capacity",
        POSITIVE,
    );
}

#[test]
fn ticks_per_gold_must_be_positive() {
    assert_rejects(
        |c| c.gather.gold.ticks_per_unit = 0,
        "gather.gold.ticks_per_unit",
        POSITIVE,
    );
}

// --- the message ----------------------------------------------------------

#[test]
fn the_error_message_names_the_field() {
    let error = reject(|c| c.unit_mut(UnitKind::SOLDIER).max_hp = 0);
    assert_eq!(error.to_string(), "soldier.max_hp: must be positive");
}

// --- names and references ----------------------------------------------------

#[test]
fn unit_names_must_be_unique() {
    assert_rejects(
        |c| c.unit_mut(UnitKind::SOLDIER).name = "villager".to_string(),
        "units[1].name",
        "must be unique",
    );
}

#[test]
fn building_names_must_be_unique() {
    assert_rejects(
        |c| c.building_mut(BuildingKind::BARRACKS).name = "town_center".to_string(),
        "buildings[1].name",
        "must be unique",
    );
}

#[test]
fn a_name_cannot_be_empty() {
    assert_rejects(
        |c| c.unit_mut(UnitKind::VILLAGER).name = String::new(),
        "units[0].name",
        "must not be empty",
    );
}

#[test]
fn a_building_can_only_train_unit_types_the_content_has() {
    assert_rejects(
        |c| {
            c.building_mut(BuildingKind::BARRACKS)
                .trains
                .push(UnitKind::new(9))
        },
        "barracks.trains",
        "must name unit types in this content",
    );
}

#[test]
fn a_building_needs_a_positive_size() {
    assert_rejects(
        |c| c.building_mut(BuildingKind::BARRACKS).size = 0,
        "barracks.size",
        POSITIVE,
    );
}

#[test]
fn content_needs_at_least_one_unit_type() {
    assert_rejects(|c| c.units.clear(), "units", "must not be empty");
}

// --- limits -------------------------------------------------------------------

#[test]
fn a_building_cannot_be_larger_than_sixteen_tiles_a_side() {
    // The original's largest building, a wonder, is 5x5. Beyond a sane bound
    // the footprint arithmetic overflows, which would panic a debug build and
    // wrap a release build.
    assert_rejects(
        |c| c.building_mut(BuildingKind::TOWN_CENTER).size = 17,
        "town_center.size",
        "must be at most 16",
    );
}

#[test]
fn sixteen_tiles_a_side_is_the_largest_allowed_building() {
    let mut content = Content::default();
    content.building_mut(BuildingKind::TOWN_CENTER).size = 16;

    assert_eq!(content.validate(), Ok(()));
}

#[test]
fn there_cannot_be_more_unit_types_than_an_id_can_name() {
    // A type ID is a u16; a 65,537th type would wrap round to type 0.
    let mut content = Content::default();
    let spare = content.unit(UnitKind::SOLDIER).clone();
    for i in 0..(usize::from(u16::MAX) + 1 - content.units.len() + 1) {
        let mut unit = spare.clone();
        unit.name = format!("extra{i}");
        content.units.push(unit);
    }

    let error = content.validate().expect_err("too many unit types");
    assert_eq!(
        (error.field.as_str(), error.problem),
        ("units", "must have at most 65536 entries")
    );
}

#[test]
fn there_cannot_be_more_building_types_than_an_id_can_name() {
    let mut content = Content::default();
    let spare = content.building(BuildingKind::BARRACKS).clone();
    for i in 0..(usize::from(u16::MAX) + 1 - content.buildings.len() + 1) {
        let mut building = spare.clone();
        building.name = format!("extra{i}");
        content.buildings.push(building);
    }

    let error = content.validate().expect_err("too many building types");
    assert_eq!(
        (error.field.as_str(), error.problem),
        ("buildings", "must have at most 65536 entries")
    );
}

#[test]
fn a_range_longer_than_any_map_is_rejected() {
    // Reach is compared squared in 64 bits; an unbounded range would
    // overflow it, and a debug and a release build would disagree.
    assert_rejects(
        |c| c.unit_mut(UnitKind::SOLDIER).range = Fx::from_int(65),
        "soldier.range",
        "must be at most 64 tiles",
    );
}

#[test]
fn a_radius_wider_than_the_largest_building_is_rejected() {
    assert_rejects(
        |c| c.unit_mut(UnitKind::SOLDIER).radius = Fx::from_int(9),
        "soldier.radius",
        "must be at most 8 tiles",
    );
}

#[test]
fn the_longest_range_and_widest_radius_allowed_still_measure_reach_safely() {
    // At both limits, two such units fight without overflow.
    let mut content = Content::default();
    content.unit_mut(UnitKind::SOLDIER).range = Fx::from_int(64);
    content.unit_mut(UnitKind::SOLDIER).radius = Fx::from_int(8);
    assert_eq!(content.validate(), Ok(()));
    let map = sim::MapSpec::new(32, 32)
        .with_building(
            sim::BuildingKind::TOWN_CENTER,
            sim::PlayerId::ONE,
            sim::TilePos::new(0, 0),
        )
        .with_building(
            sim::BuildingKind::TOWN_CENTER,
            sim::PlayerId::TWO,
            sim::TilePos::new(28, 28),
        )
        .with_unit(
            UnitKind::SOLDIER,
            sim::PlayerId::ONE,
            sim::TilePos::new(10, 10),
        )
        .with_unit(
            UnitKind::SOLDIER,
            sim::PlayerId::TWO,
            sim::TilePos::new(20, 20),
        );
    let mut world = sim::World::with_content(1, map, content);
    let (attacker, _) = world
        .view()
        .units()
        .find(|(_, u)| u.owner == sim::PlayerId::ONE)
        .expect("attacker");
    let (victim, _) = world
        .view()
        .units()
        .find(|(_, u)| u.owner == sim::PlayerId::TWO)
        .expect("victim");

    world.step(&[sim::Command::Attack {
        player: sim::PlayerId::ONE,
        units: [attacker].into_iter().collect(),
        target: sim::EntityId::Unit(victim),
    }]);

    assert!(
        world.view().unit(victim).is_some_and(|u| u.hp < 60),
        "in reach at once"
    );
}
