//! Dropping off a load by hand: a loaded villager sent to a drop-off
//! deposits there early, then goes back to the mine it was working.

use sim::{BuildingKind, Command, MapSpec, Order, PlayerId, TilePos, UnitId, UnitKind, World};
use smallvec::smallvec;

/// Player one's town centre and barracks, player two's town centre, a mine,
/// and a villager of player one beside the mine.
fn world() -> (World, UnitId, sim::MineId) {
    let map = MapSpec::new(24, 24)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(BuildingKind::BARRACKS, PlayerId::ONE, TilePos::new(2, 10))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(18, 18),
        )
        .with_mine(TilePos::new(12, 2), 500)
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(11, 3));
    let world = World::new(1, map);
    let (villager, _) = world.view().units().next().expect("the villager");
    let (mine, _) = world.view().mines().next().expect("the mine");
    (world, villager, mine)
}

fn building(world: &World, owner: PlayerId, kind: BuildingKind) -> sim::BuildingId {
    world
        .view()
        .buildings()
        .find(|(_, b)| b.owner == owner && b.kind == kind)
        .map(|(id, _)| id)
        .expect("the building")
}

fn carrying(world: &World, villager: UnitId) -> i32 {
    world.view().unit(villager).expect("the villager").carrying
}

fn gold(world: &World) -> i32 {
    world.view().resources(PlayerId::ONE).gold
}

/// Gathers until the villager carries `load`.
fn mine_until(world: &mut World, villager: UnitId, mine: sim::MineId, load: i32) {
    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: smallvec![villager],
        mine,
    }]);
    for _ in 0..1000 {
        if carrying(world, villager) >= load {
            return;
        }
        world.step(&[]);
    }
    panic!("never carried {load}");
}

fn deposit(world: &mut World, villager: UnitId, at: sim::BuildingId) {
    world.step(&[Command::Deposit {
        player: PlayerId::ONE,
        units: smallvec![villager],
        building: at,
    }]);
}

#[test]
fn a_part_load_dropped_off_by_hand_is_banked_and_the_villager_goes_back_to_mining() {
    let (mut world, villager, mine) = world();
    mine_until(&mut world, villager, mine, 4);
    let town_centre = building(&world, PlayerId::ONE, BuildingKind::TOWN_CENTER);

    deposit(&mut world, villager, town_centre);
    let load = 4;
    for _ in 0..400 {
        if gold(&world) > 0 {
            break;
        }
        world.step(&[]);
    }

    assert_eq!(gold(&world), load, "the part load is banked");
    let mut back_to_mining = false;
    for _ in 0..300 {
        world.step(&[]);
        if matches!(
            world.view().unit(villager).expect("the villager").order,
            Order::Gather {
                phase: sim::GatherPhase::Mining,
                ..
            }
        ) {
            back_to_mining = true;
            break;
        }
    }
    assert!(back_to_mining, "it did not go back to the mine");
}

#[test]
fn an_idle_villager_with_a_load_drops_it_off_and_stands() {
    let (mut world, villager, mine) = world();
    mine_until(&mut world, villager, mine, 3);
    world.step(&[Command::Stop {
        player: PlayerId::ONE,
        units: smallvec![villager],
    }]);
    let load = carrying(&world, villager);
    let town_centre = building(&world, PlayerId::ONE, BuildingKind::TOWN_CENTER);

    deposit(&mut world, villager, town_centre);
    for _ in 0..400 {
        world.step(&[]);
    }

    assert_eq!(gold(&world), load);
    assert_eq!(carrying(&world, villager), 0);
    assert!(matches!(
        world.view().unit(villager).expect("the villager").order,
        Order::Idle
    ));
}

#[test]
fn a_building_that_takes_no_resources_is_no_drop_off() {
    let (mut world, villager, mine) = world();
    mine_until(&mut world, villager, mine, 2);
    let before = world
        .view()
        .unit(villager)
        .expect("the villager")
        .order
        .clone();
    let barracks = building(&world, PlayerId::ONE, BuildingKind::BARRACKS);

    deposit(&mut world, villager, barracks);

    let after = world
        .view()
        .unit(villager)
        .expect("the villager")
        .order
        .clone();
    assert!(
        matches!(
            (&before, &after),
            (Order::Gather { .. }, Order::Gather { .. })
        ),
        "{after:?}"
    );
    assert!(
        !matches!(
            after,
            Order::Gather {
                phase: sim::GatherPhase::ToDropOff { building },
                ..
            } if building == barracks
        ),
        "sent to the barracks"
    );
}

#[test]
fn another_players_town_centre_is_no_drop_off() {
    let (mut world, villager, mine) = world();
    mine_until(&mut world, villager, mine, 2);
    let theirs = building(&world, PlayerId::TWO, BuildingKind::TOWN_CENTER);

    deposit(&mut world, villager, theirs);
    for _ in 0..600 {
        world.step(&[]);
    }

    assert_eq!(world.view().resources(PlayerId::TWO).gold, 0);
}

#[test]
fn a_villager_carrying_nothing_ignores_the_order() {
    let (mut world, villager, _) = world();
    let town_centre = building(&world, PlayerId::ONE, BuildingKind::TOWN_CENTER);

    deposit(&mut world, villager, town_centre);

    assert!(matches!(
        world.view().unit(villager).expect("the villager").order,
        Order::Idle
    ));
}
