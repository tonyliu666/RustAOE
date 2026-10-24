//! Content tables: the numbers the game plays by come from `Content`, not
//! from code.
//!
//! Each test changes one number away from the default and shows the game
//! plays differently. A test that still passed with the default numbers would
//! prove nothing about where the number comes from.

use sim::{
    BuildingId, BuildingKind, Command, Content, EntityId, Fx, MapSpec, PlayerId, Replay, TilePos,
    UnitId, UnitKind, World,
};
use smallvec::smallvec;

const SEED: u64 = 0xC0DE;

/// One town centre and one barracks for player one, one town centre for
/// player two so that nobody has already lost.
fn base_map() -> MapSpec {
    MapSpec::new(32, 32)
        .with_starting_gold(200)
        .with_building(BuildingKind::TownCenter, PlayerId::One, TilePos::new(4, 4))
        .with_building(BuildingKind::Barracks, PlayerId::One, TilePos::new(12, 4))
        .with_building(
            BuildingKind::TownCenter,
            PlayerId::Two,
            TilePos::new(24, 24),
        )
}

fn building(world: &World, owner: PlayerId, kind: BuildingKind) -> BuildingId {
    world
        .view()
        .buildings()
        .find(|(_, b)| b.owner == owner && b.kind == kind)
        .map(|(id, _)| id)
        .expect("expected the building to be on the map")
}

fn unit_count(world: &World, owner: PlayerId) -> usize {
    world
        .view()
        .units()
        .filter(|(_, u)| u.owner == owner)
        .count()
}

fn run(world: &mut World, ticks: u64) {
    for _ in 0..ticks {
        world.step(&[]);
    }
}

// --- training -------------------------------------------------------------

#[test]
fn training_charges_the_cost_from_content() {
    let mut content = Content::default();
    content.villager.cost = 30;
    let mut world = World::with_content(SEED, base_map(), content);
    let town_center = building(&world, PlayerId::One, BuildingKind::TownCenter);

    world.step(&[Command::Train {
        player: PlayerId::One,
        building: town_center,
        unit: UnitKind::Villager,
    }]);

    assert_eq!(world.view().gold(PlayerId::One), 170);
}

#[test]
fn training_takes_the_time_from_content() {
    let mut content = Content::default();
    content.villager.train_ticks = 10;
    let mut world = World::with_content(SEED, base_map(), content);
    let town_center = building(&world, PlayerId::One, BuildingKind::TownCenter);

    world.step(&[Command::Train {
        player: PlayerId::One,
        building: town_center,
        unit: UnitKind::Villager,
    }]);
    run(&mut world, 10);

    // The default is 100 ticks, so the unit only exists this early if the
    // building read the content's number.
    assert_eq!(unit_count(&world, PlayerId::One), 1);
}

// --- units ----------------------------------------------------------------

fn only_unit_of(world: &World, owner: PlayerId) -> UnitId {
    world
        .view()
        .units()
        .find(|(_, u)| u.owner == owner)
        .map(|(id, _)| id)
        .expect("expected a unit")
}

fn hp(world: &World, unit: UnitId) -> Option<i32> {
    world.view().unit(unit).map(|u| u.hp)
}

/// A soldier of player one and a villager of player two, `gap` tiles apart.
fn duel(content: Content, gap: i32) -> (World, UnitId, UnitId) {
    let map = base_map()
        .with_unit(UnitKind::Soldier, PlayerId::One, TilePos::new(20, 10))
        .with_unit(
            UnitKind::Villager,
            PlayerId::Two,
            TilePos::new(20, 10 + gap),
        );
    let mut world = World::with_content(SEED, map, content);
    let soldier = only_unit_of(&world, PlayerId::One);
    let victim = only_unit_of(&world, PlayerId::Two);
    world.step(&[Command::Attack {
        player: PlayerId::One,
        units: smallvec![soldier],
        target: EntityId::Unit(victim),
    }]);
    (world, soldier, victim)
}

#[test]
fn a_unit_starts_with_the_hit_points_from_content() {
    let mut content = Content::default();
    content.villager.max_hp = 7;
    let map = base_map().with_unit(UnitKind::Villager, PlayerId::One, TilePos::new(20, 20));
    let world = World::with_content(SEED, map, content);

    assert_eq!(hp(&world, only_unit_of(&world, PlayerId::One)), Some(7));
}

#[test]
fn a_hit_deals_the_damage_from_content() {
    let mut content = Content::default();
    content.soldier.damage = 15;
    let (mut world, _, victim) = duel(content, 1);
    run(&mut world, 2);

    assert_eq!(hp(&world, victim), Some(40 - 15));
}

#[test]
fn swings_are_spaced_by_the_cooldown_from_content() {
    let mut content = Content::default();
    content.soldier.attack_cooldown = 1;
    let (mut world, _, victim) = duel(content, 1);
    // Five eight-damage swings kill a forty-hit-point villager. At the default
    // cooldown of 20 ticks that takes 80 ticks or more.
    run(&mut world, 12);

    assert_eq!(hp(&world, victim), None, "the villager survived");
}

#[test]
fn a_hit_lands_from_the_range_in_content() {
    let mut content = Content::default();
    content.soldier.range = Fx::from_int(10);
    let (mut world, _, victim) = duel(content, 8);
    // Walking eight tiles at the default range takes over fifty ticks.
    run(&mut world, 2);

    assert_eq!(hp(&world, victim), Some(40 - 8));
}

#[test]
fn a_unit_moves_at_the_speed_from_content() {
    let walked = |content: Content| {
        let map = base_map().with_unit(UnitKind::Villager, PlayerId::One, TilePos::new(2, 20));
        let mut world = World::with_content(SEED, map, content);
        let unit = only_unit_of(&world, PlayerId::One);
        world.step(&[Command::Move {
            player: PlayerId::One,
            units: smallvec![unit],
            target: TilePos::new(30, 20),
        }]);
        run(&mut world, 20);
        world.view().unit(unit).expect("alive").pos.x
    };
    let mut fast = Content::default();
    fast.villager.speed = Fx::from_frac(1, 4);

    assert!(walked(fast) > walked(Content::default()));
}

#[test]
fn a_villager_armed_by_content_can_be_ordered_to_attack() {
    let mut content = Content::default();
    content.villager.damage = 5;
    content.villager.range = Fx::from_frac(3, 2);
    content.villager.attack_cooldown = 20;
    let map = base_map()
        .with_unit(UnitKind::Villager, PlayerId::One, TilePos::new(20, 10))
        .with_unit(UnitKind::Villager, PlayerId::Two, TilePos::new(20, 11));
    let mut world = World::with_content(SEED, map, content);
    let attacker = only_unit_of(&world, PlayerId::One);
    let victim = only_unit_of(&world, PlayerId::Two);

    world.step(&[Command::Attack {
        player: PlayerId::One,
        units: smallvec![attacker],
        target: EntityId::Unit(victim),
    }]);
    run(&mut world, 2);

    assert_eq!(hp(&world, victim), Some(40 - 5));
}

#[test]
fn the_view_exposes_the_content_the_world_plays_by() {
    let mut content = Content::default();
    content.soldier.max_hp = 99;
    let world = World::with_content(SEED, base_map(), content.clone());

    assert_eq!(world.view().content(), &content);
}

// --- buildings ------------------------------------------------------------

#[test]
fn a_building_starts_with_the_hit_points_from_content() {
    let mut content = Content::default();
    content.town_center.max_hp = 123;
    let world = World::with_content(SEED, base_map(), content);
    let town_center = building(&world, PlayerId::One, BuildingKind::TownCenter);

    assert_eq!(world.view().building(town_center).map(|b| b.hp), Some(123));
}

#[test]
fn a_building_trains_what_content_says_it_trains() {
    let mut content = Content::default();
    content.barracks.trains = vec![UnitKind::Soldier, UnitKind::Villager];
    let mut world = World::with_content(SEED, base_map(), content);
    let barracks = building(&world, PlayerId::One, BuildingKind::Barracks);

    world.step(&[Command::Train {
        player: PlayerId::One,
        building: barracks,
        unit: UnitKind::Villager,
    }]);

    // By default a barracks refuses villagers and no gold is spent.
    assert_eq!(world.view().gold(PlayerId::One), 200 - 50);
}

// --- gathering ------------------------------------------------------------

/// A villager of player one next to a mine, a short walk from the town
/// centre, already ordered to gather.
fn gathering(content: Content) -> World {
    let map = base_map().with_mine(TilePos::new(10, 4), 5000).with_unit(
        UnitKind::Villager,
        PlayerId::One,
        TilePos::new(9, 8),
    );
    let mut world = World::with_content(SEED, map, content);
    let villager = only_unit_of(&world, PlayerId::One);
    let mine = world.view().mines().next().map(|(id, _)| id).expect("mine");
    world.step(&[Command::Gather {
        player: PlayerId::One,
        units: smallvec![villager],
        mine,
    }]);
    world
}

fn mine_remaining(world: &World) -> i32 {
    world.view().mines().next().map_or(0, |(_, m)| m.remaining)
}

#[test]
fn gold_is_only_banked_at_a_building_content_marks_as_a_drop_off() {
    let mut content = Content::default();
    content.town_center.drop_off = false;
    let mut world = gathering(content);
    // Long enough at the default numbers to mine and bank a full load.
    run(&mut world, 700);

    assert!(mine_remaining(&world) < 5000, "the villager never mined");
    assert_eq!(world.view().gold(PlayerId::One), 200, "gold was banked");
}

#[test]
fn a_villager_carries_no_more_than_the_capacity_in_content() {
    let mut content = Content::default();
    content.gather.carry_capacity = 3;
    let mut world = gathering(content);

    let mut most = 0;
    for _ in 0..700 {
        world.step(&[]);
        let carrying = world.view().units().map(|(_, u)| u.carrying).max();
        most = most.max(carrying.unwrap_or(0));
    }

    assert_eq!(most, 3);
}

#[test]
fn gold_is_mined_at_the_rate_in_content() {
    let mut content = Content::default();
    content.gather.ticks_per_gold = 1;
    let mut world = gathering(content);
    // At the default 40 ticks per gold this is about two gold.
    run(&mut world, 100);

    assert!(
        mine_remaining(&world) <= 4990,
        "only {} gold mined",
        5000 - mine_remaining(&world)
    );
}

// --- replay ---------------------------------------------------------------

#[test]
fn a_recording_made_with_custom_content_verifies_against_that_content() {
    let mut content = Content::default();
    content.villager.cost = 30;
    let mut world = World::with_content(SEED, base_map(), content.clone());
    let mut replay = Replay::with_content(SEED, base_map(), content);
    let town_center = building(&world, PlayerId::One, BuildingKind::TownCenter);

    for tick in 0..=200u64 {
        let commands = if tick == 1 {
            vec![Command::Train {
                player: PlayerId::One,
                building: town_center,
                unit: UnitKind::Villager,
            }]
        } else {
            Vec::new()
        };
        replay.record(tick, &commands);
        replay.record_checksum(tick, world.checksum());
        world.step(&commands);
    }

    // Played back with the default 50-gold villager, the gold left over would
    // differ by tick 100 and the checksum with it.
    assert_eq!(sim::verify(&replay), Ok(()));
}

// --- extremes -------------------------------------------------------------

#[test]
fn the_largest_damage_content_allows_kills_rather_than_overflowing() {
    // Two hits of i32::MAX on one victim in one tick overflow plain
    // subtraction: a debug build panics and a release build wraps the victim
    // back to life, so the two builds would play different games.
    let mut content = Content::default();
    content.soldier.damage = i32::MAX;
    let map = base_map()
        .with_unit(UnitKind::Soldier, PlayerId::One, TilePos::new(20, 10))
        .with_unit(UnitKind::Soldier, PlayerId::One, TilePos::new(21, 10))
        .with_unit(UnitKind::Villager, PlayerId::Two, TilePos::new(20, 11));
    let mut world = World::with_content(SEED, map, content);
    let soldiers: smallvec::SmallVec<[UnitId; 32]> = world
        .view()
        .units()
        .filter(|(_, u)| u.owner == PlayerId::One)
        .map(|(id, _)| id)
        .collect();
    let victim = only_unit_of(&world, PlayerId::Two);

    world.step(&[Command::Attack {
        player: PlayerId::One,
        units: soldiers,
        target: EntityId::Unit(victim),
    }]);
    run(&mut world, 2);

    assert_eq!(hp(&world, victim), None, "the villager survived");
}

#[test]
fn banking_a_load_into_a_full_treasury_saturates_rather_than_overflowing() {
    let map = base_map()
        .with_starting_gold(i32::MAX - 5)
        .with_mine(TilePos::new(10, 4), 5000)
        .with_unit(UnitKind::Villager, PlayerId::One, TilePos::new(9, 8));
    let mut world = World::with_content(SEED, map, Content::default());
    let villager = only_unit_of(&world, PlayerId::One);
    let mine = world.view().mines().next().map(|(id, _)| id).expect("mine");

    world.step(&[Command::Gather {
        player: PlayerId::One,
        units: smallvec![villager],
        mine,
    }]);
    // Long enough at the default numbers to bank a full ten-gold load.
    run(&mut world, 700);

    assert_eq!(world.view().gold(PlayerId::One), i32::MAX);
}
