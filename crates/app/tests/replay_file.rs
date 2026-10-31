//! Replay files survive the round trip to disk and back.

use sim::{
    BuildingKind, Command, Content, MapSpec, PlayerId, Replay, TilePos, UnitId, UnitKind, World,
};
use std::path::PathBuf;

/// A path in the temporary directory, unique to this test binary and name.
///
/// Hand-rolled rather than pulling in a temporary-file crate: the whole need
/// is two tests writing two files.
fn scratch(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("aoe-replay-test-{name}.ron"));
    path
}

fn map() -> MapSpec {
    MapSpec::new(24, 24)
        .with_starting_gold(200)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(18, 18),
        )
        .with_mine(TilePos::new(10, 10), 500)
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(6, 6))
}

/// Records a short session of a villager being sent to a mine.
fn recorded() -> Replay {
    let mut world = World::new(99, map());
    let mut replay = Replay::new(99, map());
    let (villager, _) = world.view().units().next().expect("a villager");
    let (mine, _) = world.view().sites().next().expect("a mine");

    for tick in 0..250u64 {
        let commands: Vec<Command> = if tick == 3 {
            vec![Command::Gather {
                player: PlayerId::ONE,
                units: [villager]
                    .into_iter()
                    .collect::<smallvec::SmallVec<[UnitId; 32]>>(),
                site: mine,
            }]
        } else {
            Vec::new()
        };
        replay.record(tick, &commands);
        replay.record_checksum(tick, world.checksum());
        world.step(&commands);
    }
    replay
}

#[test]
fn a_replay_written_to_disk_reads_back_unchanged() {
    let path = scratch("round-trip");
    let replay = recorded();

    app::save(&path, &replay).expect("the replay should write");
    let loaded = app::load(&path).expect("the replay should read back");

    assert_eq!(loaded, replay);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_replay_read_from_disk_still_verifies() {
    let path = scratch("verifies");
    app::save(&path, &recorded()).expect("the replay should write");

    let loaded = app::load(&path).expect("the replay should read back");

    assert!(sim::verify(&loaded).is_ok(), "{:?}", sim::verify(&loaded));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_file_that_is_not_a_replay_is_an_error_rather_than_a_panic() {
    let path = scratch("garbage");
    std::fs::write(&path, "this is not a replay").expect("the scratch file should write");

    let error = app::load(&path);

    assert!(
        matches!(error, Err(app::ReplayError::Malformed(_))),
        "{error:?}"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_missing_replay_file_is_an_io_error() {
    let error = app::load(&scratch("does-not-exist-at-all"));

    assert!(matches!(error, Err(app::ReplayError::Io(_))), "{error:?}");
}

#[test]
fn a_replay_keeps_the_content_it_was_recorded_with() {
    let path = scratch("content");
    let mut content = Content::default();
    content.unit_mut(UnitKind::SOLDIER).damage = 13;
    let replay = Replay::with_content(5, map(), content.clone());

    app::save(&path, &replay).expect("the replay should write");
    let loaded = app::load(&path).expect("the replay should read back");

    assert_eq!(loaded.content, content);
    let _ = std::fs::remove_file(&path);
}

/// Writes `text` to a scratch file and loads it as a replay.
fn load_text(name: &str, text: &str) -> Result<Replay, app::ReplayError> {
    let path = scratch(name);
    std::fs::write(&path, text).expect("the scratch file should write");
    let result = app::load(&path);
    let _ = std::fs::remove_file(&path);
    result
}

/// A replay file as written before format versions existed: no `version`
/// field, and no `content` either. Version 1 by definition.
const VERSION_ONE: &str = "(
    seed: 7,
    map: (
        width: 8,
        height: 8,
        blocked: [],
        buildings: [],
        mines: [],
        units: [],
        starting_gold: 0,
    ),
    commands: [],
    checksums: [],
)";

#[test]
fn a_replay_is_written_with_the_current_format_version() {
    let path = scratch("version");
    app::save(&path, &recorded()).expect("the replay should write");

    let loaded = app::load(&path).expect("the replay should read back");

    assert_eq!(loaded.version, Replay::FORMAT_VERSION);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_replay_from_before_format_versions_is_refused_by_version() {
    let error = load_text("version-one", VERSION_ONE);

    match error {
        Err(app::ReplayError::Version { found, expected }) => {
            assert_eq!((found, expected), (1, Replay::FORMAT_VERSION));
        }
        other => panic!("expected a version error, got {other:?}"),
    }
}

#[test]
fn the_version_error_names_both_versions() {
    let error = load_text("version-message", VERSION_ONE).expect_err("refused");

    assert_eq!(
        error.to_string(),
        format!(
            "replay is format version 1; this build plays version {}",
            Replay::FORMAT_VERSION
        )
    );
}

#[test]
fn a_replay_from_a_newer_build_is_refused_by_version() {
    // Only the version is read before refusing, so the rest of the file may
    // be in a shape this build does not know at all.
    let error = load_text(
        "version-future",
        "(version: 99, something_new: (a: 1), seed: 1)",
    );

    assert!(
        matches!(error, Err(app::ReplayError::Version { found: 99, .. })),
        "{error:?}"
    );
}

#[test]
fn a_replay_carrying_content_the_game_cannot_run_with_is_refused() {
    // A replay is a shared bug report, so its content is user input exactly
    // like a `--content` file and gets the same checks.
    let path = scratch("invalid-content");
    let mut content = Content::default();
    content.unit_mut(UnitKind::VILLAGER).max_hp = 0;
    app::save(&path, &Replay::with_content(5, map(), content)).expect("the replay should write");

    let error = app::load(&path);

    match error {
        Err(app::ReplayError::Invalid(error)) => assert_eq!(error.field, "villager.max_hp"),
        other => panic!("expected an invalid-content error, got {other:?}"),
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_replay_whose_map_gives_an_entity_to_a_missing_player_is_refused() {
    let path = scratch("invalid-map");
    let map = map().with_unit(UnitKind::VILLAGER, PlayerId::new(3), TilePos::new(4, 4));
    app::save(&path, &Replay::new(5, map)).expect("the replay should write");

    let error = app::load(&path);

    assert!(
        matches!(error, Err(app::ReplayError::InvalidMap(_))),
        "{error:?}"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_replay_from_the_middle_of_phase_1_5_is_refused_by_version() {
    // Format 2 had two layouts while content changed shape; format 3 is the
    // first written once unit and building types became data.
    let error = load_text("version-two", "(version: 2, seed: 1)");

    assert!(
        matches!(error, Err(app::ReplayError::Version { found: 2, .. })),
        "{error:?}"
    );
}

#[test]
fn a_replay_whose_map_names_a_type_its_content_lacks_is_refused_rather_than_crashing() {
    // Found by the Phase 1.5 review: this replay loaded, then `verify`
    // panicked indexing building type 9 of 2.
    let path = scratch("unknown-kind");
    let map = map().with_building(BuildingKind::new(9), PlayerId::ONE, TilePos::new(12, 2));
    app::save(&path, &Replay::new(5, map)).expect("the replay should write");

    let error = app::load(&path);

    assert!(
        matches!(error, Err(app::ReplayError::InvalidMap(_))),
        "{error:?}"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_replay_from_before_icons_is_refused_by_version() {
    let error = load_text("version-three", "(version: 3, seed: 1)");

    assert!(
        matches!(error, Err(app::ReplayError::Version { found: 3, .. })),
        "{error:?}"
    );
}

#[test]
fn a_replay_from_before_drawing_in_parts_is_refused_by_version() {
    let error = load_text("version-four", "(version: 4, seed: 1)");

    assert!(
        matches!(error, Err(app::ReplayError::Version { found: 4, .. })),
        "{error:?}"
    );
}

#[test]
fn a_replay_from_before_edge_to_edge_range_is_refused_by_version() {
    // Version 5 measured reach centre to centre; its fights would play out
    // differently now.
    let error = load_text("version-five", "(version: 5, seed: 1)");

    assert!(
        matches!(error, Err(app::ReplayError::Version { found: 5, .. })),
        "{error:?}"
    );
}

#[test]
fn a_replay_from_before_placed_parts_is_refused_by_version() {
    let error = load_text("version-six", "(version: 6, seed: 1)");

    assert!(
        matches!(error, Err(app::ReplayError::Version { found: 6, .. })),
        "{error:?}"
    );
}

#[test]
fn a_replay_file_holds_no_pictures() {
    // Sprites, parts, and icons are how the game looks, not how it plays:
    // a replay leaves them to whoever plays it back, so changing one never
    // breaks a recording.
    let path = scratch("no-pictures");
    app::save(&path, &recorded()).expect("the replay should write");

    let text = std::fs::read_to_string(&path).expect("the replay should read back");

    for field in ["sprite", "parts", "icon"] {
        assert!(!text.contains(field), "the replay holds `{field}`");
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_replay_from_before_pictures_left_the_replay_is_refused_by_version() {
    let error = load_text("version-seven", "(version: 7, seed: 1)");

    assert!(
        matches!(error, Err(app::ReplayError::Version { found: 7, .. })),
        "{error:?}"
    );
}

#[test]
fn a_replay_from_before_mining_on_touch_is_refused_by_version() {
    // Version 8 mined from a tile and a half away and had no Deposit command.
    let error = load_text("version-eight", "(version: 8, seed: 1)");

    assert!(
        matches!(error, Err(app::ReplayError::Version { found: 8, .. })),
        "{error:?}"
    );
}

#[test]
fn a_replay_from_before_four_resources_is_refused_by_version() {
    // Version 9 had gold sites only and one set of gather numbers.
    let error = load_text("version-nine", "(version: 9, seed: 1)");

    assert!(
        matches!(error, Err(app::ReplayError::Version { found: 9, .. })),
        "{error:?}"
    );
}

#[test]
fn a_replay_from_before_drop_offs_by_resource_is_refused_by_version() {
    let error = load_text("version-ten", "(version: 10, seed: 1)");

    assert!(
        matches!(error, Err(app::ReplayError::Version { found: 10, .. })),
        "{error:?}"
    );
}

#[test]
fn a_replay_from_before_farms_is_refused_by_version() {
    // Version 11's sites were all one tile, blocked it, and never ran out.
    let error = load_text("version-eleven", "(version: 11, seed: 1)");

    assert!(
        matches!(error, Err(app::ReplayError::Version { found: 11, .. })),
        "{error:?}"
    );
}

#[test]
fn a_replay_from_before_hand_drop_offs_stood_is_refused_by_version() {
    // Version 12's villagers went back to work after a hand drop-off.
    let error = load_text("version-twelve", "(version: 12, seed: 1)");

    assert!(
        matches!(
            error,
            Err(app::ReplayError::Version {
                found: 12,
                expected: 17
            })
        ),
        "{error:?}"
    );
}

#[test]
fn a_replay_from_before_original_numbers_is_refused_by_version() {
    // Version 13 had uniform gather numbers and wrong villager/soldier stats.
    let error = load_text(
        "version-thirteen",
        "(version: 13, seed: 1, map: (width: 8, height: 8, blocked: [], buildings: [], mines: [], units: [], starting_gold: 0))",
    );

    assert!(
        matches!(
            error,
            Err(app::ReplayError::Version {
                found: 13,
                expected: 17
            })
        ),
        "{error:?}"
    );
}

#[test]
fn a_replay_from_before_walled_in_drop_offs_were_passed_by_is_refused_by_version() {
    // Version 14 sent a load to the nearest drop-off even when no villager
    // could stand beside it.
    let error = load_text("version-fourteen", "(version: 14, seed: 1)");

    assert!(
        matches!(
            error,
            Err(app::ReplayError::Version {
                found: 14,
                expected: 17
            })
        ),
        "{error:?}"
    );
}

#[test]
fn a_replay_from_before_villagers_went_on_from_a_used_up_site_is_refused_by_version() {
    // Version 15 worked farms at the forager's rate and had villagers stand
    // when their site was used up.
    let error = load_text("version-fifteen", "(version: 15, seed: 1)");

    assert!(
        matches!(
            error,
            Err(app::ReplayError::Version {
                found: 15,
                expected: 17
            })
        ),
        "{error:?}"
    );
}

#[test]
fn a_replay_from_before_buildings_were_placed_is_refused_by_version() {
    // Version 16 had no building costs, construction times, population, or
    // placement command.
    let error = load_text("version-sixteen", "(version: 16, seed: 1)");

    assert!(
        matches!(
            error,
            Err(app::ReplayError::Version {
                found: 16,
                expected: 17
            })
        ),
        "{error:?}"
    );
}
