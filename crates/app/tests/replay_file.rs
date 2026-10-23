//! Replay files survive the round trip to disk and back.

use sim::{BuildingKind, Command, MapSpec, PlayerId, Replay, TilePos, UnitId, UnitKind, World};
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
        .with_building(BuildingKind::TownCenter, PlayerId::One, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TownCenter,
            PlayerId::Two,
            TilePos::new(18, 18),
        )
        .with_mine(TilePos::new(10, 10), 500)
        .with_unit(UnitKind::Villager, PlayerId::One, TilePos::new(6, 6))
}

/// Records a short session of a villager being sent to a mine.
fn recorded() -> Replay {
    let mut world = World::new(99, map());
    let mut replay = Replay::new(99, map());
    let (villager, _) = world.view().units().next().expect("a villager");
    let (mine, _) = world.view().mines().next().expect("a mine");

    for tick in 0..250u64 {
        let commands: Vec<Command> = if tick == 3 {
            vec![Command::Gather {
                player: PlayerId::One,
                units: [villager]
                    .into_iter()
                    .collect::<smallvec::SmallVec<[UnitId; 32]>>(),
                mine,
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
