//! More than two players, Gaia, and per-player defeat.

use sim::{
    BuildingKind, Command, Content, EntityId, MapSpec, Outcome, PlayerId, TilePos, UnitId,
    UnitKind, World,
};
use smallvec::smallvec;

const SEED: u64 = 0x9A1A;

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

/// Content whose town centre falls to one soldier hit, so that a test can
/// knock a player out in a few ticks.
fn fragile_town_centres() -> Content {
    let mut content = Content::default();
    content.town_center.max_hp = 8;
    content
}

#[test]
fn a_four_player_game_plays_to_a_single_winner() {
    // Players 3 and 4 start with nothing standing and are out on tick one.
    // Player 1's soldier stands beside player 2's only building.
    let map = MapSpec::new(32, 32)
        .with_players(4)
        .with_building(BuildingKind::TownCenter, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TownCenter,
            PlayerId::TWO,
            TilePos::new(20, 20),
        )
        .with_unit(UnitKind::Soldier, PlayerId::ONE, TilePos::new(19, 21));
    let mut world = World::with_content(SEED, map, fragile_town_centres());
    world.step(&[]);

    assert!(world.view().defeated(PlayerId::new(3)));
    assert!(world.view().defeated(PlayerId::new(4)));
    assert_eq!(world.outcome(), None, "two players are still standing");

    let soldier = units_of(&world, PlayerId::ONE)[0];
    let (target, _) = world
        .view()
        .buildings()
        .find(|(_, b)| b.owner == PlayerId::TWO)
        .expect("player two's town centre");
    world.step(&[Command::Attack {
        player: PlayerId::ONE,
        units: smallvec![soldier],
        target: EntityId::Building(target),
    }]);
    run(&mut world, 5);

    assert!(world.view().defeated(PlayerId::TWO));
    assert_eq!(world.outcome(), Some(Outcome::Won(PlayerId::ONE)));
}

#[test]
fn a_defeated_player_is_out_while_the_others_play_on() {
    let map = MapSpec::new(32, 32)
        .with_players(3)
        .with_building(BuildingKind::TownCenter, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TownCenter,
            PlayerId::TWO,
            TilePos::new(20, 20),
        )
        .with_unit(UnitKind::Villager, PlayerId::new(3), TilePos::new(10, 10));
    let mut world = World::new(SEED, map);
    world.step(&[]);

    assert!(world.view().defeated(PlayerId::new(3)));
    assert!(!world.view().defeated(PlayerId::ONE));
    assert_eq!(world.outcome(), None);
}

#[test]
fn a_defeated_players_orders_are_dropped() {
    let map = MapSpec::new(32, 32)
        .with_players(3)
        .with_building(BuildingKind::TownCenter, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TownCenter,
            PlayerId::TWO,
            TilePos::new(20, 20),
        )
        .with_unit(UnitKind::Villager, PlayerId::new(3), TilePos::new(10, 10));
    let mut world = World::new(SEED, map);
    world.step(&[]);
    let villager = units_of(&world, PlayerId::new(3))[0];
    let before = world.view().unit(villager).expect("alive").pos;

    world.step(&[Command::Move {
        player: PlayerId::new(3),
        units: smallvec![villager],
        target: TilePos::new(14, 10),
    }]);
    run(&mut world, 20);

    assert_eq!(world.view().unit(villager).expect("alive").pos, before);
}

#[test]
fn when_the_last_players_fall_on_the_same_tick_it_is_a_draw() {
    let map = MapSpec::new(16, 16).with_players(2);
    let mut world = World::new(SEED, map);

    world.step(&[]);

    assert_eq!(world.outcome(), Some(Outcome::Draw));
}

#[test]
fn gaia_cannot_give_orders_even_to_its_own_units() {
    let map = MapSpec::new(32, 32)
        .with_building(BuildingKind::TownCenter, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TownCenter,
            PlayerId::TWO,
            TilePos::new(20, 20),
        )
        .with_unit(UnitKind::Villager, PlayerId::GAIA, TilePos::new(10, 10));
    let mut world = World::new(SEED, map);
    let sheep = units_of(&world, PlayerId::GAIA)[0];
    let before = world.view().unit(sheep).expect("alive").pos;

    world.step(&[Command::Move {
        player: PlayerId::GAIA,
        units: smallvec![sheep],
        target: TilePos::new(14, 10),
    }]);
    run(&mut world, 20);

    assert_eq!(world.view().unit(sheep).expect("alive").pos, before);
}

#[test]
fn gaia_is_never_defeated_and_never_wins() {
    // Gaia owns nothing, which would defeat any other player.
    let map = MapSpec::new(32, 32)
        .with_building(BuildingKind::TownCenter, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TownCenter,
            PlayerId::TWO,
            TilePos::new(20, 20),
        );
    let mut world = World::new(SEED, map);

    world.step(&[]);

    assert!(!world.view().defeated(PlayerId::GAIA));
    assert_eq!(world.outcome(), None);
}

#[test]
fn a_map_accepts_two_to_eight_players() {
    for players in 2..=8 {
        assert_eq!(MapSpec::new(8, 8).with_players(players).validate(), Ok(()));
    }
}

#[test]
fn a_map_refuses_fewer_than_two_or_more_than_eight_players() {
    for players in [0, 1, 9] {
        assert!(
            MapSpec::new(8, 8).with_players(players).validate().is_err(),
            "{players} players was accepted"
        );
    }
}

#[test]
fn a_map_refuses_an_entity_for_a_player_it_does_not_have() {
    let building = MapSpec::new(16, 16).with_players(4).with_building(
        BuildingKind::TownCenter,
        PlayerId::new(5),
        TilePos::new(2, 2),
    );
    let unit = MapSpec::new(16, 16).with_players(2).with_unit(
        UnitKind::Villager,
        PlayerId::new(3),
        TilePos::new(2, 2),
    );

    assert!(building.validate().is_err());
    assert!(unit.validate().is_err());
}

#[test]
fn a_map_may_give_entities_to_gaia() {
    let map =
        MapSpec::new(16, 16).with_unit(UnitKind::Villager, PlayerId::GAIA, TilePos::new(2, 2));

    assert_eq!(map.validate(), Ok(()));
}
