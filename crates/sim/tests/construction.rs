//! Building placement and construction.

use sim::{BuildingKind, Command, Content, MapSpec, PlayerId, Resources, TilePos, UnitKind, World};

const SEED: u64 = 0xA6E0;

fn base_map() -> MapSpec {
    MapSpec::new(32, 32)
        .with_starting_resources(Resources {
            food: 200,
            gold: 200,
            wood: 200,
            stone: 200,
        })
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(4, 4))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(24, 24),
        )
}

fn make_world(map: MapSpec) -> World {
    World::new(SEED, map)
}

fn tc(world: &World, player: PlayerId) -> sim::BuildingId {
    world
        .view()
        .buildings()
        .find(|(_, b)| b.owner == player && b.kind == BuildingKind::TOWN_CENTER)
        .map(|(id, _)| id)
        .expect("town center")
}

fn has_foundation(world: &World) -> bool {
    world.view().foundations().next().is_some()
}

fn foundation_count(world: &World) -> usize {
    world.view().foundations().count()
}

#[test]
fn a_valid_building_placement_creates_a_foundation() {
    let mut world =
        make_world(base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 8)));
    let tc_id = tc(&world, PlayerId::ONE);

    world.step(&[Command::PlaceBuilding {
        player: PlayerId::ONE,
        building: tc_id,
        kind: BuildingKind::HOUSE,
        origin: TilePos::new(8, 6),
    }]);

    // Should create a foundation
    assert!(has_foundation(&world));
    assert_eq!(foundation_count(&world), 1);
    let (_, foundation) = world.view().foundations().next().expect("foundation");
    assert_eq!(foundation.kind, BuildingKind::HOUSE);
    assert_eq!(foundation.origin, TilePos::new(8, 6));
    assert_eq!(foundation.hp, 0);
    assert_eq!(foundation.max_hp, 900); // the house, from Content::default()
}

#[test]
fn placement_off_map_is_rejected() {
    let mut world =
        make_world(base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 8)));
    let tc_id = tc(&world, PlayerId::ONE);

    world.step(&[Command::PlaceBuilding {
        player: PlayerId::ONE,
        building: tc_id,
        kind: BuildingKind::HOUSE,
        origin: TilePos::new(32, 32), // off map
    }]);

    assert!(!has_foundation(&world));
}

#[test]
fn placement_on_impassable_tile_is_rejected() {
    let mut world = make_world(
        base_map()
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 8))
            .with_blocked([
                TilePos::new(8, 6),
                TilePos::new(9, 6),
                TilePos::new(8, 7),
                TilePos::new(9, 7),
            ]),
    );
    let tc_id = tc(&world, PlayerId::ONE);

    world.step(&[Command::PlaceBuilding {
        player: PlayerId::ONE,
        building: tc_id,
        kind: BuildingKind::HOUSE,
        origin: TilePos::new(8, 6),
    }]);

    assert!(!has_foundation(&world));
}

#[test]
fn placement_on_another_building_is_rejected() {
    let mut world = make_world(
        base_map()
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 8))
            .with_building(BuildingKind::BARRACKS, PlayerId::ONE, TilePos::new(8, 6)),
    );
    let tc_id = tc(&world, PlayerId::ONE);

    world.step(&[Command::PlaceBuilding {
        player: PlayerId::ONE,
        building: tc_id,
        kind: BuildingKind::HOUSE,
        origin: TilePos::new(8, 6),
    }]);

    assert!(!has_foundation(&world));
}

#[test]
fn placement_on_resource_site_is_rejected() {
    let mut world = make_world(
        base_map()
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 8))
            .with_site(sim::Resource::Gold, TilePos::new(8, 6), 800),
    );
    let tc_id = tc(&world, PlayerId::ONE);

    world.step(&[Command::PlaceBuilding {
        player: PlayerId::ONE,
        building: tc_id,
        kind: BuildingKind::HOUSE,
        origin: TilePos::new(8, 6),
    }]);

    assert!(!has_foundation(&world));
}

#[test]
fn placement_cost_is_paid_from_player_resources() {
    let mut world =
        make_world(base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 8)));
    let tc_id = tc(&world, PlayerId::ONE);
    let content = Content::default();
    let house_cost = content.building(BuildingKind::HOUSE).cost.wood; // house costs 25 wood

    let before = world.view().resources(PlayerId::ONE);
    world.step(&[Command::PlaceBuilding {
        player: PlayerId::ONE,
        building: tc_id,
        kind: BuildingKind::HOUSE,
        origin: TilePos::new(8, 6),
    }]);
    let after = world.view().resources(PlayerId::ONE);

    // Cost should be paid
    assert_eq!(before.wood - after.wood, house_cost);
}

#[test]
fn the_view_says_where_a_building_may_be_placed_as_the_order_judges_it() {
    let world = make_world(base_map().with_blocked([TilePos::new(20, 6)]).with_site(
        sim::Resource::Gold,
        TilePos::new(14, 6),
        800,
    ));
    let view = world.view();
    let house = BuildingKind::HOUSE;

    assert!(view.can_place(house, TilePos::new(8, 6)), "open ground");
    assert!(!view.can_place(house, TilePos::new(31, 8)), "off the map");
    assert!(!view.can_place(house, TilePos::new(-1, 8)), "off the map");
    assert!(!view.can_place(house, TilePos::new(19, 5)), "impassable");
    assert!(
        !view.can_place(house, TilePos::new(3, 3)),
        "the town centre"
    );
    assert!(!view.can_place(house, TilePos::new(13, 5)), "a gold mine");
}
