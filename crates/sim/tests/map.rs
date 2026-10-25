//! The built-in map: each base has every resource and the buildings that
//! take it, and the forest across the middle is trees that can be cut.

use sim::{BuildingKind, Content, MapSpec, PlayerId, Resource, Site, TilePos, World};
use std::collections::HashSet;

fn world() -> World {
    let content = Content::default();
    World::new(
        1,
        MapSpec::proof_of_concept(&content).expect("the map builds"),
    )
}

fn home(world: &World, player: PlayerId) -> TilePos {
    world
        .view()
        .buildings()
        .find(|(_, b)| b.owner == player && b.kind == BuildingKind::TOWN_CENTER)
        .map(|(_, b)| b.origin)
        .expect("a town centre")
}

/// Sites of `resource` and `size` within 16 tiles of `at`.
fn sites_near(world: &World, at: TilePos, resource: Resource, size: i32) -> usize {
    world
        .view()
        .sites()
        .filter(|(_, s)| s.resource == resource && s.size == size)
        .filter(|(_, s)| (s.origin.x - at.x).abs() <= 16 && (s.origin.y - at.y).abs() <= 16)
        .count()
}

#[test]
fn each_base_has_every_drop_off_building() {
    let world = world();
    let content = Content::default();

    for player in [PlayerId::ONE, PlayerId::TWO] {
        let names: HashSet<&str> = world
            .view()
            .buildings()
            .filter(|(_, b)| b.owner == player)
            .map(|(_, b)| content.building(b.kind).name.as_str())
            .collect();
        for name in [
            "town_center",
            "barracks",
            "mill",
            "lumber_camp",
            "mining_camp",
        ] {
            assert!(names.contains(name), "{player:?} has no {name}");
        }
    }
}

#[test]
fn each_base_has_gold_stone_bushes_farms_and_trees() {
    let world = world();

    for player in [PlayerId::ONE, PlayerId::TWO] {
        let at = home(&world, player);
        assert_eq!(
            sites_near(&world, at, Resource::Gold, 1),
            7,
            "{player:?} gold"
        );
        assert_eq!(
            sites_near(&world, at, Resource::Stone, 1),
            5,
            "{player:?} stone"
        );
        assert_eq!(
            sites_near(&world, at, Resource::Food, 1),
            6,
            "{player:?} bushes"
        );
        assert_eq!(
            sites_near(&world, at, Resource::Food, Site::FARM_SIZE),
            2,
            "{player:?} farms"
        );
        assert!(
            sites_near(&world, at, Resource::Wood, 1) >= 20,
            "{player:?} woodline"
        );
    }
}

#[test]
fn the_sites_hold_the_originals_amounts() {
    let world = world();

    for (_, site) in world.view().sites() {
        let expected = match (site.resource, site.size) {
            (Resource::Gold, _) => 800,
            (Resource::Stone, _) => 350,
            (Resource::Food, 1) => 125,
            (Resource::Food, _) => Site::FARM_FOOD,
            (Resource::Wood, _) => 100,
        };
        assert_eq!(site.remaining, expected, "{site:?}");
    }
}

#[test]
fn the_forest_across_the_middle_is_trees_with_two_gaps() {
    let world = world();
    let width = world.view().width();
    let middle = world.view().height() / 2;

    let trees: HashSet<TilePos> = world
        .view()
        .sites()
        .filter(|(_, s)| s.resource == Resource::Wood)
        .map(|(_, s)| s.origin)
        .collect();
    // Three rows deep.
    for x in 0..width {
        let wooded = (middle - 1..=middle + 1).all(|y| trees.contains(&TilePos::new(x, y)));
        let open = (middle - 1..=middle + 1).all(|y| !trees.contains(&TilePos::new(x, y)));
        assert!(wooded || open, "column {x} is half wooded");
    }
    let gaps = (0..width)
        .filter(|x| !trees.contains(&TilePos::new(*x, middle)))
        .count();
    assert_eq!(gaps, 16, "two gaps of eight");
    assert!(world
        .view()
        .tile(TilePos::new(42, middle))
        .is_some_and(|t| t.passable));
}

#[test]
fn nothing_on_the_map_overlaps_and_everything_is_on_it() {
    let world = world();
    let content = Content::default();
    let mut taken: HashSet<TilePos> = HashSet::new();
    let (w, h) = (world.view().width(), world.view().height());
    let mut claim = |tile: TilePos, what: &str| {
        assert!(
            tile.x >= 0 && tile.y >= 0 && tile.x < w && tile.y < h,
            "{what} off the map at {tile:?}"
        );
        assert!(taken.insert(tile), "{what} overlaps at {tile:?}");
    };

    for (_, b) in world.view().buildings() {
        for tile in b.footprint() {
            claim(tile, &content.building(b.kind).name);
        }
    }
    for (_, s) in world.view().sites() {
        for tile in s.footprint() {
            claim(tile, &format!("{:?} site", s.resource));
        }
    }
}

#[test]
fn the_map_needs_the_drop_off_types_in_the_content() {
    let mut content = Content::default();
    content.buildings.retain(|b| b.name != "mill");

    let error = MapSpec::proof_of_concept(&content).expect_err("no mill");

    assert!(error.to_string().contains("mill"), "{error}");
}
