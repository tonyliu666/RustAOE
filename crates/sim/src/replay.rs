//! Recorded command streams.
//!
//! Replay is nearly free given a command-driven deterministic simulation: the
//! seed, the map, and the commands are the entire game. It is also the primary
//! determinism regression test, because a checksum that diverges on playback
//! is a desync caught on one machine instead of in a live match.

use crate::{Command, Content, MapSpec};
use serde::{Deserialize, Serialize};

/// Everything needed to reproduce a session exactly.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Replay {
    /// The file format this replay was written in. See
    /// [`Replay::FORMAT_VERSION`].
    pub version: u32,
    pub seed: u64,
    pub map: MapSpec,
    /// The numbers the game was played by.
    pub content: Content,
    /// Tick to the commands issued on it, sparse and in ascending tick order.
    pub commands: Vec<(u64, Vec<Command>)>,
    /// Checksums sampled every [`Replay::CHECKSUM_INTERVAL`] ticks.
    pub checksums: Vec<(u64, u64)>,
}

impl Replay {
    /// The replay format this build writes and plays.
    ///
    /// Bumped whenever a change alters how a recorded game plays back or how
    /// the file is laid out. Version 1 is every file written before versions
    /// existed, which have no `version` field. Version 2 added the field, and
    /// was written in two layouts while Phase 1.5 changed the content's shape.
    /// Version 3 is the first with unit and building types as data.
    pub const FORMAT_VERSION: u32 = 3;

    /// How often the checksum is sampled during recording.
    ///
    /// Every tick would make the file larger than the commands it exists to
    /// hold; every hundred ticks is five seconds, which is close enough to
    /// point at the system that diverged.
    pub const CHECKSUM_INTERVAL: u64 = 100;

    /// A recording of a game played with the default content.
    pub fn new(seed: u64, map: MapSpec) -> Replay {
        Replay::with_content(seed, map, Content::default())
    }

    /// A recording of a game played with `content`.
    pub fn with_content(seed: u64, map: MapSpec, content: Content) -> Replay {
        Replay {
            version: Replay::FORMAT_VERSION,
            seed,
            map,
            content,
            commands: Vec::new(),
            checksums: Vec::new(),
        }
    }

    /// Records the commands applied on a tick.
    ///
    /// Empty ticks are not stored, which is most of them.
    pub fn record(&mut self, tick: u64, commands: &[Command]) {
        if commands.is_empty() {
            return;
        }
        self.commands.push((tick, commands.to_vec()));
    }

    /// Records a checksum if this tick is a sampling point.
    pub fn record_checksum(&mut self, tick: u64, checksum: u64) {
        if tick.is_multiple_of(Replay::CHECKSUM_INTERVAL) {
            self.checksums.push((tick, checksum));
        }
    }

    /// The commands recorded for a tick.
    pub fn commands_at(&self, tick: u64) -> &[Command] {
        self.commands
            .iter()
            .find(|(at, _)| *at == tick)
            .map_or(&[], |(_, cmds)| cmds.as_slice())
    }

    /// The checksum expected at a tick, if one was sampled there.
    pub fn checksum_at(&self, tick: u64) -> Option<u64> {
        self.checksums
            .iter()
            .find(|(at, _)| *at == tick)
            .map(|(_, sum)| *sum)
    }

    /// The last tick the recording covers.
    pub fn last_tick(&self) -> u64 {
        let commands = self.commands.last().map_or(0, |(tick, _)| *tick);
        let checksums = self.checksums.last().map_or(0, |(tick, _)| *tick);
        commands.max(checksums)
    }
}

/// Where a playback diverged from its recording.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Desync {
    pub tick: u64,
    pub expected: u64,
    pub actual: u64,
}

impl std::fmt::Display for Desync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "desync at tick {}: expected checksum {:#018x}, got {:#018x}",
            self.tick, self.expected, self.actual
        )
    }
}

impl std::error::Error for Desync {}

/// Replays a recording from scratch and checks every sampled checksum.
///
/// Returns the first divergence rather than all of them: after the first one
/// the two worlds are different games, and the later mismatches say nothing.
pub fn verify(replay: &Replay) -> Result<(), Desync> {
    let mut world =
        crate::World::with_content(replay.seed, replay.map.clone(), replay.content.clone());
    let last = replay.last_tick();
    for tick in 0..=last {
        if let Some(expected) = replay.checksum_at(tick) {
            let actual = world.checksum();
            if actual != expected {
                return Err(Desync {
                    tick,
                    expected,
                    actual,
                });
            }
        }
        world.step(replay.commands_at(tick));
    }
    Ok(())
}
