//! Sites on the map: a tree or mine blocks its tile until it is used up and
//! disappears; a farm is three tiles a side, does not block, and is worked
//! from on it.

use sim::{BuildingKind, Command, MapSpec, PlayerId, Resource, TilePos, UnitId, UnitKind, World};
use smallvec::smallvec;

fn base() -> MapSpec {
    MapSpec::new(24, 24)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(20, 20),
        )
}

fn villager(world: &World) -> UnitId {
    world
        .view()
        .units()
        .next()
        .map(|(id, _)| id)
        .expect("the villager")
}

fn gather(world: &mut World, villager: UnitId, site: sim::SiteId) {
    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: smallvec![villager],
        site,
    }]);
}

fn passable(world: &World, at: TilePos) -> bool {
    world.view().tile(at).is_some_and(|t| t.passable)
}

#[test]
fn a_tree_blocks_its_tile_until_it_is_cut_down_then_frees_it() {
    let tree = TilePos::new(10, 3);
    let map = base().with_site(Resource::Wood, tree, 3).with_unit(
        UnitKind::VILLAGER,
        PlayerId::ONE,
        TilePos::new(9, 3),
    );
    let mut world = World::new(1, map);
    let (site, _) = world.view().sites().next().expect("the tree");
    assert!(!passable(&world, tree), "a standing tree blocks");

    let worker = villager(&world);
    gather(&mut world, worker, site);
    for _ in 0..600 {
        world.step(&[]);
    }

    assert!(
        world.view().site(site).is_none(),
        "the tree is still standing"
    );
    assert!(passable(&world, tree), "the stump still blocks");
}

#[test]
fn a_site_used_up_disappears() {
    let map = base().with_mine(TilePos::new(10, 3), 2).with_unit(
        UnitKind::VILLAGER,
        PlayerId::ONE,
        TilePos::new(9, 3),
    );
    let mut world = World::new(1, map);
    let (site, _) = world.view().sites().next().expect("the mine");

    let worker = villager(&world);
    gather(&mut world, worker, site);
    for _ in 0..600 {
        world.step(&[]);
    }

    assert_eq!(world.view().sites().count(), 0);
    assert_eq!(world.view().resources(PlayerId::ONE).gold, 2, "both banked");
}

#[test]
fn a_farm_is_three_tiles_a_side_and_blocks_none_of_them() {
    let map = base().with_farm(TilePos::new(10, 3), 175);
    let world = World::new(1, map);

    let (_, farm) = world.view().sites().next().expect("the farm");

    assert_eq!(
        (farm.resource, farm.size, farm.remaining),
        (Resource::Food, 3, 175)
    );
    assert_eq!(farm.footprint().count(), 9);
    for tile in farm.footprint() {
        assert!(passable(&world, tile), "{tile:?} blocked");
    }
}

#[test]
fn a_villager_farms_food_from_a_farm() {
    let map = base().with_farm(TilePos::new(9, 3), 175).with_unit(
        UnitKind::VILLAGER,
        PlayerId::ONE,
        TilePos::new(8, 4),
    );
    let mut world = World::new(1, map);
    let (farm, _) = world.view().sites().next().expect("the farm");

    let worker = villager(&world);
    gather(&mut world, worker, farm);
    for _ in 0..2000 {
        world.step(&[]);
    }

    assert!(world.view().resources(PlayerId::ONE).food > 0);
}

#[test]
fn a_unit_walks_across_a_farm() {
    // From one side of a farm to the other, straight over it.
    let map = base().with_farm(TilePos::new(9, 3), 175).with_unit(
        UnitKind::SOLDIER,
        PlayerId::ONE,
        TilePos::new(8, 4),
    );
    let mut world = World::new(1, map);
    let soldier = villager(&world);

    world.step(&[Command::Move {
        player: PlayerId::ONE,
        units: smallvec![soldier],
        target: TilePos::new(13, 4),
    }]);
    for _ in 0..200 {
        world.step(&[]);
    }

    // The farm covers x 9 to 11; the soldier ends past its far side (a move
    // counts as arrived a little short of the goal tile's centre).
    let at = world.view().unit(soldier).expect("the soldier").pos.tile();
    assert!(at.x >= 12, "stopped at {at:?}");
}

/// Player one's base, a villager at (4, 10), and a block of trees three by
/// three at (10, 9): the middle one is walled in by the other eight.
fn walled_in_tree() -> (World, UnitId, sim::SiteId) {
    let mut map = base().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(4, 10));
    for y in 9..12 {
        for x in 10..13 {
            map = map.with_site(Resource::Wood, TilePos::new(x, y), 100);
        }
    }
    let world = World::new(1, map);
    let (middle, _) = world
        .view()
        .sites()
        .find(|(_, s)| s.origin == TilePos::new(11, 10))
        .expect("the middle tree");
    let worker = villager(&world);
    (world, worker, middle)
}

#[test]
fn sent_to_a_tree_it_cannot_reach_a_villager_cuts_the_nearest_it_can() {
    // As in the original: clicking into a woodline sends the villager to its
    // edge.
    let (mut world, worker, middle) = walled_in_tree();

    gather(&mut world, worker, middle);
    for _ in 0..1500 {
        world.step(&[]);
    }

    let unit = world.view().unit(worker).expect("the villager");
    assert!(
        unit.carrying > 0 || world.view().resources(PlayerId::ONE).wood > 0,
        "no wood cut: {:?}",
        unit.order
    );
    assert_eq!(
        world.view().site(middle).map(|s| s.remaining),
        Some(100),
        "the walled-in tree was cut"
    );
}

#[test]
fn the_nearest_reachable_tree_is_the_one_nearest_the_tree_clicked() {
    let (mut world, worker, middle) = walled_in_tree();

    gather(&mut world, worker, middle);

    let sim::Order::Gather { site, .. } = world.view().unit(worker).expect("the villager").order
    else {
        panic!("not gathering");
    };
    let chosen = world.view().site(site).expect("a tree").origin;
    let one_tile_away = (chosen.x - 11).abs() + (chosen.y - 10).abs() == 1;
    assert!(one_tile_away, "went for the tree at {chosen:?}");
}

#[test]
fn sent_to_a_site_it_cannot_reach_with_none_like_it_reachable_a_villager_stands() {
    // Gold walled in by trees, and no other gold: nothing to retarget to.
    let mut map = base().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(4, 10));
    for y in 9..12 {
        for x in 10..13 {
            map = if (x, y) == (11, 10) {
                map.with_mine(TilePos::new(x, y), 100)
            } else {
                map.with_site(Resource::Wood, TilePos::new(x, y), 100)
            };
        }
    }
    let mut world = World::new(1, map);
    let (gold, _) = world
        .view()
        .sites()
        .find(|(_, s)| s.resource == Resource::Gold)
        .expect("the gold");
    let worker = villager(&world);
    // Busy first, so that standing is the new order's doing.
    let (tree, _) = world
        .view()
        .sites()
        .find(|(_, s)| s.resource == Resource::Wood)
        .expect("a tree");
    gather(&mut world, worker, tree);

    gather(&mut world, worker, gold);
    world.step(&[]);

    assert!(matches!(
        world.view().unit(worker).expect("the villager").order,
        sim::Order::Idle
    ));
}
