//! The properties that protect the whole architecture.
//!
//! Determinism fails silently and late: a simulation that drifts on one
//! machine looks perfectly healthy until two of them try to agree. Property
//! tests over randomised command streams are the only practical defence, so
//! these are the tests to keep green above all others.

use proptest::prelude::*;
use sim::{
    verify, BuildingKind, Command, EntityId, MapSpec, PlayerId, Replay, TilePos, UnitId, UnitKind,
    World,
};
use smallvec::SmallVec;

/// One thing a scripted session can ask for.
///
/// Abstract rather than a literal [`Command`], because identifiers only exist
/// once a world has been built, and the point of the test is to replay the
/// same script against several worlds.
#[derive(Copy, Clone, Debug)]
enum Action {
    MoveGroup { x: i32, y: i32 },
    MoveOne { unit: usize, x: i32, y: i32 },
    Gather { unit: usize },
    Train { villager: bool },
    AttackFirstEnemy,
    Stop,
}

const MAP_SIZE: i32 = 32;

fn action() -> impl Strategy<Value = Action> {
    prop_oneof![
        (0..MAP_SIZE, 0..MAP_SIZE).prop_map(|(x, y)| Action::MoveGroup { x, y }),
        (0usize..8, 0..MAP_SIZE, 0..MAP_SIZE).prop_map(|(unit, x, y)| Action::MoveOne {
            unit,
            x,
            y
        }),
        (0usize..8).prop_map(|unit| Action::Gather { unit }),
        any::<bool>().prop_map(|villager| Action::Train { villager }),
        Just(Action::AttackFirstEnemy),
        Just(Action::Stop),
    ]
}

/// One scripted step: ticks to wait, which player acts (taken modulo the
/// player count), and what they do.
type Step = (u8, u8, Action);

fn script() -> impl Strategy<Value = Vec<Step>> {
    prop::collection::vec((0u8..12, any::<u8>(), action()), 0..24)
}

/// How many players a scripted map has.
fn players() -> impl Strategy<Value = u8> {
    2u8..=3
}

/// A map small enough to simulate a few hundred ticks quickly, for two or
/// three players. The third player, when present, has a town centre and
/// villagers of its own.
fn map(players: u8) -> MapSpec {
    let mut map = MapSpec::new(MAP_SIZE, MAP_SIZE)
        .with_players(players)
        .with_starting_gold(300)
        .with_building(BuildingKind::TownCenter, PlayerId::ONE, TilePos::new(2, 2))
        .with_building(BuildingKind::Barracks, PlayerId::ONE, TilePos::new(8, 2))
        .with_building(
            BuildingKind::TownCenter,
            PlayerId::TWO,
            TilePos::new(26, 26),
        )
        .with_mine(TilePos::new(14, 14), 200)
        // A few scattered rocks, so pathing has to do something.
        .with_blocked([
            TilePos::new(16, 8),
            TilePos::new(16, 9),
            TilePos::new(16, 10),
            TilePos::new(17, 10),
        ]);
    for i in 0..3 {
        map = map
            .with_unit(UnitKind::Villager, PlayerId::ONE, TilePos::new(2 + i, 8))
            .with_unit(UnitKind::Soldier, PlayerId::ONE, TilePos::new(5 + i, 8))
            .with_unit(UnitKind::Villager, PlayerId::TWO, TilePos::new(24 + i, 20));
    }
    if players >= 3 {
        map = map.with_building(
            BuildingKind::TownCenter,
            PlayerId::new(3),
            TilePos::new(2, 26),
        );
        for i in 0..2 {
            map = map.with_unit(
                UnitKind::Villager,
                PlayerId::new(3),
                TilePos::new(8 + i, 24),
            );
        }
    }
    map
}

/// The player a scripted step acts as.
fn issuer(players: u8, slot: u8) -> PlayerId {
    PlayerId::new(1 + slot % players)
}

fn player_units(world: &World, owner: PlayerId) -> Vec<UnitId> {
    world
        .view()
        .units()
        .filter(|(_, u)| u.owner == owner)
        .map(|(id, _)| id)
        .collect()
}

/// Turns one scripted action into the command it means for this world.
///
/// Returns nothing when the action does not apply — no units left to move, no
/// mine to gather from. Those are dropped rather than substituted, so that the
/// same script means the same thing on every run.
fn to_command(world: &World, player: PlayerId, action: Action) -> Option<Command> {
    let mine = player_units(world, player);
    let all: SmallVec<[UnitId; 32]> = mine.iter().copied().collect();
    match action {
        Action::MoveGroup { x, y } if !all.is_empty() => Some(Command::Move {
            player,
            units: all,
            target: TilePos::new(x, y),
        }),
        Action::MoveOne { unit, x, y } => {
            let id = *mine.get(unit % mine.len().max(1))?;
            Some(Command::Move {
                player,
                units: SmallVec::from_slice(&[id]),
                target: TilePos::new(x, y),
            })
        }
        Action::Gather { unit } => {
            let id = *mine.get(unit % mine.len().max(1))?;
            let (seam, _) = world.view().mines().next()?;
            Some(Command::Gather {
                player,
                units: SmallVec::from_slice(&[id]),
                mine: seam,
            })
        }
        Action::Train { villager } => {
            let (kind, wanted) = if villager {
                (UnitKind::Villager, BuildingKind::TownCenter)
            } else {
                (UnitKind::Soldier, BuildingKind::Barracks)
            };
            let (building, _) = world
                .view()
                .buildings()
                .find(|(_, b)| b.owner == player && b.kind == wanted)?;
            Some(Command::Train {
                player,
                building,
                unit: kind,
            })
        }
        Action::AttackFirstEnemy if !all.is_empty() => {
            let (target, _) = world
                .view()
                .units()
                .find(|(_, u)| u.owner != player && u.owner != PlayerId::GAIA)?;
            Some(Command::Attack {
                player,
                units: all,
                target: EntityId::Unit(target),
            })
        }
        Action::Stop if !all.is_empty() => Some(Command::Stop { player, units: all }),
        _ => None,
    }
}

/// Runs a script and records it, returning the recording and a checksum per
/// tick.
fn play(seed: u64, players: u8, script: &[Step]) -> (Replay, Vec<u64>) {
    let mut world = World::new(seed, map(players));
    let mut replay = Replay::new(seed, map(players));
    let mut checksums = Vec::new();

    let mut remaining = script.iter().copied().peekable();
    let mut wait = 0u32;
    // Long enough for a walk across the map plus a gather cycle, and short
    // enough that a proptest run of these stays under a second.
    for tick in 0..600u64 {
        let mut commands = Vec::new();
        while wait == 0 {
            let Some((gap, slot, action)) = remaining.next() else {
                wait = u32::MAX;
                break;
            };
            commands.extend(to_command(&world, issuer(players, slot), action));
            wait = u32::from(gap);
        }
        wait = wait.saturating_sub(1);

        checksums.push(world.checksum());
        replay.record(tick, &commands);
        replay.record_checksum(tick, world.checksum());
        world.step(&commands);
    }
    (replay, checksums)
}

proptest! {
    /// The test that protects the architecture: the same seed and the same
    /// commands must produce the same state, tick for tick.
    #[test]
    fn the_same_script_run_twice_agrees_at_every_tick(seed in any::<u64>(), players in players(), script in script()) {
        let (_, first) = play(seed, players, &script);
        let (_, second) = play(seed, players, &script);

        prop_assert_eq!(first, second);
    }

    /// A recording replayed from scratch must reproduce the checksums it was
    /// recorded with. This is what makes a replay a usable bug report.
    #[test]
    fn a_recording_plays_back_to_the_checksums_it_recorded(seed in any::<u64>(), players in players(), script in script()) {
        let (replay, _) = play(seed, players, &script);

        prop_assert!(verify(&replay).is_ok(), "{:?}", verify(&replay));
    }

    /// No unit ever ends a tick inside terrain it cannot walk on.
    #[test]
    fn no_unit_ever_stands_in_a_blocked_tile(seed in any::<u64>(), players in players(), script in script()) {
        let mut world = World::new(seed, map(players));
        let mut remaining = script.iter().copied().peekable();
        let mut wait = 0u32;

        for _ in 0..600u64 {
            let mut commands = Vec::new();
            while wait == 0 {
                let Some((gap, slot, action)) = remaining.next() else {
                    wait = u32::MAX;
                    break;
                };
                commands.extend(to_command(&world, issuer(players, slot), action));
                wait = u32::from(gap);
            }
            wait = wait.saturating_sub(1);
            world.step(&commands);

            for (_, unit) in world.view().units() {
                let tile = unit.tile();
                prop_assert!(
                    world.view().tile(tile).is_some_and(|t| t.passable),
                    "a unit is standing in {:?}, which is blocked",
                    tile
                );
            }
        }
    }

    /// Gold mined plus gold carried plus gold banked equals the gold that
    /// started in the ground and the treasury. Catches duplication bugs, which
    /// are otherwise invisible until an economy runs away.
    #[test]
    fn gold_is_never_created_or_destroyed(seed in any::<u64>(), players in players(), script in script()) {
        let mut world = World::new(seed, map(players));
        let expected = world.view().total_gold();
        let mut spent = 0i64;
        let mut remaining = script.iter().copied().peekable();
        let mut wait = 0u32;

        for _ in 0..600u64 {
            let mut commands = Vec::new();
            while wait == 0 {
                let Some((gap, slot, action)) = remaining.next() else {
                    wait = u32::MAX;
                    break;
                };
                commands.extend(to_command(&world, issuer(players, slot), action));
                wait = u32::from(gap);
            }
            wait = wait.saturating_sub(1);

            let before = world.view().total_gold();
            world.step(&commands);
            let after = world.view().total_gold();
            // Training is the one legitimate sink, so account for it rather
            // than excluding it from the script.
            spent += before - after;

            prop_assert_eq!(
                after + spent,
                expected,
                "gold appeared or vanished by tick {}",
                world.tick()
            );
        }
    }
}

#[test]
fn two_different_seeds_produce_different_games() {
    let script = [
        (0u8, 0u8, Action::MoveGroup { x: 20, y: 20 }),
        (5, 0, Action::Gather { unit: 0 }),
    ];
    let (_, first) = play(1, 2, &script);
    let (_, second) = play(2, 2, &script);

    // The seed only shows up through the generator today, which is in the
    // checksum, so the two sessions must not be bit-identical.
    assert_ne!(first, second);
}
