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
        parse_content(&default_with("cost: (food: 50)", "cost: (food: 30)")).expect("should parse");
    let map = MapSpec::new(16, 16)
        .with_starting_resources(Resources {
            food: 200,
            ..Resources::default()
        })
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TOWN_CENTER,
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
        unit: UnitKind::VILLAGER,
    }]);

    assert_eq!(world.view().resources(PlayerId::ONE).food, 170);
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
        "max_hp: 40,\n            hp_regen: 1,",
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
        "radius: (1, 4),\n            cost: (food: 50)",
        "radius: (1, 0),\n            cost: (food: 50)",
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
        "radius: (1, 4),\n            cost: (food: 50)",
        "radius: (100000, 1),\n            cost: (food: 50)",
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
        content.unit(UnitKind::VILLAGER).cost,
        Resources {
            food: 50,
            ..Resources::default()
        }
    );
}

#[test]
fn a_misspelled_resource_in_a_cost_is_an_error() {
    let result = parse_content(&default_with("cost: (food: 50)", "cost: (fod: 50)"));

    assert!(
        matches!(result, Err(ContentFileError::Malformed(_))),
        "{result:?}"
    );
}

// --- types are entries ----------------------------------------------------------

#[test]
fn a_unit_type_added_only_in_the_file_loads_and_can_be_trained() {
    let archer = r#"(
            name: "archer",
            sprite: 1270,
            parts: [],
            icon: 17,
            gathers: false,
            max_hp: 30,
            speed: (1, 8),
            damage: 6,
            range: (4, 1),
            attack_cooldown: 20,
            radius: (1, 4),
            cost: (gold: 45),
            train_ticks: 120,
        ),
    ],
    buildings: ["#;
    let text = default_with("    ],\n    buildings: [", archer);
    let text = text.replacen(
        r#"trains: ["soldier"]"#,
        r#"trains: ["soldier", "archer"]"#,
        1,
    );

    let content = parse_content(&text).expect("an added unit type loads");

    let kind = content
        .unit_named("archer")
        .expect("the archer is a unit type");
    assert_eq!(content.unit(kind).damage, 6);
    assert!(content
        .building(BuildingKind::BARRACKS)
        .trains
        .contains(&kind));
}

#[test]
fn a_building_that_trains_a_unit_type_the_file_does_not_have_is_an_error() {
    let result = parse_content(&default_with(
        r#"trains: ["soldier"]"#,
        r#"trains: ["dragon"]"#,
    ));

    assert_eq!(
        invalid(result),
        (
            "barracks.trains".to_string(),
            "names a unit type the content does not have"
        )
    );
}

#[test]
fn an_icon_is_read_from_the_file() {
    let looks = app::parse_looks(&default_with("icon: 15,", "icon: 99,")).expect("parses");

    assert_eq!(looks.unit("villager").map(|l| l.icon), Some(99));
}

#[test]
fn a_type_without_an_icon_is_an_error() {
    let result = parse_content(&default_with("icon: 15,", ""));

    assert!(
        matches!(result, Err(ContentFileError::Malformed(_))),
        "{result:?}"
    );
}

#[test]
fn a_types_parts_are_read_from_the_file() {
    let looks = app::parse_looks(&default_with(
        "sprite: 1473,\n            parts: [],",
        "sprite: 1473,\n            parts: [(7, 0, 0), (1473, 0, 0), (8, 3, -2)],",
    ))
    .expect("parses");

    assert_eq!(
        looks.unit("villager").expect("the villager's look").parts,
        [
            app::Part::new(7, 0, 0),
            app::Part::new(1473, 0, 0),
            app::Part::new(8, 3, -2),
        ]
    );
}

#[test]
fn a_type_without_parts_is_an_error() {
    let result = parse_content(&default_with(
        "sprite: 1473,\n            parts: [],",
        "sprite: 1473,",
    ));

    assert!(
        matches!(result, Err(ContentFileError::Malformed(_))),
        "{result:?}"
    );
}

// --- how each type looks ----------------------------------------------------------

#[test]
fn a_types_look_is_read_from_the_file_by_its_name() {
    let looks = app::parse_looks(DEFAULT_CONTENT).expect("default.ron should parse");

    let villager = looks.unit("villager").expect("the villager's look");
    assert_eq!((villager.sprite, villager.icon), (1473, 15));
    let town_centre = looks
        .building("town_center")
        .expect("the town centre's look");
    assert_eq!(town_centre.sprite, 3596);
    assert_eq!(town_centre.parts.len(), 8);
    assert_eq!(town_centre.parts[0], app::Part::new(889, 0, 0));
}

#[test]
fn changing_how_a_type_looks_leaves_the_rules_unchanged() {
    // So a replay recorded before the change still plays the same game.
    let restyled = default_with("sprite: 1473,", "sprite: 9999,");
    let restyled = restyled.replace("icon: 15,", "icon: 3,");

    assert_eq!(
        parse_content(&restyled).expect("parses"),
        parse_content(DEFAULT_CONTENT).expect("parses")
    );
    assert_eq!(
        app::parse_looks(&restyled)
            .expect("parses")
            .unit("villager")
            .map(|l| l.sprite),
        Some(9999)
    );
}

#[test]
fn the_default_types_look_like_the_originals() {
    // From the .dat: icon frames (villager 15, militia 8, town centre 28,
    // barracks 2); the militia (unit 74) stands with SLP 993; the barracks
    // (unit 12) is SLP 2683, one piece; the town centre (unit 109) is its
    // graphic's pieces (889, 3596, 4612; 890 is in no archive) and its three
    // annexes' (891; 3595, 4611; 3594, 4610), at one point in that order.
    let looks = app::default_looks();
    let unit = |name| looks.unit(name).expect("a look");
    let building = |name| looks.building(name).expect("a look");

    assert_eq!((unit("villager").icon, unit("soldier").icon), (15, 8));
    assert_eq!(
        (building("town_center").icon, building("barracks").icon),
        (28, 2)
    );
    assert_eq!(unit("soldier").sprite, 993);
    assert!(unit("villager").parts.is_empty() && unit("soldier").parts.is_empty());
    assert_eq!(
        (
            building("barracks").sprite,
            building("barracks").parts.len()
        ),
        (2683, 0)
    );
    let parts: Vec<u32> = building("town_center")
        .parts
        .iter()
        .map(|p| p.slp)
        .collect();
    assert_eq!(parts, [889, 3596, 4612, 891, 3595, 4611, 3594, 4610]);
    assert!(building("town_center")
        .parts
        .iter()
        .all(|p| p.offset == [0, 0]));
}

#[test]
fn every_default_type_has_a_look() {
    let content = parse_content(DEFAULT_CONTENT).expect("parses");
    let looks = app::default_looks();

    for unit in &content.units {
        assert!(looks.unit(&unit.name).is_some(), "{}", unit.name);
    }
    for building in &content.buildings {
        assert!(
            looks.building(&building.name).is_some(),
            "{}",
            building.name
        );
    }
}

#[test]
fn the_game_draws_with_the_looks_of_the_file_it_was_started_with() {
    // In a new game and in playback alike: a replay carries no pictures.
    let mut path = std::env::temp_dir();
    path.push(format!("aoe-looks-test-{}.ron", std::process::id()));
    std::fs::write(&path, default_with("sprite: 1473,", "sprite: 9999,"))
        .expect("the scratch file should write");

    let from_file = app::game_looks(Some(&path)).expect("loads");
    let built_in = app::game_looks(None).expect("loads");

    assert_eq!(from_file.unit("villager").map(|l| l.sprite), Some(9999));
    assert_eq!(built_in, app::default_looks());
    let _ = std::fs::remove_file(&path);
}
