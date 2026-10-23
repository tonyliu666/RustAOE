//! Entities, their statistics, and the identifiers that address them.
//!
//! Every reference between entities is a generational identifier rather than a
//! Rust reference. That sidesteps the cyclic-ownership problem that makes game
//! object graphs painful, and it keeps the whole state trivially copyable and
//! serializable.

use crate::{Fx, FxVec2, TilePos};
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

/// One of the two hotseat players.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum PlayerId {
    One,
    Two,
}

impl PlayerId {
    pub const ALL: [PlayerId; 2] = [PlayerId::One, PlayerId::Two];

    pub const fn index(self) -> usize {
        match self {
            PlayerId::One => 0,
            PlayerId::Two => 1,
        }
    }

    /// The other player. Only meaningful because the slice is two-player.
    pub const fn other(self) -> PlayerId {
        match self {
            PlayerId::One => PlayerId::Two,
            PlayerId::Two => PlayerId::One,
        }
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

/// The two unit types in the slice.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum UnitKind {
    Villager,
    Soldier,
}

/// Combat and movement numbers for a unit type.
///
/// A plain struct rather than a match scattered through the systems, so that
/// loading these from RON later is a change to where the table comes from and
/// not a change to any system that reads it.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct UnitStats {
    pub max_hp: i32,
    /// Tiles per tick.
    pub speed: Fx,
    /// Zero for a unit that cannot attack.
    pub damage: i32,
    /// Attack reach in tiles, measured centre to centre.
    pub range: Fx,
    /// Ticks between attacks.
    pub attack_cooldown: u32,
    /// Collision radius used by local avoidance, in tiles.
    pub radius: Fx,
}

impl UnitKind {
    pub const fn stats(self) -> UnitStats {
        match self {
            // 2.5 tiles per second at 20 Hz.
            UnitKind::Villager => UnitStats {
                max_hp: 40,
                speed: Fx::from_frac(1, 8),
                damage: 0,
                range: Fx::ZERO,
                attack_cooldown: 0,
                radius: Fx::from_frac(1, 4),
            },
            UnitKind::Soldier => UnitStats {
                max_hp: 60,
                speed: Fx::from_frac(1, 8),
                damage: 8,
                range: Fx::from_frac(3, 2),
                attack_cooldown: 20,
                radius: Fx::from_frac(1, 4),
            },
        }
    }

    /// Gold cost to train.
    pub const fn cost(self) -> i32 {
        match self {
            UnitKind::Villager => 50,
            UnitKind::Soldier => 60,
        }
    }

    /// Ticks of training time.
    pub const fn train_ticks(self) -> u32 {
        match self {
            UnitKind::Villager => 100,
            UnitKind::Soldier => 120,
        }
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
    /// Gold held, up to [`Unit::CARRY_CAPACITY`].
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
    /// Gold a villager holds before it must walk back to a drop-off.
    pub const CARRY_CAPACITY: i32 = 10;
    /// Ticks per gold extracted: one gold every two seconds at 20 Hz.
    ///
    /// Whole gold on a tick counter rather than a fractional per-tick rate, so
    /// that gold is always an integer and the conservation property has
    /// nothing to round.
    pub const MINE_TICKS_PER_GOLD: u32 = 40;
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

    pub fn new(kind: UnitKind, owner: PlayerId, pos: FxVec2) -> Unit {
        Unit {
            kind,
            owner,
            pos,
            hp: kind.stats().max_hp,
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

/// The two building types in the slice.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum BuildingKind {
    TownCenter,
    Barracks,
}

impl BuildingKind {
    pub const fn max_hp(self) -> i32 {
        match self {
            BuildingKind::TownCenter => 1000,
            BuildingKind::Barracks => 600,
        }
    }

    /// Footprint in tiles, square.
    pub const fn size(self) -> i32 {
        match self {
            BuildingKind::TownCenter => 4,
            BuildingKind::Barracks => 3,
        }
    }

    /// Whether villagers may deposit gold here.
    pub const fn is_drop_off(self) -> bool {
        matches!(self, BuildingKind::TownCenter)
    }

    /// Whether this building can train `kind`.
    pub const fn trains(self, kind: UnitKind) -> bool {
        matches!(
            (self, kind),
            (BuildingKind::TownCenter, UnitKind::Villager)
                | (BuildingKind::Barracks, UnitKind::Soldier)
        )
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
    /// Units queued for training, oldest first.
    pub queue: Vec<UnitKind>,
    /// Ticks remaining on the unit at the head of the queue.
    pub train_ticks: u32,
}

impl Building {
    /// How many units may be waiting to train.
    pub const QUEUE_LIMIT: usize = 5;

    pub fn new(kind: BuildingKind, owner: PlayerId, origin: TilePos) -> Building {
        Building {
            kind,
            owner,
            origin,
            hp: kind.max_hp(),
            queue: Vec::new(),
            train_ticks: 0,
        }
    }

    /// The tiles this building covers.
    pub fn footprint(&self) -> impl Iterator<Item = TilePos> + '_ {
        let size = self.kind.size();
        let origin = self.origin;
        (0..size)
            .flat_map(move |dy| (0..size).map(move |dx| TilePos::new(origin.x + dx, origin.y + dy)))
    }

    /// Centre of the footprint in world space.
    pub fn center(&self) -> FxVec2 {
        let size = self.kind.size();
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
    pub gold: i32,
}
