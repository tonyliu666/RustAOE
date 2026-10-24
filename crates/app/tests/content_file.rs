//! Content files: the RON the binary reads its numbers from.
//!
//! Every way a file can be wrong is an error value, never a panic, because a
//! content file is user input.

use app::{load_content, parse_content, ContentFileError, DEFAULT_CONTENT};
use sim::{BuildingKind, Command, Content, MapSpec, PlayerId, Resources, TilePos, UnitKind, World};

/// The shipped file with exactly one piece of text swapped.
fn default_with(from: &str, to: &str) -> String {
    assert_eq!(
        DEFAULT_CONTENT.matches(from).count(),
        1,
        "`{from}` must appear exactly once in default.ron"
    );
    DEFAULT_CONTENT.replacen(from, to, 1)
}

/// The field and problem of an invalid-content error.
fn invalid(result: Result<Content, ContentFileError>) -> (String, &'static str) {
    match result {
        Err(ContentFileError::Invalid(error)) => (error.field, error.problem),
        other => panic!("expected an invalid-content error, got {other:?}"),
    }
}

#[test]
fn the_shipped_file_holds_the_default_content() {
    assert_eq!(
        parse_content(DEFAULT_CONTENT).expect("default.ron should parse"),
        Content::default()
    );
}

#[test]
fn changing_a_number_in_the_file_changes_the_game() {
    let content =
        parse_content(&default_with("cost: (gold: 50)", "cost: (gold: 30)")).expect("should parse");
    let map = MapSpec::new(16, 16)
        .with_starting_gold(200)
        .with_building(BuildingKind::TownCenter, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TownCenter,
            PlayerId::TWO,
            TilePos::new(10, 10),
        );
    let mut world = World::with_content(1, map, content);
    let (town_center, _) = world
        .view()
        .buildings()
        .find(|(_, b)| b.owner == PlayerId::ONE)
        .expect("a town centre");

    world.step(&[Command::Train {
        player: PlayerId::ONE,
        building: town_center,
        unit: UnitKind::Villager,
    }]);

    assert_eq!(world.view().gold(PlayerId::ONE), 170);
}

#[test]
fn text_that_is_not_ron_is_an_error_rather_than_a_panic() {
    let result = parse_content("this is not a content file");

    assert!(
        matches!(result, Err(ContentFileError::Malformed(_))),
        "{result:?}"
    );
}

#[test]
fn a_file_missing_a_section_is_an_error() {
    let result = parse_content("(villager: (max_hp: 40))");

    assert!(
        matches!(result, Err(ContentFileError::Malformed(_))),
        "{result:?}"
    );
}

#[test]
fn a_misspelled_field_is_an_error_rather_than_ignored() {
    // Without this a typo like `hp_regen` would load and do nothing, and the
    // author would never learn the number had no effect.
    let result = parse_content(&default_with(
        "max_hp: 40,",
        "max_hp: 40,\n        hp_regen: 1,",
    ));

    assert!(
        matches!(result, Err(ContentFileError::Malformed(_))),
        "{result:?}"
    );
}

#[test]
fn a_number_the_game_cannot_run_with_names_its_field() {
    let result = parse_content(&default_with("max_hp: 40,", "max_hp: 0,"));

    assert_eq!(
        invalid(result),
        ("villager.max_hp".to_string(), "must be positive")
    );
}

#[test]
fn a_fraction_over_zero_is_an_error_rather_than_a_division_panic() {
    let result = parse_content(&default_with(
        "radius: (1, 4),\n        cost: (gold: 50)",
        "radius: (1, 0),\n        cost: (gold: 50)",
    ));

    assert_eq!(
        invalid(result),
        (
            "villager.radius".to_string(),
            "denominator must be positive"
        )
    );
}

#[test]
fn a_fraction_too_large_for_fixed_point_is_an_error_rather_than_wrapping() {
    let result = parse_content(&default_with(
        "radius: (1, 4),\n        cost: (gold: 50)",
        "radius: (100000, 1),\n        cost: (gold: 50)",
    ));

    // Wrapped, this would come out negative and be reported as "must be
    // positive", which points the author at the wrong mistake.
    assert_eq!(
        invalid(result),
        ("villager.radius".to_string(), "too large for fixed point")
    );
}

#[test]
fn a_missing_file_is_an_io_error() {
    let mut path = std::env::temp_dir();
    path.push("aoe-content-test-does-not-exist.ron");

    let result = load_content(&path);

    assert!(matches!(result, Err(ContentFileError::Io(_))), "{result:?}");
}

#[test]
fn a_resource_left_out_of_a_cost_reads_as_zero() {
    let content = parse_content(DEFAULT_CONTENT).expect("default.ron should parse");

    assert_eq!(
        content.villager.cost,
        Resources {
            gold: 50,
            ..Resources::default()
        }
    );
}

#[test]
fn a_misspelled_resource_in_a_cost_is_an_error() {
    let result = parse_content(&default_with("cost: (gold: 50)", "cost: (gld: 50)"));

    assert!(
        matches!(result, Err(ContentFileError::Malformed(_))),
        "{result:?}"
    );
}
