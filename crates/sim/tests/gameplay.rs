//! The game loop end to end: move, gather, train, fight, win.
//!
//! Every test builds a small world, steps it, and asserts. The simulation is a
//! pure function of state and commands, so no mocks and no fixtures are needed.

use sim::{
    BuildingId, BuildingKind, Command, Content, EntityId, MapSpec, MineId, Outcome, PlayerId,
    TilePos, UnitId, UnitKind, World,
};
use smallvec::smallvec;

const SEED: u64 = 0xA6E0;

/// A 32x32 map with one town centre and one barracks for player one.
fn base_map() -> MapSpec {
    MapSpec::new(32, 32)
        .with_starting_gold(200)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(4, 4))
        .with_building(BuildingKind::BARRACKS, PlayerId::ONE, TilePos::new(12, 4))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(24, 24),
        )
}

fn world(map: MapSpec) -> World {
    World::new(SEED, map)
}

/// The one unit on the map. Panics if there is more than one.
fn only_unit(world: &World) -> UnitId {
    let mut units = world.view().units();
    let (id, _) = units.next().expect("expected a unit on the map");
    assert!(units.next().is_none(), "expected exactly one unit");
    id
}

fn first_building(world: &World, owner: PlayerId, kind: BuildingKind) -> BuildingId {
    world
        .view()
        .buildings()
        .find(|(_, b)| b.owner == owner && b.kind == kind)
        .map(|(id, _)| id)
        .expect("expected the building to be on the map")
}

fn first_mine(world: &World) -> MineId {
    world
        .view()
        .mines()
        .next()
        .map(|(id, _)| id)
        .expect("expected a mine on the map")
}

fn units_of(world: &World, owner: PlayerId) -> Vec<UnitId> {
    world
        .view()
        .units()
        .filter(|(_, u)| u.owner == owner)
        .map(|(id, _)| id)
        .collect()
}

fn run(world: &mut World, ticks: u64) {
    for _ in 0..ticks {
        world.step(&[]);
    }
}

// --- movement -------------------------------------------------------------

#[test]
fn a_lone_unit_walks_to_the_tile_it_was_ordered_to() {
    let mut world =
        world(base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(10, 10)));
    let unit = only_unit(&world);
    let goal = TilePos::new(16, 10);

    world.step(&[Command::Move {
        player: PlayerId::ONE,
        units: smallvec![unit],
        target: goal,
    }]);
    run(&mut world, 200);

    let arrived = world
        .view()
        .unit(unit)
        .expect("the villager is alive")
        .tile();
    assert!(
        (arrived.x - goal.x).abs() <= 1 && (arrived.y - goal.y).abs() <= 1,
        "walked to {arrived:?} instead of {goal:?}"
    );
}

#[test]
fn a_group_walks_around_a_wall_to_reach_the_far_side() {
    let wall = (0..30).filter(|y| *y != 29).map(|y| TilePos::new(16, y));
    let mut map = base_map().with_blocked(wall);
    for i in 0..4 {
        map = map.with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(10 + i, 10));
    }
    let mut world = world(map);
    let group = units_of(&world, PlayerId::ONE);
    let goal = TilePos::new(20, 10);

    world.step(&[Command::Move {
        player: PlayerId::ONE,
        units: group.iter().copied().collect(),
        target: goal,
    }]);
    run(&mut world, 900);

    for id in group {
        let unit = world.view().unit(id).expect("soldiers do not die here");
        assert!(
            unit.tile().x > 16,
            "a soldier stopped at {:?}, on the near side of the wall",
            unit.tile()
        );
    }
}

#[test]
fn a_unit_never_ends_a_tick_standing_in_a_blocked_tile() {
    // Ordered straight at a building, which is solid: the unit has to stop
    // outside it rather than walk in.
    let mut world =
        world(base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(10, 6)));
    let unit = only_unit(&world);

    world.step(&[Command::Move {
        player: PlayerId::ONE,
        units: smallvec![unit],
        target: TilePos::new(10, 10),
    }]);
    for _ in 0..300 {
        world.step(&[]);
        let tile = world.view().unit(unit).expect("alive").tile();
        assert!(
            world.view().tile(tile).is_some_and(|t| t.passable),
            "the villager is standing in the blocked tile {tile:?}"
        );
    }
}

#[test]
fn a_unit_ordered_into_a_wall_gives_up_rather_than_grinding_forever() {
    let mut world =
        world(base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(10, 10)));
    let unit = only_unit(&world);

    // The centre of the town centre is solid, so there is no path to it.
    world.step(&[Command::Move {
        player: PlayerId::ONE,
        units: smallvec![unit],
        target: TilePos::new(5, 5),
    }]);
    run(&mut world, 100);

    assert_eq!(
        world.view().unit(unit).expect("alive").tile(),
        TilePos::new(10, 10),
        "the villager moved towards a goal it cannot reach"
    );
}

// --- command validation ---------------------------------------------------

#[test]
fn an_order_to_someone_elses_unit_is_dropped() {
    let mut world =
        world(base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(10, 10)));
    let unit = only_unit(&world);

    world.step(&[Command::Move {
        player: PlayerId::TWO,
        units: smallvec![unit],
        target: TilePos::new(16, 10),
    }]);
    run(&mut world, 100);

    assert_eq!(
        world.view().unit(unit).expect("alive").tile(),
        TilePos::new(10, 10)
    );
}

#[test]
fn an_order_to_a_tile_off_the_map_is_dropped() {
    let mut world =
        world(base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(10, 10)));
    let unit = only_unit(&world);

    world.step(&[Command::Move {
        player: PlayerId::ONE,
        units: smallvec![unit],
        target: TilePos::new(-5, 400),
    }]);
    run(&mut world, 50);

    assert_eq!(
        world.view().unit(unit).expect("alive").tile(),
        TilePos::new(10, 10)
    );
}

#[test]
fn a_villager_cannot_be_ordered_to_attack() {
    let map = base_map()
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(24, 22))
        .with_unit(UnitKind::VILLAGER, PlayerId::TWO, TilePos::new(24, 21));
    let mut world = world(map);
    let attacker = units_of(&world, PlayerId::ONE)[0];
    let victim = units_of(&world, PlayerId::TWO)[0];

    world.step(&[Command::Attack {
        player: PlayerId::ONE,
        units: smallvec![attacker],
        target: EntityId::Unit(victim),
    }]);
    run(&mut world, 100);

    assert_eq!(
        world.view().unit(victim).map(|u| u.hp),
        Some(Content::default().unit(UnitKind::VILLAGER).max_hp)
    );
}

#[test]
fn a_unit_cannot_be_ordered_to_attack_its_own_side() {
    let map = base_map()
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(20, 20))
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(20, 21));
    let mut world = world(map);
    let soldier = world
        .view()
        .units()
        .find(|(_, u)| u.kind == UnitKind::SOLDIER)
        .map(|(id, _)| id)
        .expect("the soldier is on the map");
    let friend = world
        .view()
        .units()
        .find(|(_, u)| u.kind == UnitKind::VILLAGER)
        .map(|(id, _)| id)
        .expect("the villager is on the map");

    world.step(&[Command::Attack {
        player: PlayerId::ONE,
        units: smallvec![soldier],
        target: EntityId::Unit(friend),
    }]);
    run(&mut world, 60);

    assert_eq!(
        world.view().unit(friend).map(|u| u.hp),
        Some(Content::default().unit(UnitKind::VILLAGER).max_hp)
    );
}

#[test]
fn stop_cancels_an_order_in_progress() {
    let mut world =
        world(base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(10, 10)));
    let unit = only_unit(&world);

    world.step(&[Command::Move {
        player: PlayerId::ONE,
        units: smallvec![unit],
        target: TilePos::new(20, 10),
    }]);
    run(&mut world, 10);
    let stopped_at = world.view().unit(unit).expect("alive").pos;
    world.step(&[Command::Stop {
        player: PlayerId::ONE,
        units: smallvec![unit],
    }]);
    run(&mut world, 100);

    assert_eq!(world.view().unit(unit).expect("alive").pos, stopped_at);
}

// --- gathering ------------------------------------------------------------

/// A villager, a mine, and a drop-off within a short walk of each other.
fn gather_map() -> MapSpec {
    base_map().with_mine(TilePos::new(10, 4), 5000).with_unit(
        UnitKind::VILLAGER,
        PlayerId::ONE,
        TilePos::new(9, 8),
    )
}

#[test]
fn a_villager_mines_gold_and_banks_it() {
    let mut world = world(gather_map());
    let villager = only_unit(&world);
    let mine = first_mine(&world);
    let before = world.view().gold(PlayerId::ONE);

    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: smallvec![villager],
        mine,
    }]);
    // Long enough to walk over, fill a ten-gold load, and walk back.
    run(&mut world, 700);

    assert!(
        world.view().gold(PlayerId::ONE) >= before + 10,
        "banked {} gold, expected a full load",
        world.view().gold(PlayerId::ONE) - before
    );
}

#[test]
fn mined_gold_leaves_the_seam_it_came_from() {
    let mut world = world(gather_map());
    let villager = only_unit(&world);
    let mine = first_mine(&world);

    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: smallvec![villager],
        mine,
    }]);
    run(&mut world, 700);

    let remaining = world
        .view()
        .mine(mine)
        .expect("the seam is not empty")
        .remaining;
    assert!(remaining < 5000, "the mine still holds all of its gold");
}

#[test]
fn gold_is_conserved_across_a_gathering_session() {
    let mut world = world(gather_map());
    let villager = only_unit(&world);
    let mine = first_mine(&world);
    let total = world.view().total_gold();

    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: smallvec![villager],
        mine,
    }]);
    for _ in 0..1500 {
        world.step(&[]);
        assert_eq!(
            world.view().total_gold(),
            total,
            "gold appeared or vanished at tick {}",
            world.tick()
        );
    }
}

#[test]
fn a_gather_order_against_a_depleted_seam_is_dropped() {
    let map = base_map().with_mine(TilePos::new(10, 4), 0).with_unit(
        UnitKind::VILLAGER,
        PlayerId::ONE,
        TilePos::new(9, 8),
    );
    let mut world = world(map);
    let villager = only_unit(&world);
    let mine = first_mine(&world);

    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: smallvec![villager],
        mine,
    }]);
    run(&mut world, 60);

    assert_eq!(
        world.view().unit(villager).expect("alive").tile(),
        TilePos::new(9, 8)
    );
}

// --- training -------------------------------------------------------------

#[test]
fn training_a_villager_spends_gold_and_produces_a_unit() {
    let mut world = world(base_map());
    let town_center = first_building(&world, PlayerId::ONE, BuildingKind::TOWN_CENTER);
    let before = world.view().gold(PlayerId::ONE);

    world.step(&[Command::Train {
        player: PlayerId::ONE,
        building: town_center,
        unit: UnitKind::VILLAGER,
    }]);
    assert_eq!(
        world.view().gold(PlayerId::ONE),
        before - Content::default().unit(UnitKind::VILLAGER).cost.gold,
        "gold is charged when the unit is queued, not when it pops"
    );

    run(
        &mut world,
        Content::default().unit(UnitKind::VILLAGER).train_ticks as u64 + 1,
    );

    assert_eq!(units_of(&world, PlayerId::ONE).len(), 1);
}

#[test]
fn training_is_refused_when_the_player_cannot_afford_it() {
    let mut world = world(base_map().with_starting_gold(10));
    let town_center = first_building(&world, PlayerId::ONE, BuildingKind::TOWN_CENTER);

    world.step(&[Command::Train {
        player: PlayerId::ONE,
        building: town_center,
        unit: UnitKind::VILLAGER,
    }]);
    run(&mut world, 200);

    assert_eq!(world.view().gold(PlayerId::ONE), 10);
    assert!(units_of(&world, PlayerId::ONE).is_empty());
}

#[test]
fn a_town_center_cannot_train_soldiers() {
    let mut world = world(base_map());
    let town_center = first_building(&world, PlayerId::ONE, BuildingKind::TOWN_CENTER);

    world.step(&[Command::Train {
        player: PlayerId::ONE,
        building: town_center,
        unit: UnitKind::SOLDIER,
    }]);
    run(&mut world, 200);

    assert_eq!(world.view().gold(PlayerId::ONE), 200);
    assert!(units_of(&world, PlayerId::ONE).is_empty());
}

#[test]
fn a_barracks_trains_soldiers() {
    let mut world = world(base_map());
    let barracks = first_building(&world, PlayerId::ONE, BuildingKind::BARRACKS);

    world.step(&[Command::Train {
        player: PlayerId::ONE,
        building: barracks,
        unit: UnitKind::SOLDIER,
    }]);
    run(
        &mut world,
        Content::default().unit(UnitKind::SOLDIER).train_ticks as u64 + 1,
    );

    let trained = units_of(&world, PlayerId::ONE);
    assert_eq!(trained.len(), 1);
    assert_eq!(
        world.view().unit(trained[0]).expect("alive").kind,
        UnitKind::SOLDIER
    );
}

#[test]
fn a_queue_of_two_units_costs_two_units_worth_of_gold() {
    let mut world = world(base_map());
    let town_center = first_building(&world, PlayerId::ONE, BuildingKind::TOWN_CENTER);
    let order = Command::Train {
        player: PlayerId::ONE,
        building: town_center,
        unit: UnitKind::VILLAGER,
    };

    world.step(&[order.clone(), order]);

    assert_eq!(
        world.view().gold(PlayerId::ONE),
        200 - 2 * Content::default().unit(UnitKind::VILLAGER).cost.gold
    );
}

#[test]
fn training_a_building_you_do_not_own_is_dropped() {
    let mut world = world(base_map());
    let town_center = first_building(&world, PlayerId::ONE, BuildingKind::TOWN_CENTER);

    world.step(&[Command::Train {
        player: PlayerId::TWO,
        building: town_center,
        unit: UnitKind::VILLAGER,
    }]);

    assert_eq!(world.view().gold(PlayerId::TWO), 200);
    assert_eq!(world.view().gold(PlayerId::ONE), 200);
}

// --- combat ---------------------------------------------------------------

#[test]
fn a_soldier_kills_an_enemy_villager_in_range() {
    let map = base_map()
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(20, 20))
        .with_unit(UnitKind::VILLAGER, PlayerId::TWO, TilePos::new(20, 21));
    let mut world = world(map);
    let soldier = units_of(&world, PlayerId::ONE)[0];
    let victim = units_of(&world, PlayerId::TWO)[0];

    world.step(&[Command::Attack {
        player: PlayerId::ONE,
        units: smallvec![soldier],
        target: EntityId::Unit(victim),
    }]);
    // Forty hit points at eight damage a second is five swings.
    run(
        &mut world,
        5 * Content::default().unit(UnitKind::SOLDIER).attack_cooldown as u64,
    );

    assert!(
        world.view().unit(victim).is_none(),
        "the villager survived five swings"
    );
}

#[test]
fn a_soldier_walks_into_range_before_it_can_hit() {
    let map = base_map()
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(20, 12))
        .with_unit(UnitKind::VILLAGER, PlayerId::TWO, TilePos::new(20, 20));
    let mut world = world(map);
    let soldier = units_of(&world, PlayerId::ONE)[0];
    let victim = units_of(&world, PlayerId::TWO)[0];

    world.step(&[Command::Attack {
        player: PlayerId::ONE,
        units: smallvec![soldier],
        target: EntityId::Unit(victim),
    }]);
    run(&mut world, 200);

    assert!(
        world.view().unit(victim).is_none(),
        "the victim was not chased down"
    );
}

#[test]
fn an_attacker_stops_when_its_target_is_already_dead() {
    let map = base_map()
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(20, 20))
        .with_unit(UnitKind::VILLAGER, PlayerId::TWO, TilePos::new(20, 21));
    let mut world = world(map);
    let soldier = units_of(&world, PlayerId::ONE)[0];
    let victim = units_of(&world, PlayerId::TWO)[0];

    world.step(&[Command::Attack {
        player: PlayerId::ONE,
        units: smallvec![soldier],
        target: EntityId::Unit(victim),
    }]);
    run(&mut world, 300);

    // Nothing panicked, and the soldier is still standing where it finished.
    assert!(world.view().unit(soldier).is_some());
}

// --- win condition --------------------------------------------------------

#[test]
fn no_outcome_while_both_sides_still_have_buildings() {
    let mut world = world(base_map());
    run(&mut world, 50);

    assert_eq!(world.outcome(), None);
}

#[test]
fn losing_every_building_loses_the_game() {
    // Player two starts with nothing standing, so the check fires on tick one.
    let map = MapSpec::new(32, 32).with_building(
        BuildingKind::TOWN_CENTER,
        PlayerId::ONE,
        TilePos::new(4, 4),
    );
    let mut world = world(map);

    world.step(&[]);

    assert_eq!(world.outcome(), Some(Outcome::Won(PlayerId::ONE)));
}

#[test]
fn razing_the_last_enemy_building_wins_the_game() {
    let mut map = MapSpec::new(32, 32)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(4, 4))
        .with_building(BuildingKind::BARRACKS, PlayerId::TWO, TilePos::new(16, 16));
    for i in 0..5 {
        map = map.with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(14 + i, 20));
    }
    let mut world = world(map);
    let barracks = first_building(&world, PlayerId::TWO, BuildingKind::BARRACKS);
    let attackers = units_of(&world, PlayerId::ONE);

    world.step(&[Command::Attack {
        player: PlayerId::ONE,
        units: attackers.into_iter().collect(),
        target: EntityId::Building(barracks),
    }]);
    // 600 hit points, five soldiers at eight damage a second.
    run(&mut world, 500);

    assert_eq!(world.outcome(), Some(Outcome::Won(PlayerId::ONE)));
    assert!(world.view().building(barracks).is_none());
}

#[test]
fn the_outcome_is_recorded_once_and_never_changes() {
    let map = MapSpec::new(32, 32).with_building(
        BuildingKind::TOWN_CENTER,
        PlayerId::ONE,
        TilePos::new(4, 4),
    );
    let mut world = world(map);

    world.step(&[]);
    let decided = world.outcome();
    run(&mut world, 100);

    assert_eq!(world.outcome(), decided);
}

#[test]
fn a_razed_building_stops_blocking_the_tiles_it_stood_on() {
    let mut map = MapSpec::new(32, 32)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(4, 4))
        .with_building(BuildingKind::BARRACKS, PlayerId::TWO, TilePos::new(16, 16));
    for i in 0..5 {
        map = map.with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(14 + i, 20));
    }
    let mut world = world(map);
    let barracks = first_building(&world, PlayerId::TWO, BuildingKind::BARRACKS);
    let attackers = units_of(&world, PlayerId::ONE);
    assert!(
        !world
            .view()
            .tile(TilePos::new(17, 17))
            .expect("on the map")
            .passable
    );

    world.step(&[Command::Attack {
        player: PlayerId::ONE,
        units: attackers.into_iter().collect(),
        target: EntityId::Building(barracks),
    }]);
    run(&mut world, 500);

    assert!(
        world
            .view()
            .tile(TilePos::new(17, 17))
            .expect("on the map")
            .passable
    );
}
