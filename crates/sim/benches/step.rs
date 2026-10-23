//! Tick and flow field budgets.
//!
//! The design target is 200 units moving on a 128x128 map in under 5 ms per
//! tick — one quarter of the 50 ms tick, leaving headroom for the systems
//! later specs add. Flow field construction is measured separately because it
//! is the one operation whose cost is independent of unit count and therefore
//! needs its own budget.

use criterion::{criterion_group, criterion_main, Criterion};
use sim::{
    BuildingKind, Command, FlowField, Grid, MapSpec, PlayerId, Tile, TilePos, UnitId, UnitKind,
    World,
};
use smallvec::SmallVec;
use std::hint::black_box;

const MAP_SIZE: i32 = 128;
const UNITS_PER_SIDE: i32 = 100;

/// A 128x128 map with 200 units and a wall the length of the map to path
/// around, which is the worst case the slice is specified for.
fn crowded_map() -> MapSpec {
    let mut map = MapSpec::new(MAP_SIZE, MAP_SIZE)
        .with_starting_gold(200)
        .with_building(BuildingKind::TownCenter, PlayerId::One, TilePos::new(4, 4))
        .with_building(
            BuildingKind::TownCenter,
            PlayerId::Two,
            TilePos::new(MAP_SIZE - 8, MAP_SIZE - 8),
        );

    for i in 0..UNITS_PER_SIDE {
        let (x, y) = (i % 20, i / 20);
        map = map
            .with_unit(
                UnitKind::Soldier,
                PlayerId::One,
                TilePos::new(20 + x, 20 + y),
            )
            .with_unit(
                UnitKind::Soldier,
                PlayerId::Two,
                TilePos::new(20 + x, 100 + y),
            );
    }

    let wall = (0..MAP_SIZE)
        .filter(|x| !(60..64).contains(x))
        .map(|x| TilePos::new(x, MAP_SIZE / 2));
    map.with_blocked(wall)
}

/// A world with every unit walking towards the far corner.
fn marching_world() -> World {
    let mut world = World::new(0xB0A7, crowded_map());
    for player in [PlayerId::One, PlayerId::Two] {
        let units: SmallVec<[UnitId; 32]> = world
            .view()
            .units()
            .filter(|(_, u)| u.owner == player)
            .map(|(id, _)| id)
            .collect();
        let target = match player {
            PlayerId::One => TilePos::new(100, 110),
            PlayerId::Two => TilePos::new(10, 10),
        };
        world.step(&[Command::Move {
            player,
            units,
            target,
        }]);
    }
    // Let the crowd spread out of its starting block, so the benchmark
    // measures steady-state movement rather than the first-tick pile-up.
    for _ in 0..40 {
        world.step(&[]);
    }
    world
}

/// Sends every unit of both sides to a fresh corner.
fn march(world: &mut World, flip: bool) {
    for player in [PlayerId::One, PlayerId::Two] {
        let units: SmallVec<[UnitId; 32]> = world
            .view()
            .units()
            .filter(|(_, u)| u.owner == player)
            .map(|(id, _)| id)
            .collect();
        let target = match (player, flip) {
            (PlayerId::One, false) | (PlayerId::Two, true) => TilePos::new(100, 110),
            (PlayerId::One, true) | (PlayerId::Two, false) => TilePos::new(10, 10),
        };
        world.step(&[Command::Move {
            player,
            units,
            target,
        }]);
    }
}

fn bench_step(c: &mut Criterion) {
    let mut world = marching_world();
    let mut elapsed = 0u64;
    c.bench_function("step 200 units moving", |b| {
        b.iter(|| {
            // Criterion reuses one world across iterations, so without a fresh
            // order every so often the crowd would arrive and the benchmark
            // would quietly start measuring 200 idle units. Both goals stay in
            // the flow field cache, so this measures marching, not rebuilding.
            if elapsed.is_multiple_of(400) {
                march(&mut world, elapsed.is_multiple_of(800));
            }
            elapsed += 1;
            world.step(black_box(&[]));
        });
    });
}

fn bench_flow_field(c: &mut Criterion) {
    let mut terrain = Grid::new(MAP_SIZE, MAP_SIZE, Tile::OPEN);
    for x in 0..MAP_SIZE {
        if !(60..64).contains(&x) {
            terrain.set(TilePos::new(x, MAP_SIZE / 2), Tile::BLOCKED);
        }
    }
    c.bench_function("flow field 128x128", |b| {
        b.iter(|| black_box(FlowField::build(&terrain, TilePos::new(100, 110))));
    });
}

criterion_group!(benches, bench_step, bench_flow_field);
criterion_main!(benches);
