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

/// Food carried after `ticks` of working the first site of `map`, with a
/// forager's rate of `forage` ticks per unit and a farmer's of `farm`.
fn food_carried(map: MapSpec, forage: u32, farm: u32, ticks: u32) -> i32 {
    let mut content = sim::Content::default();
    content.gather.food.ticks_per_unit = forage;
    content.gather.farm.ticks_per_unit = farm;
    let mut world = World::with_content(1, map, content);
    let (site, _) = world.view().sites().next().expect("the site");
    let worker = villager(&world);
    gather(&mut world, worker, site);
    for _ in 0..ticks {
        world.step(&[]);
    }
    world.view().unit(worker).expect("the villager").carrying
}

#[test]
fn a_farm_is_worked_at_the_farmers_rate_and_a_bush_at_the_foragers() {
    // As in the original, where a villager on a farm becomes a farmer
    // (unit 259) and one at a bush a forager (unit 120), each with its own
    // work rate.
    let farm = || {
        base().with_farm(TilePos::new(9, 3), 175).with_unit(
            UnitKind::VILLAGER,
            PlayerId::ONE,
            TilePos::new(8, 4),
        )
    };
    let bush = || {
        base()
            .with_site(Resource::Food, TilePos::new(9, 3), 175)
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 3))
    };

    assert!(
        food_carried(farm(), 1000, 3, 60) > 0,
        "farmed at the forager's rate"
    );
    assert_eq!(
        food_carried(farm(), 3, 1000, 60),
        0,
        "farmed at the forager's rate"
    );
    assert!(
        food_carried(bush(), 3, 1000, 60) > 0,
        "foraged at the farmer's rate"
    );
    assert_eq!(
        food_carried(bush(), 1000, 3, 60),
        0,
        "foraged at the farmer's rate"
    );
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

/// Runs `ticks` empty ticks.
fn run(world: &mut World, ticks: u32) {
    for _ in 0..ticks {
        world.step(&[]);
    }
}

fn site_at(world: &World, at: TilePos) -> Option<sim::SiteId> {
    world
        .view()
        .sites()
        .find(|(_, s)| s.origin == at)
        .map(|(id, _)| id)
}

#[test]
fn when_its_tree_is_cut_down_a_villager_goes_on_to_the_nearest_tree() {
    // As in the original, where a lumberjack moves on to the next tree
    // rather than standing at the stump. The near tree is two tiles from the
    // cut one, the far one eight.
    let map = base()
        .with_site(Resource::Wood, TilePos::new(10, 8), 3)
        .with_site(Resource::Wood, TilePos::new(12, 8), 100)
        .with_site(Resource::Wood, TilePos::new(18, 8), 100)
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(9, 8));
    let mut world = World::new(1, map);
    let first = site_at(&world, TilePos::new(10, 8)).expect("the first tree");
    let near = site_at(&world, TilePos::new(12, 8)).expect("the near tree");
    let far = site_at(&world, TilePos::new(18, 8)).expect("the far tree");
    let worker = villager(&world);

    gather(&mut world, worker, first);
    run(&mut world, 1500);

    assert!(world.view().site(first).is_none(), "the first tree is cut");
    let left = |site| world.view().site(site).expect("standing").remaining;
    assert!(left(near) < 100, "the near tree is not being cut");
    assert_eq!(left(far), 100, "the far tree was cut first");
    assert!(
        matches!(
            world.view().unit(worker).expect("the villager").order,
            sim::Order::Gather { site, .. } if site == near
        ),
        "the villager stopped working"
    );
}

#[test]
fn when_its_mine_runs_out_a_villager_goes_on_to_a_mine_of_the_same_resource() {
    // A tree stands nearer than the other gold mine; the miner still goes to
    // the gold.
    let map = base()
        .with_site(Resource::Gold, TilePos::new(10, 8), 3)
        .with_site(Resource::Wood, TilePos::new(12, 8), 100)
        .with_site(Resource::Gold, TilePos::new(15, 8), 100)
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(9, 8));
    let mut world = World::new(1, map);
    let first = site_at(&world, TilePos::new(10, 8)).expect("the first mine");
    let tree = site_at(&world, TilePos::new(12, 8)).expect("the tree");
    let gold = site_at(&world, TilePos::new(15, 8)).expect("the other mine");
    let worker = villager(&world);

    gather(&mut world, worker, first);
    run(&mut world, 1500);

    let left = |site| world.view().site(site).expect("standing").remaining;
    assert_eq!(left(tree), 100, "the miner cut wood");
    assert!(left(gold) < 100, "the miner did not go on to the gold");
}

#[test]
fn a_villager_whose_tree_another_cut_down_banks_its_load_rather_than_holding_it() {
    // Two villagers on one tree and no other tree: when one takes the last
    // wood, the other is left holding part of a load. It must take it to the
    // drop-off, not stand with it.
    let amount = 15;
    let map = base()
        .with_site(Resource::Wood, TilePos::new(10, 8), amount)
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(9, 8))
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(11, 8));
    let mut world = World::new(1, map);
    let tree = site_at(&world, TilePos::new(10, 8)).expect("the tree");
    let workers: Vec<UnitId> = world.view().units().map(|(id, _)| id).collect();
    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: workers.iter().copied().collect(),
        site: tree,
    }]);

    run(&mut world, 3000);

    for worker in &workers {
        assert_eq!(
            world.view().unit(*worker).expect("alive").carrying,
            0,
            "a villager still holds wood"
        );
    }
    assert_eq!(world.view().resources(PlayerId::ONE).wood, amount);
}

#[test]
fn when_a_farm_runs_out_its_farmer_does_not_go_on_to_other_food() {
    // In the original a used-up farm is reseeded or left; the farmer walks
    // neither to someone else's field nor to a forage bush.
    let map = base()
        .with_farm(TilePos::new(9, 8), 3)
        .with_farm(TilePos::new(13, 8), 100)
        .with_site(Resource::Food, TilePos::new(9, 12), 100)
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 9));
    let mut world = World::new(1, map);
    let first = site_at(&world, TilePos::new(9, 8)).expect("the first farm");
    let other = site_at(&world, TilePos::new(13, 8)).expect("the other farm");
    let bush = site_at(&world, TilePos::new(9, 12)).expect("the bush");
    let worker = villager(&world);

    gather(&mut world, worker, first);
    run(&mut world, 1500);

    let left = |site| world.view().site(site).expect("standing").remaining;
    assert_eq!(left(other), 100, "went on to another farm");
    assert_eq!(left(bush), 100, "went on to a bush");
}

#[test]
fn a_villager_whose_last_load_used_up_its_tree_comes_back_to_the_next_one() {
    // The tree holds exactly one load: the villager leaves to bank it on the
    // tick the tree falls, and comes back to the next tree afterwards.
    let map = base()
        .with_site(Resource::Wood, TilePos::new(10, 8), 10)
        .with_site(Resource::Wood, TilePos::new(12, 8), 100)
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(9, 8));
    let mut world = World::new(1, map);
    let first = site_at(&world, TilePos::new(10, 8)).expect("the first tree");
    let next = site_at(&world, TilePos::new(12, 8)).expect("the next tree");
    let worker = villager(&world);

    gather(&mut world, worker, first);
    run(&mut world, 2000);

    assert!(
        world.view().resources(PlayerId::ONE).wood >= 10,
        "not banked"
    );
    assert!(
        world.view().site(next).expect("standing").remaining < 100,
        "the villager did not come back to the next tree"
    );
}
