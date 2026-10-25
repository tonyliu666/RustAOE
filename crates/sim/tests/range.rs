//! Attack range is measured between edges, as in the original: a melee
//! unit's range is 0, and it hits what it touches.

use sim::{
    BuildingKind, Command, Content, EntityId, Fx, MapSpec, PlayerId, TilePos, UnitId, UnitKind,
    World,
};
use smallvec::smallvec;

/// A map with a town centre for each player far away, so neither has lost,
/// and units placed by the test.
fn map() -> MapSpec {
    MapSpec::new(32, 32)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(0, 0))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(28, 28),
        )
}

/// Soldiers with `range` tiles of reach and the default quarter-tile radius.
fn content(range: Fx) -> Content {
    let mut content = Content::default();
    content.unit_mut(UnitKind::SOLDIER).range = range;
    content
}

fn unit_of(world: &World, owner: PlayerId) -> UnitId {
    world
        .view()
        .units()
        .find(|(_, u)| u.owner == owner)
        .map(|(id, _)| id)
        .expect("the unit")
}

fn attack(world: &mut World, attacker: UnitId, target: EntityId) {
    world.step(&[Command::Attack {
        player: PlayerId::ONE,
        units: smallvec![attacker],
        target,
    }]);
}

fn hp(world: &World, unit: UnitId) -> i32 {
    world.view().unit(unit).map_or(0, |u| u.hp)
}

#[test]
fn a_target_whose_edge_is_within_range_is_hit_at_once() {
    // Centres one tile apart, each a quarter tile in radius: their edges are
    // half a tile apart, which a 3/8 range with the eighth-tile contact
    // tolerance reaches before the attacker takes a step. Leaving out either
    // radius falls short even after the first step's eighth of a tile.
    let map = map()
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(10, 10))
        .with_unit(UnitKind::SOLDIER, PlayerId::TWO, TilePos::new(11, 10));
    let mut world = World::with_content(1, map, content(Fx::from_frac(3, 8)));
    let (attacker, victim) = (
        unit_of(&world, PlayerId::ONE),
        unit_of(&world, PlayerId::TWO),
    );
    let full = hp(&world, victim);

    attack(&mut world, attacker, EntityId::Unit(victim));

    assert!(hp(&world, victim) < full, "not hit on the first tick");
}

#[test]
fn a_target_whose_edge_is_out_of_range_is_not_hit_from_where_the_attacker_stands() {
    // An eighth-tile range, a quarter with the contact tolerance, does not
    // reach an edge half a tile away; the attacker has to walk first.
    let map = map()
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(10, 10))
        .with_unit(UnitKind::SOLDIER, PlayerId::TWO, TilePos::new(11, 10));
    let mut world = World::with_content(1, map, content(Fx::from_frac(1, 8)));
    let (attacker, victim) = (
        unit_of(&world, PlayerId::ONE),
        unit_of(&world, PlayerId::TWO),
    );
    let full = hp(&world, victim);

    attack(&mut world, attacker, EntityId::Unit(victim));

    // One tick of walking, an eighth of a tile, still leaves the edges more
    // than a quarter tile apart.
    assert_eq!(hp(&world, victim), full, "hit from out of range");
}

#[test]
fn a_melee_unit_walks_up_to_its_target_and_hits_it() {
    // Range 0, the original's militia: it must touch, and touching must be
    // reachable despite the two units pushing each other apart.
    let map = map()
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(10, 10))
        .with_unit(UnitKind::SOLDIER, PlayerId::TWO, TilePos::new(14, 10));
    let mut world = World::with_content(1, map, content(Fx::ZERO));
    let (attacker, victim) = (
        unit_of(&world, PlayerId::ONE),
        unit_of(&world, PlayerId::TWO),
    );
    let full = hp(&world, victim);

    attack(&mut world, attacker, EntityId::Unit(victim));
    for _ in 0..200 {
        world.step(&[]);
    }

    assert!(hp(&world, victim) < full, "never landed a hit");
}

#[test]
fn a_building_is_reached_at_its_footprints_edge() {
    // The barracks covers tiles 10 to 12. A unit on tile 13 stands half a
    // tile from its east edge, a quarter tile once its own radius is taken
    // off: a quarter-tile range reaches it. Measured to the centre less half
    // the footprint, it would be half a tile, short.
    let map = map()
        .with_building(BuildingKind::BARRACKS, PlayerId::TWO, TilePos::new(10, 10))
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(13, 11));
    let mut world = World::with_content(1, map, content(Fx::from_frac(1, 4)));
    let attacker = unit_of(&world, PlayerId::ONE);
    let (barracks, building) = world
        .view()
        .buildings()
        .find(|(_, b)| b.kind == BuildingKind::BARRACKS)
        .map(|(id, b)| (id, b.hp))
        .expect("the barracks");

    attack(&mut world, attacker, EntityId::Building(barracks));

    let after = world.view().building(barracks).expect("standing").hp;
    assert!(after < building, "not hit on the first tick");
}

#[test]
fn a_buildings_corner_is_reached_at_the_corner() {
    // Diagonally off the corner, the footprint's nearest point is the corner
    // itself: 0.71 tiles from the unit's centre, 0.46 from its edge, which a
    // half-tile range reaches. Measured to the centre less half the
    // footprint, the corner would be 1.33 tiles away.
    let map = map()
        .with_building(BuildingKind::BARRACKS, PlayerId::TWO, TilePos::new(10, 10))
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(13, 13));
    let mut world = World::with_content(1, map, content(Fx::from_frac(1, 2)));
    let attacker = unit_of(&world, PlayerId::ONE);
    let (barracks, building) = world
        .view()
        .buildings()
        .find(|(_, b)| b.kind == BuildingKind::BARRACKS)
        .map(|(id, b)| (id, b.hp))
        .expect("the barracks");

    attack(&mut world, attacker, EntityId::Building(barracks));

    let after = world.view().building(barracks).expect("standing").hp;
    assert!(after < building, "not hit on the first tick");
}

#[test]
fn a_buildings_corner_out_of_range_is_not_hit() {
    // The same corner, 0.46 tiles from the unit's edge, is beyond an
    // eighth-tile range (a quarter with the contact tolerance) even after a
    // tick of walking.
    let map = map()
        .with_building(BuildingKind::BARRACKS, PlayerId::TWO, TilePos::new(10, 10))
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(13, 13));
    let mut world = World::with_content(1, map, content(Fx::from_frac(1, 8)));
    let attacker = unit_of(&world, PlayerId::ONE);
    let (barracks, building) = world
        .view()
        .buildings()
        .find(|(_, b)| b.kind == BuildingKind::BARRACKS)
        .map(|(id, b)| (id, b.hp))
        .expect("the barracks");

    attack(&mut world, attacker, EntityId::Building(barracks));

    assert_eq!(
        world.view().building(barracks).expect("standing").hp,
        building
    );
}

#[test]
fn the_default_soldier_fights_at_the_militias_range_of_zero() {
    assert_eq!(Content::default().unit(UnitKind::SOLDIER).range, Fx::ZERO);
}

#[test]
fn melee_units_crowding_a_building_all_reach_it() {
    // Five soldiers pressing on one wall push each other a little away from
    // it; each still counts as touching, as the original's do.
    let mut map = map().with_building(BuildingKind::BARRACKS, PlayerId::TWO, TilePos::new(16, 16));
    for i in 0..5 {
        map = map.with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(14 + i, 20));
    }
    let mut world = World::with_content(1, map, Content::default());
    let barracks = world
        .view()
        .buildings()
        .find(|(_, b)| b.kind == BuildingKind::BARRACKS)
        .map(|(id, _)| id)
        .expect("the barracks");
    let attackers: smallvec::SmallVec<[UnitId; 32]> = world
        .view()
        .units()
        .filter(|(_, u)| u.owner == PlayerId::ONE)
        .map(|(id, _)| id)
        .collect();
    world.step(&[Command::Attack {
        player: PlayerId::ONE,
        units: attackers,
        target: EntityId::Building(barracks),
    }]);
    for _ in 0..140 {
        world.step(&[]);
    }
    let before = world.view().building(barracks).expect("standing").hp;
    // One second: every soldier that reaches swings once.
    for _ in 0..20 {
        world.step(&[]);
    }

    let damage = Content::default().unit(UnitKind::SOLDIER).damage;
    let after = world.view().building(barracks).map_or(0, |b| b.hp);
    assert_eq!(before - after, 5 * damage, "not every soldier reached it");
}
