//! The minimal world: terrain, tick counter, checksum.

use sim::{MapSpec, TilePos, World};

fn flat_map() -> MapSpec {
    MapSpec::new(8, 8)
}

#[test]
fn a_new_world_starts_at_tick_zero() {
    let world = World::new(1, flat_map());

    assert_eq!(world.tick(), 0);
}

#[test]
fn stepping_advances_exactly_one_tick() {
    let mut world = World::new(1, flat_map());

    world.step(&[]);
    world.step(&[]);

    assert_eq!(world.tick(), 2);
}

#[test]
fn a_map_with_no_blocked_tiles_is_passable_everywhere() {
    let world = World::new(1, flat_map());
    let view = world.view();

    for y in 0..view.height() {
        for x in 0..view.width() {
            assert!(
                view.tile(TilePos::new(x, y)).expect("in bounds").passable,
                "tile {x},{y} should be passable"
            );
        }
    }
}

#[test]
fn blocked_tiles_from_the_map_spec_are_impassable() {
    let map = MapSpec::new(8, 8).with_blocked([TilePos::new(3, 4)]);
    let world = World::new(1, map);

    let blocked = world.view().tile(TilePos::new(3, 4)).expect("in bounds");
    let open = world.view().tile(TilePos::new(4, 4)).expect("in bounds");

    assert!(!blocked.passable);
    assert!(open.passable);
}

#[test]
fn a_tile_outside_the_map_reads_as_missing_rather_than_panicking() {
    let world = World::new(1, flat_map());

    assert!(world.view().tile(TilePos::new(-1, 0)).is_none());
    assert!(world.view().tile(TilePos::new(8, 0)).is_none());
}

#[test]
fn the_checksum_changes_as_the_world_ticks() {
    let mut world = World::new(1, flat_map());
    let before = world.checksum();

    world.step(&[]);

    assert_ne!(before, world.checksum());
}

#[test]
fn two_worlds_from_the_same_seed_and_map_agree_on_their_checksum() {
    let mut a = World::new(7, flat_map());
    let mut b = World::new(7, flat_map());

    for _ in 0..10 {
        a.step(&[]);
        b.step(&[]);
        assert_eq!(a.checksum(), b.checksum(), "at tick {}", a.tick());
    }
}

#[test]
fn worlds_from_different_seeds_do_not_share_a_checksum() {
    let a = World::new(1, flat_map());
    let b = World::new(2, flat_map());

    assert_ne!(a.checksum(), b.checksum());
}
