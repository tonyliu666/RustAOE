//! Content tables: the numbers the game plays by come from `Content`, not
//! from code.
//!
//! Each test changes one number away from the default and shows the game
//! plays differently. A test that still passed with the default numbers would
//! prove nothing about where the number comes from.

use sim::{
    BuildingId, BuildingKind, Command, Content, EntityId, Fx, MapSpec, PlayerId, Replay, Resources,
    TilePos, UnitId, UnitKind, World,
};
use smallvec::smallvec;

const SEED: u64 = 0xC0DE;

/// One town centre and one barracks for player one, one town centre for
/// player two so that nobody has already lost.
fn base_map() -> MapSpec {
    MapSpec::new(32, 32)
        .with_starting_resources(Resources {
            food: 200,
            gold: 200,
            ..Resources::default()
        })
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(4, 4))
        .with_building(BuildingKind::BARRACKS, PlayerId::ONE, TilePos::new(12, 4))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
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
    content.unit_mut(UnitKind::VILLAGER).cost.food = 30;
    let mut world = World::with_content(SEED, base_map(), content);
    let town_center = building(&world, PlayerId::ONE, BuildingKind::TOWN_CENTER);

    world.step(&[Command::Train {
        player: PlayerId::ONE,
        building: town_center,
        unit: UnitKind::VILLAGER,
    }]);

    assert_eq!(world.view().resources(PlayerId::ONE).food, 170);
}

#[test]
fn training_takes_the_time_from_content() {
    let mut content = Content::default();
    content.unit_mut(UnitKind::VILLAGER).train_ticks = 10;
    let mut world = World::with_content(SEED, base_map(), content);
    let town_center = building(&world, PlayerId::ONE, BuildingKind::TOWN_CENTER);

    world.step(&[Command::Train {
        player: PlayerId::ONE,
        building: town_center,
        unit: UnitKind::VILLAGER,
    }]);
    run(&mut world, 10);

    // The default is 100 ticks, so the unit only exists this early if the
    // building read the content's number.
    assert_eq!(unit_count(&world, PlayerId::ONE), 1);
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
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(20, 10))
        .with_unit(
            UnitKind::VILLAGER,
            PlayerId::TWO,
            TilePos::new(20, 10 + gap),
        );
    let mut world = World::with_content(SEED, map, content);
    let soldier = only_unit_of(&world, PlayerId::ONE);
    let victim = only_unit_of(&world, PlayerId::TWO);
    world.step(&[Command::Attack {
        player: PlayerId::ONE,
        units: smallvec![soldier],
        target: EntityId::Unit(victim),
    }]);
    (world, soldier, victim)
}

#[test]
fn a_unit_starts_with_the_hit_points_from_content() {
    let mut content = Content::default();
    content.unit_mut(UnitKind::VILLAGER).max_hp = 7;
    let map = base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(20, 20));
    let world = World::with_content(SEED, map, content);

    assert_eq!(hp(&world, only_unit_of(&world, PlayerId::ONE)), Some(7));
}

#[test]
fn a_hit_deals_the_damage_from_content() {
    let mut content = Content::default();
    content.unit_mut(UnitKind::SOLDIER).damage = 15;
    let (mut world, _, victim) = duel(content, 1);
    run(&mut world, 2);

    assert_eq!(hp(&world, victim), Some(40 - 15));
}

#[test]
fn swings_are_spaced_by_the_cooldown_from_content() {
    let mut content = Content::default();
    content.unit_mut(UnitKind::SOLDIER).attack_cooldown = 1;
    let (mut world, _, victim) = duel(content, 1);
    // Five eight-damage swings kill a forty-hit-point villager. At the default
    // cooldown of 20 ticks that takes 80 ticks or more.
    run(&mut world, 12);

    assert_eq!(hp(&world, victim), None, "the villager survived");
}

#[test]
fn a_hit_lands_from_the_range_in_content() {
    let mut content = Content::default();
    content.unit_mut(UnitKind::SOLDIER).range = Fx::from_int(10);
    let (mut world, _, victim) = duel(content, 8);
    // Walking eight tiles at the default range takes over fifty ticks.
    run(&mut world, 2);

    assert_eq!(hp(&world, victim), Some(40 - 8));
}

#[test]
fn a_unit_moves_at_the_speed_from_content() {
    let walked = |content: Content| {
        let map = base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(2, 20));
        let mut world = World::with_content(SEED, map, content);
        let unit = only_unit_of(&world, PlayerId::ONE);
        world.step(&[Command::Move {
            player: PlayerId::ONE,
            units: smallvec![unit],
            target: TilePos::new(30, 20),
        }]);
        run(&mut world, 20);
        world.view().unit(unit).expect("alive").pos.x
    };
    let mut fast = Content::default();
    fast.unit_mut(UnitKind::VILLAGER).speed = Fx::from_frac(1, 4);

    assert!(walked(fast) > walked(Content::default()));
}

#[test]
fn a_villager_armed_by_content_can_be_ordered_to_attack() {
    let mut content = Content::default();
    content.unit_mut(UnitKind::VILLAGER).damage = 5;
    content.unit_mut(UnitKind::VILLAGER).range = Fx::from_frac(3, 2);
    content.unit_mut(UnitKind::VILLAGER).attack_cooldown = 20;
    let map = base_map()
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(20, 10))
        .with_unit(UnitKind::VILLAGER, PlayerId::TWO, TilePos::new(20, 11));
    let mut world = World::with_content(SEED, map, content);
    let attacker = only_unit_of(&world, PlayerId::ONE);
    let victim = only_unit_of(&world, PlayerId::TWO);

    world.step(&[Command::Attack {
        player: PlayerId::ONE,
        units: smallvec![attacker],
        target: EntityId::Unit(victim),
    }]);
    run(&mut world, 2);

    assert_eq!(hp(&world, victim), Some(40 - 5));
}

#[test]
fn the_view_exposes_the_content_the_world_plays_by() {
    let mut content = Content::default();
    content.unit_mut(UnitKind::SOLDIER).max_hp = 99;
    let world = World::with_content(SEED, base_map(), content.clone());

    assert_eq!(world.view().content(), &content);
}

// --- buildings ------------------------------------------------------------

#[test]
fn a_building_starts_with_the_hit_points_from_content() {
    let mut content = Content::default();
    content.building_mut(BuildingKind::TOWN_CENTER).max_hp = 123;
    let world = World::with_content(SEED, base_map(), content);
    let town_center = building(&world, PlayerId::ONE, BuildingKind::TOWN_CENTER);

    assert_eq!(world.view().building(town_center).map(|b| b.hp), Some(123));
}

#[test]
fn a_building_trains_what_content_says_it_trains() {
    let mut content = Content::default();
    content.building_mut(BuildingKind::BARRACKS).trains =
        vec![UnitKind::SOLDIER, UnitKind::VILLAGER];
    let mut world = World::with_content(SEED, base_map(), content);
    let barracks = building(&world, PlayerId::ONE, BuildingKind::BARRACKS);

    world.step(&[Command::Train {
        player: PlayerId::ONE,
        building: barracks,
        unit: UnitKind::VILLAGER,
    }]);

    // By default a barracks refuses villagers and no food is spent.
    assert_eq!(world.view().resources(PlayerId::ONE).food, 200 - 50);
}

// --- gathering ------------------------------------------------------------

/// A villager of player one next to a mine, a short walk from the town
/// centre, already ordered to gather.
fn gathering(content: Content) -> World {
    let map = base_map().with_mine(TilePos::new(10, 4), 5000).with_unit(
        UnitKind::VILLAGER,
        PlayerId::ONE,
        TilePos::new(9, 8),
    );
    let mut world = World::with_content(SEED, map, content);
    let villager = only_unit_of(&world, PlayerId::ONE);
    let mine = world.view().mines().next().map(|(id, _)| id).expect("mine");
    world.step(&[Command::Gather {
        player: PlayerId::ONE,
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
    content.building_mut(BuildingKind::TOWN_CENTER).drop_off = false;
    let mut world = gathering(content);
    // Long enough at the default numbers to mine and bank a full load.
    run(&mut world, 700);

    assert!(mine_remaining(&world) < 5000, "the villager never mined");
    assert_eq!(world.view().gold(PlayerId::ONE), 200, "gold was banked");
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
    content.unit_mut(UnitKind::VILLAGER).cost.food = 30;
    let mut world = World::with_content(SEED, base_map(), content.clone());
    let mut replay = Replay::with_content(SEED, base_map(), content);
    let town_center = building(&world, PlayerId::ONE, BuildingKind::TOWN_CENTER);

    for tick in 0..=200u64 {
        let commands = if tick == 1 {
            vec![Command::Train {
                player: PlayerId::ONE,
                building: town_center,
                unit: UnitKind::VILLAGER,
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
    content.unit_mut(UnitKind::SOLDIER).damage = i32::MAX;
    let map = base_map()
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(20, 10))
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(21, 10))
        .with_unit(UnitKind::VILLAGER, PlayerId::TWO, TilePos::new(20, 11));
    let mut world = World::with_content(SEED, map, content);
    let soldiers: smallvec::SmallVec<[UnitId; 32]> = world
        .view()
        .units()
        .filter(|(_, u)| u.owner == PlayerId::ONE)
        .map(|(id, _)| id)
        .collect();
    let victim = only_unit_of(&world, PlayerId::TWO);

    world.step(&[Command::Attack {
        player: PlayerId::ONE,
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
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(9, 8));
    let mut world = World::with_content(SEED, map, Content::default());
    let villager = only_unit_of(&world, PlayerId::ONE);
    let mine = world.view().mines().next().map(|(id, _)| id).expect("mine");

    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: smallvec![villager],
        mine,
    }]);
    // Long enough at the default numbers to bank a full ten-gold load.
    run(&mut world, 700);

    assert_eq!(world.view().gold(PlayerId::ONE), i32::MAX);
}

// --- resources ------------------------------------------------------------

/// A villager costing 20 food and 30 gold, on a map where each player starts
/// with `food` food and 100 gold.
fn mixed_cost_world(food: i32) -> World {
    let mut content = Content::default();
    content.unit_mut(UnitKind::VILLAGER).cost = Resources {
        food: 20,
        gold: 30,
        ..Resources::default()
    };
    let map = base_map().with_starting_resources(Resources {
        food,
        gold: 100,
        ..Resources::default()
    });
    World::with_content(SEED, map, content)
}

fn train_villager(world: &mut World) {
    let town_center = building(world, PlayerId::ONE, BuildingKind::TOWN_CENTER);
    world.step(&[Command::Train {
        player: PlayerId::ONE,
        building: town_center,
        unit: UnitKind::VILLAGER,
    }]);
}

#[test]
fn training_charges_every_part_of_a_mixed_cost() {
    let mut world = mixed_cost_world(100);
    train_villager(&mut world);

    assert_eq!(
        world.view().resources(PlayerId::ONE),
        Resources {
            food: 80,
            gold: 70,
            ..Resources::default()
        }
    );
}

#[test]
fn training_is_refused_when_any_one_resource_is_short() {
    // Plenty of gold, not enough food.
    let mut world = mixed_cost_world(10);
    train_villager(&mut world);

    assert_eq!(
        world.view().resources(PlayerId::ONE),
        Resources {
            food: 10,
            gold: 100,
            ..Resources::default()
        },
        "something was charged for a refused order"
    );
}

#[test]
fn the_checksum_covers_every_resource() {
    let world_with_wood = |wood| {
        let map = base_map().with_starting_resources(Resources {
            wood,
            ..Resources::default()
        });
        World::with_content(SEED, map, Content::default()).checksum()
    };

    assert_ne!(world_with_wood(0), world_with_wood(1));
}

// --- kinds are data -----------------------------------------------------------

/// The default content plus an archer, trained at the barracks: a unit type
/// that exists only in content, with no code naming it.
fn with_archer() -> (Content, UnitKind) {
    let mut content = Content::default();
    let mut archer = content.unit(UnitKind::SOLDIER).clone();
    archer.name = "archer".to_string();
    archer.damage = 6;
    archer.range = Fx::from_int(4);
    archer.max_hp = 30;
    content.units.push(archer);
    let kind = content.unit_named("archer").expect("the archer was added");
    content
        .building_mut(BuildingKind::BARRACKS)
        .trains
        .push(kind);
    (content, kind)
}

#[test]
fn a_unit_type_added_only_in_content_is_trained_and_fights_with_its_own_numbers() {
    let (content, archer) = with_archer();
    let map = base_map().with_unit(UnitKind::VILLAGER, PlayerId::TWO, TilePos::new(16, 12));
    let mut world = World::with_content(SEED, map, content);
    let barracks = building(&world, PlayerId::ONE, BuildingKind::BARRACKS);

    world.step(&[Command::Train {
        player: PlayerId::ONE,
        building: barracks,
        unit: archer,
    }]);
    run(&mut world, 125);
    let (bowman, unit) = world
        .view()
        .units()
        .find(|(_, u)| u.kind == archer)
        .expect("the archer was trained");
    assert_eq!(unit.hp, 30);

    let victim = only_unit_of(&world, PlayerId::TWO);
    world.step(&[Command::Attack {
        player: PlayerId::ONE,
        units: smallvec![bowman],
        target: EntityId::Unit(victim),
    }]);
    run(&mut world, 60);

    // The archer's six damage, not the soldier's eight, so the villager's
    // hit points are a multiple of six below forty.
    let left = hp(&world, victim).unwrap_or(0);
    assert!(left < 40, "the archer never hit");
    assert_eq!((40 - left) % 6, 0, "villager at {left} hit points");
}

#[test]
fn a_unit_type_that_does_not_gather_refuses_a_gather_order() {
    let mut content = Content::default();
    content.unit_mut(UnitKind::VILLAGER).gathers = false;
    let map = base_map().with_mine(TilePos::new(10, 4), 5000).with_unit(
        UnitKind::VILLAGER,
        PlayerId::ONE,
        TilePos::new(9, 8),
    );
    let mut world = World::with_content(SEED, map, content);
    let villager = only_unit_of(&world, PlayerId::ONE);
    let mine = world.view().mines().next().map(|(id, _)| id).expect("mine");

    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: smallvec![villager],
        mine,
    }]);
    run(&mut world, 200);

    assert_eq!(world.view().mine(mine).map(|m| m.remaining), Some(5000));
}

#[test]
fn a_buildings_footprint_comes_from_content() {
    let mut content = Content::default();
    content.building_mut(BuildingKind::TOWN_CENTER).size = 5;
    let world = World::with_content(SEED, base_map(), content);

    // Player one's town centre stands at (4, 4): with size 5 it covers
    // (8, 8), which the default size 4 does not.
    assert_eq!(
        world.view().tile(TilePos::new(8, 8)).map(|t| t.passable),
        Some(false)
    );
    assert_eq!(
        world.view().tile(TilePos::new(9, 9)).map(|t| t.passable),
        Some(true)
    );
}

#[test]
fn kinds_are_found_by_name() {
    let content = Content::default();

    assert_eq!(content.unit_named("villager"), Some(UnitKind::VILLAGER));
    assert_eq!(content.unit_named("soldier"), Some(UnitKind::SOLDIER));
    assert_eq!(
        content.building_named("barracks"),
        Some(BuildingKind::BARRACKS)
    );
    assert_eq!(content.unit_named("dragon"), None);
}

// --- the built-in map -----------------------------------------------------------

#[test]
fn the_built_in_map_is_built_from_the_contents_named_types() {
    let content = Content::default();

    let map = MapSpec::proof_of_concept(&content).expect("the default content has every type");

    assert_eq!(map.validate(), Ok(()));
    let world = World::with_content(SEED, map, content);
    assert!(world
        .view()
        .buildings()
        .any(|(_, b)| b.kind == BuildingKind::TOWN_CENTER && b.owner == PlayerId::ONE));
}

#[test]
fn the_built_in_map_needs_a_town_centre_and_says_so() {
    let mut content = Content::default();
    content.building_mut(BuildingKind::TOWN_CENTER).name = "keep".to_string();

    let error = MapSpec::proof_of_concept(&content).expect_err("no town_center");

    assert!(error.to_string().contains("town_center"), "{error}");
}

// --- icons ------------------------------------------------------------------

#[test]
fn the_default_types_carry_the_originals_icons() {
    // Frames in the original's icon sprites: units in interfac.drs 50730,
    // buildings in 50705. Picked by eye; Phase 1.6 confirms them against the
    // .dat's icon IDs.
    let content = Content::default();

    assert_eq!(content.unit(UnitKind::VILLAGER).icon, 15);
    assert_eq!(content.unit(UnitKind::SOLDIER).icon, 8);
    assert_eq!(content.building(BuildingKind::TOWN_CENTER).icon, 28);
    assert_eq!(content.building(BuildingKind::BARRACKS).icon, 2);
}

#[test]
fn the_default_soldier_looks_like_the_militia_in_the_world_and_on_its_button() {
    // Read from the militia's entry in the .dat (unit 74): standing graphic
    // 1102, SPRMN_FN, whose SLP is 993; icon 8. The names mislead: SWDMN_FN
    // (SLP 1044) is the man-at-arms, and SLP 1181, used before, is the
    // two-handed swordsman, so the button and the unit it trained did not
    // match.
    let soldier = Content::default().unit(UnitKind::SOLDIER).clone();

    assert_eq!((soldier.sprite, soldier.icon), (993, 8));
}

#[test]
fn the_default_villager_costs_food_as_the_originals_does() {
    // 50 food, as the original's .dat (unit 83) has it; the soldier keeps
    // its 60 gold until Phase 2.
    let content = Content::default();

    assert_eq!(
        content.unit(UnitKind::VILLAGER).cost,
        sim::Resources {
            food: 50,
            ..sim::Resources::default()
        }
    );
    assert_eq!(content.unit(UnitKind::SOLDIER).cost.gold, 60);
}

#[test]
fn the_default_buildings_are_drawn_as_the_originals_dark_age_british_ones() {
    // From the .dat: the barracks (unit 12) stands with SLP 2683, one piece;
    // the town centre (unit 109) is its own graphic's pieces (889, 3596,
    // 4612; 890 is in no archive) and its three annexes' (891; 3595, 4611;
    // 3594, 4610), drawn at one point in that order.
    let content = Content::default();
    let barracks = content.building(BuildingKind::BARRACKS);
    let town_centre = content.building(BuildingKind::TOWN_CENTER);

    assert_eq!(
        (barracks.sprite, barracks.parts.as_slice()),
        (2683, &[][..])
    );
    assert_eq!(town_centre.sprite, 3596);
    assert_eq!(
        town_centre.parts,
        [889, 3596, 4612, 891, 3595, 4611, 3594, 4610]
    );
    assert!(content.unit(UnitKind::VILLAGER).parts.is_empty());
}
