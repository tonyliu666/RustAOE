//! More than two players, Gaia, and per-player defeat.

use sim::{
    BuildingKind, Command, Content, EntityId, Fx, MapSpec, Outcome, PlayerId, TilePos, UnitId,
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
/// knock a player out in a few ticks. Uses old-style soldier stats (damage 8,
/// cooldown 20) so 5 ticks is enough for the hit to land.
fn fragile_town_centres() -> Content {
    let mut content = Content::default();
    content.building_mut(BuildingKind::TOWN_CENTER).max_hp = 8;
    content.unit_mut(UnitKind::SOLDIER).damage = 8;
    content.unit_mut(UnitKind::SOLDIER).attack_cooldown = 20;
    content.unit_mut(UnitKind::SOLDIER).range = Fx::ONE;
    content
}

#[test]
fn a_four_player_game_plays_to_a_single_winner() {
    // Players 3 and 4 start with nothing standing and are out on tick one.
    // Player 1's soldier stands beside player 2's only building.
    let map = MapSpec::new(32, 32)
        .with_players(4)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(20, 20),
        )
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(19, 21));
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
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(20, 20),
        )
        .with_unit(UnitKind::VILLAGER, PlayerId::new(3), TilePos::new(10, 10));
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
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(20, 20),
        )
        .with_unit(UnitKind::VILLAGER, PlayerId::new(3), TilePos::new(10, 10));
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
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(20, 20),
        )
        .with_unit(UnitKind::VILLAGER, PlayerId::GAIA, TilePos::new(10, 10));
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
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TOWN_CENTER,
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
        BuildingKind::TOWN_CENTER,
        PlayerId::new(5),
        TilePos::new(2, 2),
    );
    let unit = MapSpec::new(16, 16).with_players(2).with_unit(
        UnitKind::VILLAGER,
        PlayerId::new(3),
        TilePos::new(2, 2),
    );

    assert!(building.validate().is_err());
    assert!(unit.validate().is_err());
}

#[test]
fn a_map_may_give_entities_to_gaia() {
    let map =
        MapSpec::new(16, 16).with_unit(UnitKind::VILLAGER, PlayerId::GAIA, TilePos::new(2, 2));

    assert_eq!(map.validate(), Ok(()));
}

// --- maps against content -------------------------------------------------------

#[test]
fn a_map_fits_the_content_it_names_types_from() {
    let map = MapSpec::new(16, 16)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(2, 2))
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 8));

    assert_eq!(map.validate_for(&Content::default()), Ok(()));
}

#[test]
fn a_map_naming_a_building_type_the_content_lacks_is_refused() {
    let map =
        MapSpec::new(16, 16).with_building(BuildingKind::new(9), PlayerId::ONE, TilePos::new(2, 2));

    assert!(map.validate_for(&Content::default()).is_err());
}

#[test]
fn a_map_naming_a_unit_type_the_content_lacks_is_refused() {
    let map = MapSpec::new(16, 16).with_unit(UnitKind::new(7), PlayerId::ONE, TilePos::new(2, 2));

    assert!(map.validate_for(&Content::default()).is_err());
}

#[test]
fn a_map_checked_against_content_still_checks_its_players() {
    let map = MapSpec::new(16, 16).with_players(9);

    assert!(map.validate_for(&Content::default()).is_err());
}

// --- the gate, played out ---------------------------------------------------------

/// A soldier of `attacker` beside every enemy town centre it is told to hit.
fn attack_building(
    world: &World,
    attacker: PlayerId,
    soldier_at: TilePos,
    owner: PlayerId,
) -> Command {
    let (soldier, _) = world
        .view()
        .units()
        .find(|(_, u)| u.owner == attacker && u.tile() == soldier_at)
        .expect("the soldier");
    let (target, _) = world
        .view()
        .buildings()
        .find(|(_, b)| b.owner == owner)
        .expect("the target");
    Command::Attack {
        player: attacker,
        units: smallvec![soldier],
        target: EntityId::Building(target),
    }
}

#[test]
fn three_players_lose_their_buildings_in_play_and_the_fourth_wins() {
    // Every player has a fragile town centre; player one has a soldier beside
    // each of the others'.
    let map = MapSpec::new(32, 32)
        .with_players(4)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(20, 2),
        )
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::new(3),
            TilePos::new(2, 20),
        )
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::new(4),
            TilePos::new(20, 20),
        )
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(19, 3))
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(3, 19))
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(19, 21));
    let mut world = World::with_content(SEED, map, fragile_town_centres());

    let first = [
        attack_building(&world, PlayerId::ONE, TilePos::new(19, 3), PlayerId::TWO),
        attack_building(&world, PlayerId::ONE, TilePos::new(3, 19), PlayerId::new(3)),
    ];
    world.step(&first);
    run(&mut world, 5);
    assert!(world.view().defeated(PlayerId::TWO));
    assert!(world.view().defeated(PlayerId::new(3)));
    assert_eq!(world.outcome(), None, "player four is still standing");

    let last = attack_building(
        &world,
        PlayerId::ONE,
        TilePos::new(19, 21),
        PlayerId::new(4),
    );
    world.step(&[last]);
    run(&mut world, 5);

    assert!(world.view().defeated(PlayerId::new(4)));
    assert_eq!(world.outcome(), Some(Outcome::Won(PlayerId::ONE)));
}

#[test]
fn two_players_losing_their_last_buildings_on_the_same_tick_is_a_draw() {
    // Each side's soldier stands beside the other's fragile town centre, and
    // both strike on the same tick.
    let map = MapSpec::new(32, 32)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(20, 2),
        )
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(19, 3))
        .with_unit(UnitKind::SOLDIER, PlayerId::TWO, TilePos::new(6, 3));
    let mut world = World::with_content(SEED, map, fragile_town_centres());

    let strikes = [
        attack_building(&world, PlayerId::ONE, TilePos::new(19, 3), PlayerId::TWO),
        attack_building(&world, PlayerId::TWO, TilePos::new(6, 3), PlayerId::ONE),
    ];
    world.step(&strikes);
    run(&mut world, 5);

    assert!(world.view().defeated(PlayerId::ONE));
    assert!(world.view().defeated(PlayerId::TWO));
    assert_eq!(world.outcome(), Some(Outcome::Draw));
}

#[test]
fn every_player_on_the_proof_of_concept_map_starts_with_food_and_gold() {
    // 200 food, the original's standard start, so the town centre can train
    // villagers before food can be gathered; 200 gold for the barracks.
    let content = sim::Content::default();
    let world = World::new(
        1,
        MapSpec::proof_of_concept(&content).expect("the map builds"),
    );

    for player in [PlayerId::ONE, PlayerId::TWO] {
        let stock = world.view().resources(player);
        assert_eq!((stock.food, stock.gold), (200, 200), "{player:?}");
    }
}
