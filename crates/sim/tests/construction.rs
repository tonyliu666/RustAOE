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

/// Every villager of `player`, as an order lists them.
fn villagers_of(world: &World, player: PlayerId) -> smallvec::SmallVec<[sim::UnitId; 32]> {
    units_of(world, player, UnitKind::VILLAGER)
        .into_iter()
        .map(|(id, _)| id)
        .collect()
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

    world.step(&[Command::PlaceBuilding {
        player: PlayerId::ONE,
        units: villagers_of(&world, PlayerId::ONE),
        kind: BuildingKind::HOUSE,
        origin: TilePos::new(8, 6),
    }]);

    // Should create a foundation
    assert!(has_foundation(&world));
    assert_eq!(foundation_count(&world), 1);
    let (_, foundation) = world.view().foundations().next().expect("foundation");
    assert_eq!(foundation.kind, BuildingKind::HOUSE);
    assert_eq!(foundation.origin, TilePos::new(8, 6));
    assert_eq!(foundation.hp, 1, "one hit point before any work");
    assert_eq!(foundation.max_hp, 900); // the house, from Content::default()
}

#[test]
fn placement_off_map_is_rejected() {
    let mut world =
        make_world(base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 8)));

    world.step(&[Command::PlaceBuilding {
        player: PlayerId::ONE,
        units: villagers_of(&world, PlayerId::ONE),
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

    world.step(&[Command::PlaceBuilding {
        player: PlayerId::ONE,
        units: villagers_of(&world, PlayerId::ONE),
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

    world.step(&[Command::PlaceBuilding {
        player: PlayerId::ONE,
        units: villagers_of(&world, PlayerId::ONE),
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

    world.step(&[Command::PlaceBuilding {
        player: PlayerId::ONE,
        units: villagers_of(&world, PlayerId::ONE),
        kind: BuildingKind::HOUSE,
        origin: TilePos::new(8, 6),
    }]);

    assert!(!has_foundation(&world));
}

#[test]
fn placement_cost_is_paid_from_player_resources() {
    let mut world =
        make_world(base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 8)));
    let content = Content::default();
    let house_cost = content.building(BuildingKind::HOUSE).cost.wood; // house costs 25 wood

    let before = world.view().resources(PlayerId::ONE);
    world.step(&[Command::PlaceBuilding {
        player: PlayerId::ONE,
        units: villagers_of(&world, PlayerId::ONE),
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

// --- the foundation: work done by its builders ------------------------------------

/// The house's build time in ticks: one villager's time, from the content.
fn house_ticks() -> u32 {
    Content::default()
        .building(BuildingKind::HOUSE)
        .build_time
        .expect("a house is built")
}

/// A world with `villagers` of player one's villagers beside tile (8, 6),
/// a house foundation placed there, and every villager building it.
fn house_being_built_by(villagers: i32) -> (World, sim::FoundationId) {
    let mut map = base_map();
    for i in 0..villagers {
        map = map.with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(7 + i, 9));
    }
    let mut world = make_world(map);
    let foundation = place_house(&mut world);
    for (id, _) in units_of(&world, PlayerId::ONE, UnitKind::VILLAGER) {
        assert!(world.assign_builder(foundation, id));
    }
    (world, foundation)
}

/// Places a house, then at once stops the villagers who would have gone to
/// build it, so that no one works on it until a test says so.
fn place_house(world: &mut World) -> sim::FoundationId {
    let villagers = villagers_of(world, PlayerId::ONE);
    world.step(&[
        Command::PlaceBuilding {
            player: PlayerId::ONE,
            units: villagers.clone(),
            kind: BuildingKind::HOUSE,
            origin: TilePos::new(8, 6),
        },
        Command::Stop {
            player: PlayerId::ONE,
            units: villagers,
        },
    ]);
    world.view().foundations().next().expect("a foundation").0
}

fn units_of(world: &World, owner: PlayerId, kind: UnitKind) -> Vec<(sim::UnitId, TilePos)> {
    world
        .view()
        .units()
        .filter(|(_, u)| u.owner == owner && u.kind == kind)
        .map(|(id, u)| (id, u.tile()))
        .collect()
}

fn houses(world: &World) -> usize {
    world
        .view()
        .buildings()
        .filter(|(_, b)| b.kind == BuildingKind::HOUSE)
        .count()
}

fn steps(world: &mut World, ticks: u32) {
    for _ in 0..ticks {
        world.step(&[]);
    }
}

#[test]
fn one_villager_builds_a_house_in_its_build_time() {
    let (mut world, _) = house_being_built_by(1);

    steps(&mut world, house_ticks() - 1);
    assert_eq!((world.view().foundations().count(), houses(&world)), (1, 0));

    world.step(&[]);
    assert_eq!((world.view().foundations().count(), houses(&world)), (0, 1));
}

#[test]
fn a_foundations_hit_points_rise_with_the_work_done() {
    let (mut world, foundation) = house_being_built_by(1);

    steps(&mut world, house_ticks() / 2);

    let foundation = world.view().foundation(foundation).expect("still building");
    assert_eq!(foundation.hp, 1 + (foundation.max_hp - 1) / 2);
}

#[test]
fn each_further_builder_adds_a_third_of_a_villagers_work() {
    // Provisional: the original's rule as players know it, a build time of
    // 3t / (n + 2) for n builders; not yet found in the decompile.
    for (builders, ticks) in [(2, house_ticks() * 3 / 4), (3, house_ticks() * 3 / 5)] {
        let (mut world, _) = house_being_built_by(builders);

        steps(&mut world, ticks - 1);
        assert_eq!(houses(&world), 0, "{builders} builders, a tick early");
        world.step(&[]);
        assert_eq!(houses(&world), 1, "{builders} builders");
    }
}

#[test]
fn a_foundation_no_one_builds_stays_as_it_is() {
    let mut world =
        make_world(base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 9)));
    let foundation = place_house(&mut world);

    steps(&mut world, house_ticks() * 2);

    assert_eq!(
        world.view().foundation(foundation).expect("still there").hp,
        1
    );
    assert_eq!(houses(&world), 0);
}

#[test]
fn a_finished_house_stands_where_its_foundation_was_and_blocks_its_tiles() {
    let (mut world, _) = house_being_built_by(1);

    steps(&mut world, house_ticks());

    let (_, house) = world
        .view()
        .buildings()
        .find(|(_, b)| b.kind == BuildingKind::HOUSE)
        .expect("a house");
    let max_hp = Content::default().building(BuildingKind::HOUSE).max_hp;
    assert_eq!(
        (house.owner, house.origin, house.size, house.hp),
        (PlayerId::ONE, TilePos::new(8, 6), 2, max_hp)
    );
    for tile in house.footprint() {
        assert!(!world.view().tile(tile).expect("on the map").passable);
    }
}

#[test]
fn a_builders_death_pauses_the_work_and_another_villager_resumes_it() {
    // An enemy soldier kills the first of two villagers; the second is set
    // to build only afterwards.
    let mut world = make_world(
        base_map()
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 9))
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(12, 12))
            .with_unit(UnitKind::SOLDIER, PlayerId::TWO, TilePos::new(9, 9)),
    );
    let foundation = place_house(&mut world);
    let villagers = units_of(&world, PlayerId::ONE, UnitKind::VILLAGER);
    let doomed = villagers
        .iter()
        .find(|(_, at)| *at == TilePos::new(8, 9))
        .expect("the builder")
        .0;
    let spare = villagers
        .iter()
        .find(|(id, _)| *id != doomed)
        .expect("a spare")
        .0;
    let (soldier, _) = units_of(&world, PlayerId::TWO, UnitKind::SOLDIER)[0];
    assert!(world.assign_builder(foundation, doomed));
    world.step(&[Command::Attack {
        player: PlayerId::TWO,
        units: smallvec::smallvec![soldier],
        target: sim::EntityId::Unit(doomed),
    }]);
    while world.view().unit(doomed).is_some() {
        world.step(&[]);
    }
    let paused_at = world.view().foundation(foundation).expect("unfinished").hp;
    assert!(paused_at > 0);

    steps(&mut world, 50);
    assert_eq!(
        world.view().foundation(foundation).expect("unfinished").hp,
        paused_at
    );

    assert!(world.assign_builder(foundation, spare));
    world.step(&[]);
    assert!(world.view().foundation(foundation).expect("unfinished").hp > paused_at);
}

#[test]
fn only_a_villager_of_the_foundations_owner_may_build_it() {
    let mut world = make_world(
        base_map()
            .with_unit(UnitKind::VILLAGER, PlayerId::TWO, TilePos::new(8, 9))
            .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(9, 9))
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(10, 9)),
    );
    let foundation = place_house(&mut world);
    let (enemy, _) = units_of(&world, PlayerId::TWO, UnitKind::VILLAGER)[0];
    let (soldier, _) = units_of(&world, PlayerId::ONE, UnitKind::SOLDIER)[0];
    let (villager, _) = units_of(&world, PlayerId::ONE, UnitKind::VILLAGER)[0];

    assert!(!world.assign_builder(foundation, enemy), "another player's");
    assert!(!world.assign_builder(foundation, soldier), "not a villager");
    assert!(world.assign_builder(foundation, villager));
    assert!(
        !world.assign_builder(foundation, villager),
        "already building"
    );
    let builders = &world.view().foundation(foundation).expect("there").builders;
    assert_eq!(builders.as_slice(), [villager]);
}

#[test]
fn work_on_a_foundation_shows_in_the_checksum() {
    // Both end with one builder on the house; only one has worked a tick.
    let (mut worked, _) = house_being_built_by(1);
    worked.step(&[]);
    let mut idle =
        make_world(base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(7, 9)));
    let foundation = place_house(&mut idle);
    idle.step(&[]);
    let (villager, _) = units_of(&idle, PlayerId::ONE, UnitKind::VILLAGER)[0];
    assert!(idle.assign_builder(foundation, villager));

    assert_ne!(worked.checksum(), idle.checksum());
}

#[test]
fn a_foundations_builders_show_in_the_checksum() {
    let mut with =
        make_world(base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(7, 9)));
    let without = {
        let mut w =
            make_world(base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(7, 9)));
        place_house(&mut w);
        w
    };
    let foundation = place_house(&mut with);
    assert_eq!(with.checksum(), without.checksum());
    let (villager, _) = units_of(&with, PlayerId::ONE, UnitKind::VILLAGER)[0];

    assert!(with.assign_builder(foundation, villager));

    assert_ne!(with.checksum(), without.checksum());
}

// --- the foundation under attack ------------------------------------------------

/// Player one's villager at (8, 9) building a house at (8, 6), and, with
/// `attacked`, player two's soldier beside it attacking the foundation.
fn house_built_while(attacked: bool) -> (World, sim::FoundationId) {
    let mut map = base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 9));
    if attacked {
        map = map.with_unit(UnitKind::SOLDIER, PlayerId::TWO, TilePos::new(10, 7));
    }
    let mut world = make_world(map);
    let foundation = place_house(&mut world);
    let (villager, _) = units_of(&world, PlayerId::ONE, UnitKind::VILLAGER)[0];
    assert!(world.assign_builder(foundation, villager));
    if attacked {
        let (soldier, _) = units_of(&world, PlayerId::TWO, UnitKind::SOLDIER)[0];
        world.step(&[Command::Attack {
            player: PlayerId::TWO,
            units: smallvec::smallvec![soldier],
            target: sim::EntityId::Foundation(foundation),
        }]);
    } else {
        world.step(&[]);
    }
    (world, foundation)
}

#[test]
fn damage_to_a_foundation_stays_while_it_is_built() {
    let (mut peaceful, foundation) = house_built_while(false);
    let (mut attacked, attacked_foundation) = house_built_while(true);

    steps(&mut peaceful, 100);
    steps(&mut attacked, 100);

    let hp = |world: &World, id| world.view().foundation(id).expect("unfinished").hp;
    assert!(hp(&attacked, attacked_foundation) < hp(&peaceful, foundation));
}

#[test]
fn a_foundation_brought_to_no_hit_points_is_destroyed() {
    let mut world = make_world(
        base_map()
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 14))
            .with_unit(UnitKind::SOLDIER, PlayerId::TWO, TilePos::new(10, 7)),
    );
    let foundation = place_house(&mut world);
    let (soldier, _) = units_of(&world, PlayerId::TWO, UnitKind::SOLDIER)[0];
    world.step(&[Command::Attack {
        player: PlayerId::TWO,
        units: smallvec::smallvec![soldier],
        target: sim::EntityId::Foundation(foundation),
    }]);

    steps(&mut world, 200);

    assert!(world.view().foundation(foundation).is_none());
    assert_eq!(houses(&world), 0);
}

#[test]
fn a_house_finished_after_damage_keeps_the_damage() {
    let (mut world, _) = house_built_while(true);
    steps(&mut world, 20);
    let (soldier, _) = units_of(&world, PlayerId::TWO, UnitKind::SOLDIER)[0];
    world.step(&[Command::Stop {
        player: PlayerId::TWO,
        units: smallvec::smallvec![soldier],
    }]);

    steps(&mut world, house_ticks());

    let (_, house) = world
        .view()
        .buildings()
        .find(|(_, b)| b.kind == BuildingKind::HOUSE)
        .expect("a house");
    assert!(house.hp < house_max_hp(), "{}", house.hp);
}

fn house_max_hp() -> i32 {
    Content::default().building(BuildingKind::HOUSE).max_hp
}

// --- the build order ----------------------------------------------------------------

fn villager(world: &World, at: TilePos) -> sim::UnitId {
    units_of(world, PlayerId::ONE, UnitKind::VILLAGER)
        .into_iter()
        .find(|(_, tile)| *tile == at)
        .expect("a villager there")
        .0
}

fn place_with(world: &mut World, units: &[sim::UnitId]) -> Option<sim::FoundationId> {
    world.step(&[Command::PlaceBuilding {
        player: PlayerId::ONE,
        units: units.iter().copied().collect(),
        kind: BuildingKind::HOUSE,
        origin: TilePos::new(8, 6),
    }]);
    world.view().foundations().next().map(|(id, _)| id)
}

fn build(player: PlayerId, units: &[sim::UnitId], foundation: sim::FoundationId) -> Command {
    Command::Build {
        player,
        units: units.iter().copied().collect(),
        foundation,
    }
}

fn run_until(world: &mut World, limit: u32, done: impl Fn(&World) -> bool) -> bool {
    for _ in 0..limit {
        if done(world) {
            return true;
        }
        world.step(&[]);
    }
    done(world)
}

#[test]
fn the_villagers_that_place_a_house_walk_over_and_build_it() {
    let mut world =
        make_world(base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 14)));
    let v = villager(&world, TilePos::new(8, 14));

    place_with(&mut world, &[v]).expect("a foundation");

    assert!(
        run_until(&mut world, house_ticks() + 600, |w| houses(w) == 1),
        "never built"
    );
    assert!(matches!(
        world.view().unit(v).expect("alive").order,
        sim::Order::Idle
    ));
}

#[test]
fn work_starts_only_when_the_villager_touches_the_foundation() {
    let mut world =
        make_world(base_map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 24)));
    let v = villager(&world, TilePos::new(8, 24));
    let foundation = place_with(&mut world, &[v]).expect("a foundation");

    steps(&mut world, 20);

    let far = world.view().foundation(foundation).expect("there");
    assert_eq!((far.hp, far.builders.len()), (1, 0), "still walking");
    assert!(run_until(&mut world, 1500, |w| {
        w.view().foundation(foundation).is_none_or(|f| f.hp > 1)
    }));
}

#[test]
fn a_house_needs_a_villager_to_place_it_and_costs_nothing_otherwise() {
    let mut world = make_world(
        base_map()
            .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(8, 14))
            .with_unit(UnitKind::VILLAGER, PlayerId::TWO, TilePos::new(9, 14)),
    );
    let soldier = units_of(&world, PlayerId::ONE, UnitKind::SOLDIER)[0].0;
    let enemy = units_of(&world, PlayerId::TWO, UnitKind::VILLAGER)[0].0;
    let wood = world.view().resources(PlayerId::ONE).wood;

    assert!(place_with(&mut world, &[]).is_none(), "no one");
    assert!(place_with(&mut world, &[soldier]).is_none(), "a soldier");
    assert!(
        place_with(&mut world, &[enemy]).is_none(),
        "an enemy villager"
    );

    assert_eq!(world.view().resources(PlayerId::ONE).wood, wood);
}

#[test]
fn a_build_order_sends_villagers_to_an_unfinished_foundation() {
    let mut world = make_world(
        base_map()
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 14))
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(14, 14)),
    );
    let first = villager(&world, TilePos::new(8, 14));
    let second = villager(&world, TilePos::new(14, 14));
    let foundation = place_with(&mut world, &[first]).expect("a foundation");
    world.step(&[Command::Stop {
        player: PlayerId::ONE,
        units: [first].into_iter().collect(),
    }]);

    world.step(&[build(PlayerId::ONE, &[first, second], foundation)]);

    assert!(
        run_until(&mut world, 1500, |w| {
            w.view()
                .foundation(foundation)
                .is_some_and(|f| f.builders.len() == 2)
        }),
        "both should be building"
    );
}

#[test]
fn only_your_villagers_obey_a_build_order_on_your_foundation() {
    let mut world = make_world(
        base_map()
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 14))
            .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(9, 14))
            .with_unit(UnitKind::VILLAGER, PlayerId::TWO, TilePos::new(10, 14)),
    );
    let mine = villager(&world, TilePos::new(8, 14));
    let soldier = units_of(&world, PlayerId::ONE, UnitKind::SOLDIER)[0].0;
    let enemy = units_of(&world, PlayerId::TWO, UnitKind::VILLAGER)[0].0;
    let foundation = place_with(&mut world, &[mine]).expect("a foundation");
    world.step(&[Command::Stop {
        player: PlayerId::ONE,
        units: [mine].into_iter().collect(),
    }]);

    world.step(&[
        build(PlayerId::ONE, &[soldier], foundation),
        build(PlayerId::TWO, &[enemy], foundation),
        build(PlayerId::TWO, &[mine], foundation),
    ]);

    for unit in [soldier, enemy, mine] {
        assert!(
            matches!(
                world.view().unit(unit).expect("alive").order,
                sim::Order::Idle
            ),
            "{unit:?}"
        );
    }
}

#[test]
fn another_order_stops_the_building() {
    let (mut world, foundation) = house_being_built_by(1);
    steps(&mut world, 10);
    let (v, _) = units_of(&world, PlayerId::ONE, UnitKind::VILLAGER)[0];
    let at = world.view().foundation(foundation).expect("there").hp;

    world.step(&[Command::Move {
        player: PlayerId::ONE,
        units: [v].into_iter().collect(),
        target: TilePos::new(20, 20),
    }]);
    steps(&mut world, 30);

    let after = world.view().foundation(foundation).expect("there");
    assert!(after.builders.is_empty());
    assert!(after.hp <= at + 2, "{} {at}", after.hp);
}

#[test]
fn builders_of_a_destroyed_foundation_stand_idle() {
    let mut world = make_world(
        base_map()
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 9))
            .with_unit(UnitKind::SOLDIER, PlayerId::TWO, TilePos::new(10, 7)),
    );
    let v = villager(&world, TilePos::new(8, 9));
    let foundation = place_with(&mut world, &[v]).expect("a foundation");
    let (soldier, _) = units_of(&world, PlayerId::TWO, UnitKind::SOLDIER)[0];
    world.step(&[Command::Attack {
        player: PlayerId::TWO,
        units: [soldier].into_iter().collect(),
        target: sim::EntityId::Foundation(foundation),
    }]);

    assert!(run_until(&mut world, 400, |w| {
        w.view().foundation(foundation).is_none()
    }));
    world.step(&[]);

    assert!(matches!(
        world.view().unit(v).expect("alive").order,
        sim::Order::Idle
    ));
}

#[test]
fn a_unit_on_the_footprint_when_the_house_is_finished_is_moved_off_it() {
    let mut world = make_world(
        base_map()
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 9))
            .with_unit(UnitKind::SOLDIER, PlayerId::TWO, TilePos::new(8, 6)),
    );
    let v = villager(&world, TilePos::new(8, 9));
    place_with(&mut world, &[v]).expect("a foundation");

    assert!(run_until(&mut world, house_ticks() + 600, |w| houses(w) == 1));
    steps(&mut world, 2);

    let (_, house) = world
        .view()
        .buildings()
        .find(|(_, b)| b.kind == BuildingKind::HOUSE)
        .expect("a house");
    let footprint: Vec<TilePos> = house.footprint().collect();
    for (_, unit) in world.view().units() {
        assert!(
            !footprint.contains(&unit.tile()),
            "{:?} stands in the house",
            unit.tile()
        );
    }
}

#[test]
fn which_foundation_a_villager_is_sent_to_shows_in_the_checksum() {
    // Both villagers stand still on both worlds; only which foundation each
    // is set to differs, and no tick passes to move them apart.
    let after = |swap: bool| {
        let mut world = make_world(
            base_map()
                .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 14))
                .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(9, 14)),
        );
        let first = villager(&world, TilePos::new(8, 14));
        let second = villager(&world, TilePos::new(9, 14));
        let mut foundations = Vec::new();
        for origin in [TilePos::new(8, 6), TilePos::new(14, 6)] {
            world.step(&[
                Command::PlaceBuilding {
                    player: PlayerId::ONE,
                    units: [first].into_iter().collect(),
                    kind: BuildingKind::HOUSE,
                    origin,
                },
                Command::Stop {
                    player: PlayerId::ONE,
                    units: [first].into_iter().collect(),
                },
            ]);
            foundations.push(world.view().foundations().last().expect("placed").0);
        }
        let (a, b) = (foundations[0], foundations[1]);
        let (to_first, to_second) = if swap { (b, a) } else { (a, b) };
        assert!(world.assign_builder(to_first, first));
        assert!(world.assign_builder(to_second, second));
        world.checksum()
    };

    assert_ne!(after(false), after(true));
}
