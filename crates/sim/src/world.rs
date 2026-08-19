//! The simulation state and the single tick that advances it.

use crate::{Grid, Rng, TilePos};

/// One terrain cell.
///
/// Only passability for now. Units never write to terrain; buildings stamp
/// tiles blocked when placed and unstamp on death.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Tile {
    pub passable: bool,
}

impl Tile {
    pub const OPEN: Tile = Tile { passable: true };
    pub const BLOCKED: Tile = Tile { passable: false };
}

/// The map a world is built from.
///
/// Part of a replay, so it has to describe the map fully rather than pointing
/// at a file that may have changed since the recording.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MapSpec {
    width: i32,
    height: i32,
    blocked: Vec<TilePos>,
}

impl MapSpec {
    /// A map that is passable everywhere.
    pub fn new(width: i32, height: i32) -> MapSpec {
        assert!(width > 0 && height > 0, "map must have positive extent");
        MapSpec {
            width,
            height,
            blocked: Vec::new(),
        }
    }

    /// Marks tiles impassable. Positions outside the map are ignored.
    pub fn with_blocked(mut self, tiles: impl IntoIterator<Item = TilePos>) -> MapSpec {
        self.blocked.extend(tiles);
        self
    }

    pub const fn width(&self) -> i32 {
        self.width
    }

    pub const fn height(&self) -> i32 {
        self.height
    }
}

/// Something a player asked the simulation to do.
///
/// This enum is simultaneously the input abstraction, the replay format, and
/// the future wire format. Variants arrive with the systems that act on them,
/// so it is empty while the world has nothing but terrain.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Command {}

/// The whole simulation state.
pub struct World {
    tick: u64,
    rng: Rng,
    terrain: Grid<Tile>,
}

impl World {
    pub fn new(seed: u64, map: MapSpec) -> World {
        let mut terrain = Grid::new(map.width, map.height, Tile::OPEN);
        for pos in &map.blocked {
            if terrain.in_bounds(*pos) {
                terrain.set(*pos, Tile::BLOCKED);
            }
        }
        World {
            tick: 0,
            rng: Rng::from_seed(seed),
            terrain,
        }
    }

    /// Advances the world exactly one tick.
    ///
    /// Cannot fail and reports nothing. Invalid commands are dropped rather
    /// than returned as errors: once commands arrive from remote players,
    /// invalid traffic is expected traffic.
    pub fn step(&mut self, _cmds: &[Command]) {
        self.tick += 1;
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    /// Read-only access to the state, for drawing and for the application's
    /// command mapping.
    pub const fn view(&self) -> WorldView<'_> {
        WorldView { world: self }
    }

    /// FNV-1a over the state that a desync would show up in.
    ///
    /// Computed on demand rather than every tick. The generator state is
    /// included because every draw is part of world state: two worlds that
    /// have consumed different numbers of draws have diverged even if nothing
    /// visible differs yet.
    pub fn checksum(&self) -> u64 {
        const OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01B3;

        let mut hash = OFFSET;
        let mut eat = |value: u64| {
            for byte in value.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(PRIME);
            }
        };

        eat(self.tick);
        for word in self.rng.state() {
            eat(word);
        }
        hash
    }
}

/// A read-only window onto a [`World`].
#[derive(Copy, Clone)]
pub struct WorldView<'a> {
    world: &'a World,
}

impl WorldView<'_> {
    pub const fn tick(&self) -> u64 {
        self.world.tick
    }

    pub const fn width(&self) -> i32 {
        self.world.terrain.width()
    }

    pub const fn height(&self) -> i32 {
        self.world.terrain.height()
    }

    /// The tile at `pos`, or `None` if it is off the map.
    pub fn tile(&self, pos: TilePos) -> Option<Tile> {
        self.world.terrain.get(pos).copied()
    }
}
