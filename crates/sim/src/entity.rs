//! Entities, their statistics, and the identifiers that address them.
//!
//! Every reference between entities is a generational identifier rather than a
//! Rust reference. That sidesteps the cyclic-ownership problem that makes game
//! object graphs painful, and it keeps the whole state trivially copyable and
//! serializable.

use crate::{Fx, FxVec2, Resources, TilePos};
use serde::{Deserialize, Serialize};
use slotmap::new_key_type;

new_key_type! {
    /// Addresses a unit for as long as it is alive.
    pub struct UnitId;
    /// Addresses a building for as long as it is standing.
    pub struct BuildingId;
    /// Addresses a gold mine.
    pub struct MineId;
}

/// A player slot: Gaia, or one of up to eight players.
///
/// Gaia is slot 0, as in the original (`FUN_0040bdf0` builds Gaia before the
/// players). It owns the map's neutral objects, never issues commands, and is
/// never defeated.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct PlayerId(u8);

impl PlayerId {
    pub const GAIA: PlayerId = PlayerId(0);
    pub const ONE: PlayerId = PlayerId(1);
    pub const TWO: PlayerId = PlayerId(2);
    /// Players a map may have, not counting Gaia.
    pub const MAX_PLAYERS: u8 = 8;

    /// Player `number`: 0 is Gaia, 1 to 8 are players.
    pub const fn new(number: u8) -> PlayerId {
        PlayerId(number)
    }

    /// 0 for Gaia, 1 to 8 for players, as shown to people.
    pub const fn number(self) -> u8 {
        self.0
    }

    /// The slot in the world's player table.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    pub const fn is_gaia(self) -> bool {
        self.0 == 0
    }
}

/// Anything a command can point at.
///
/// A tagged identifier rather than three separate command variants: an attack
/// order does not care what shape its victim is.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum EntityId {
    Unit(UnitId),
    Building(BuildingId),
    Mine(MineId),
}

/// A unit type: an index into [`crate::Content::units`].
///
/// Types are data, not code: the simulation never names one. The constants
/// are the IDs in [`crate::Content::default`], for tests and the built-in map.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct UnitKind(u16);

impl UnitKind {
    /// The villager in the default content.
    pub const VILLAGER: UnitKind = UnitKind(0);
    /// The soldier in the default content.
    pub const SOLDIER: UnitKind = UnitKind(1);

    pub const fn new(index: u16) -> UnitKind {
        UnitKind(index)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// What a unit is currently trying to do.
///
/// One order at a time, replaced wholesale by a new command. Order queues are a
/// later spec.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Order {
    Idle,
    Move {
        goal: TilePos,
        route: Route,
    },
    Attack {
        target: EntityId,
        route: Route,
    },
    Gather {
        mine: MineId,
        phase: GatherPhase,
        route: Route,
    },
}

/// How a unit is finding its way to the current goal.
///
/// Group orders share one flow field; a lone unit gets its own A* path, since
/// building a 16 KB field for one villager is pure waste.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Route {
    /// Waiting in the path queue for the budget to reach it.
    Pending { goal: TilePos },
    /// Following the shared flow field for this goal tile.
    Flow { goal: TilePos },
    /// Following its own waypoint list; `next` indexes the current waypoint.
    Path { steps: Vec<TilePos>, next: usize },
    /// Standing still: adjacent to the goal, or given up on reaching it.
    Arrived,
}

impl Route {
    pub fn goal(&self) -> Option<TilePos> {
        match self {
            Route::Pending { goal } | Route::Flow { goal } => Some(*goal),
            Route::Path { steps, next } => {
                steps.get(*next).copied().or_else(|| steps.last().copied())
            }
            Route::Arrived => None,
        }
    }
}

/// Where a villager is in the mine-carry-deposit cycle.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum GatherPhase {
    /// Walking to the mine.
    ToMine,
    /// Standing at the mine, accumulating gold on a tick counter.
    Mining,
    /// Walking to a drop-off building with a full load.
    ToDropOff { building: BuildingId },
}

/// A unit on the map.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Unit {
    pub kind: UnitKind,
    pub owner: PlayerId,
    pub pos: FxVec2,
    pub hp: i32,
    pub order: Order,
    /// Gold held, up to the content's `gather.carry_capacity`.
    pub carrying: i32,
    /// Ticks spent at the mine face since the last gold was extracted.
    pub mine_ticks: u32,
    /// Ticks until this unit may attack again.
    pub attack_cooldown: u32,
    /// Consecutive ticks of no net movement while holding an unreached goal.
    pub stuck_ticks: u32,
    /// Re-plans spent on the current order after stalling.
    pub retries: u32,
    /// Position at the start of the tick, for the stuck check and for render
    /// interpolation.
    pub prev_pos: FxVec2,
}

impl Unit {
    /// Ticks of no movement before a unit re-plans or abandons its order.
    pub const STUCK_LIMIT: u32 = 30;
    /// Re-plans allowed before a stalled unit gives up for good.
    ///
    /// Without a retry, one unit shouldered into a wall while the rest of its
    /// group files through a gap would idle there permanently. Without a
    /// limit, a unit ordered somewhere genuinely unreachable would re-plan
    /// forever and burn the path budget every tick.
    pub const STUCK_RETRIES: u32 = 2;
    /// How close counts as having arrived, in tiles.
    pub const ARRIVE_RADIUS: Fx = Fx::from_frac(3, 4);

    /// A fresh unit at full health. `hp` is the kind's `max_hp` from the
    /// content the world plays by.
    pub fn new(kind: UnitKind, owner: PlayerId, pos: FxVec2, hp: i32) -> Unit {
        Unit {
            kind,
            owner,
            pos,
            hp,
            order: Order::Idle,
            carrying: 0,
            mine_ticks: 0,
            attack_cooldown: 0,
            stuck_ticks: 0,
            retries: 0,
            prev_pos: pos,
        }
    }

    pub const fn tile(&self) -> TilePos {
        self.pos.tile()
    }
}

/// A building type: an index into [`crate::Content::buildings`].
///
/// The constants are the IDs in [`crate::Content::default`].
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct BuildingKind(u16);

impl BuildingKind {
    /// The town centre in the default content.
    pub const TOWN_CENTER: BuildingKind = BuildingKind(0);
    /// The barracks in the default content.
    pub const BARRACKS: BuildingKind = BuildingKind(1);

    pub const fn new(index: u16) -> BuildingKind {
        BuildingKind(index)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// A building on the map.
///
/// Buildings are placed by the map, never constructed in game: build-site
/// validation and construction progress are a later spec.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Building {
    pub kind: BuildingKind,
    pub owner: PlayerId,
    /// Top-left tile of the footprint.
    pub origin: TilePos,
    pub hp: i32,
    /// Footprint side in tiles, from its type's content when placed, so that
    /// its footprint is known without the content in hand.
    pub size: i32,
    /// Units queued for training, oldest first.
    pub queue: Vec<UnitKind>,
    /// Ticks remaining on the unit at the head of the queue.
    pub train_ticks: u32,
}

impl Building {
    /// How many units may be waiting to train.
    pub const QUEUE_LIMIT: usize = 5;

    /// A building at full health. `hp` and `size` are the kind's `max_hp`
    /// and `size` from the content the world plays by.
    pub fn new(
        kind: BuildingKind,
        owner: PlayerId,
        origin: TilePos,
        hp: i32,
        size: i32,
    ) -> Building {
        Building {
            kind,
            owner,
            origin,
            hp,
            size,
            queue: Vec::new(),
            train_ticks: 0,
        }
    }

    /// The tiles this building covers.
    pub fn footprint(&self) -> impl Iterator<Item = TilePos> + '_ {
        let size = self.size;
        let origin = self.origin;
        (0..size)
            .flat_map(move |dy| (0..size).map(move |dx| TilePos::new(origin.x + dx, origin.y + dy)))
    }

    /// Centre of the footprint in world space.
    pub fn center(&self) -> FxVec2 {
        let size = self.size;
        FxVec2::new(
            Fx::from_frac(self.origin.x * 2 + size, 2),
            Fx::from_frac(self.origin.y * 2 + size, 2),
        )
    }
}

/// A depletable gold deposit.
///
/// An entity rather than a terrain flag so that it can hold remaining gold and
/// be the target of a command.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Mine {
    /// Top-left tile of the 2x2 footprint.
    pub origin: TilePos,
    pub remaining: i32,
}

impl Mine {
    pub const SIZE: i32 = 2;
    pub const STARTING_GOLD: i32 = 5000;

    pub fn new(origin: TilePos, remaining: i32) -> Mine {
        Mine { origin, remaining }
    }

    pub fn footprint(&self) -> impl Iterator<Item = TilePos> + '_ {
        let origin = self.origin;
        (0..Mine::SIZE).flat_map(move |dy| {
            (0..Mine::SIZE).map(move |dx| TilePos::new(origin.x + dx, origin.y + dy))
        })
    }

    pub fn center(&self) -> FxVec2 {
        FxVec2::new(
            Fx::from_frac(self.origin.x * 2 + Mine::SIZE, 2),
            Fx::from_frac(self.origin.y * 2 + Mine::SIZE, 2),
        )
    }
}

/// Per-player state the simulation owns.
///
/// Camera and selection are deliberately absent: where someone is looking is
/// the application's business and must never affect the simulation.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Player {
    pub resources: Resources,
    /// Out of the game. Set once and never cleared; a defeated player's
    /// commands are dropped.
    pub defeated: bool,
}
