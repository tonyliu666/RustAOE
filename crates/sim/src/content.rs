//! The numbers the game plays by.
//!
//! Loaded once by the application, from RON, and handed to the world. Systems
//! read from here rather than from matches on the entity kind, so that a
//! balance change is a data change. Parsing text is the application's job:
//! this module holds plain structs and never touches the filesystem.

use crate::{BuildingKind, Fx, Resource, Resources, UnitKind};
use serde::{Deserialize, Serialize};

/// Every table the simulation reads.
///
/// One named field per kind rather than a map, so that a content file missing
/// a kind fails to load instead of failing mid-game.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Content {
    pub villager: UnitStats,
    pub soldier: UnitStats,
    pub town_center: BuildingStats,
    pub barracks: BuildingStats,
    pub gather: GatherStats,
}

/// Combat, movement, and training numbers for a unit type.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
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
    /// Charged when the unit is queued.
    pub cost: Resources,
    /// Ticks of training time.
    pub train_ticks: u32,
}

/// Numbers for a building type.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BuildingStats {
    pub max_hp: i32,
    /// Whether villagers may deposit gold here.
    pub drop_off: bool,
    /// Unit types this building can train.
    pub trains: Vec<UnitKind>,
}

/// Numbers for the gather loop.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct GatherStats {
    /// Gold a villager holds before it must walk back to a drop-off.
    pub carry_capacity: i32,
    /// Ticks per gold extracted.
    ///
    /// Whole gold on a tick counter rather than a fractional per-tick rate, so
    /// that gold is always an integer and the conservation property has
    /// nothing to round.
    pub ticks_per_gold: u32,
}

/// A content table holds a number the game cannot run with.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ContentError {
    /// Path to the field, such as `soldier.max_hp`.
    pub field: String,
    /// What is wrong with it, such as `must be positive`.
    pub problem: &'static str,
}

impl std::fmt::Display for ContentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.problem)
    }
}

impl std::error::Error for ContentError {}

const POSITIVE: &str = "must be positive";
const NOT_NEGATIVE: &str = "must not be negative";

/// Fails with `problem` on `owner.name` unless `ok`.
fn check(ok: bool, owner: &str, name: &str, problem: &'static str) -> Result<(), ContentError> {
    if ok {
        return Ok(());
    }
    Err(ContentError {
        field: format!("{owner}.{name}"),
        problem,
    })
}

impl UnitStats {
    fn validate(&self, owner: &str) -> Result<(), ContentError> {
        check(self.max_hp > 0, owner, "max_hp", POSITIVE)?;
        check(self.speed > Fx::ZERO, owner, "speed", POSITIVE)?;
        // A step is never shortened to the distance left, so a step wider
        // than the arrival window (`Unit::ARRIVE_RADIUS` either side, 1.5
        // tiles) overshoots forever, and a step over one tile can hop a
        // one-tile wall because only the landing point is tested. Half a
        // tile is well inside both, and four times the default.
        check(
            self.speed <= Fx::from_frac(1, 2),
            owner,
            "speed",
            "must be at most 1/2 tile per tick",
        )?;
        check(self.damage >= 0, owner, "damage", NOT_NEGATIVE)?;
        check(self.range >= Fx::ZERO, owner, "range", NOT_NEGATIVE)?;
        // A zero cooldown on an armed unit is a hit every tick: no rate limit.
        check(
            self.damage == 0 || self.attack_cooldown > 0,
            owner,
            "attack_cooldown",
            "must be positive when damage is positive",
        )?;
        check(self.radius > Fx::ZERO, owner, "radius", POSITIVE)?;
        // A negative cost would pay the player for queueing a unit.
        for resource in Resource::ALL {
            let name = format!("cost.{}", resource.name());
            check(self.cost.get(resource) >= 0, owner, &name, NOT_NEGATIVE)?;
        }
        check(self.train_ticks > 0, owner, "train_ticks", POSITIVE)
    }
}

impl BuildingStats {
    fn validate(&self, owner: &str) -> Result<(), ContentError> {
        check(self.max_hp > 0, owner, "max_hp", POSITIVE)
    }
}

impl GatherStats {
    fn validate(&self) -> Result<(), ContentError> {
        check(
            self.carry_capacity > 0,
            "gather",
            "carry_capacity",
            POSITIVE,
        )?;
        check(
            self.ticks_per_gold > 0,
            "gather",
            "ticks_per_gold",
            POSITIVE,
        )
    }
}

impl Content {
    /// Checks every number once, at load time.
    ///
    /// Reports the first problem in field order. The simulation trusts
    /// content that passed here and does not check it again per tick.
    pub fn validate(&self) -> Result<(), ContentError> {
        self.villager.validate("villager")?;
        self.soldier.validate("soldier")?;
        self.town_center.validate("town_center")?;
        self.barracks.validate("barracks")?;
        self.gather.validate()
    }

    pub const fn unit(&self, kind: UnitKind) -> &UnitStats {
        match kind {
            UnitKind::Villager => &self.villager,
            UnitKind::Soldier => &self.soldier,
        }
    }

    pub const fn building(&self, kind: BuildingKind) -> &BuildingStats {
        match kind {
            BuildingKind::TownCenter => &self.town_center,
            BuildingKind::Barracks => &self.barracks,
        }
    }
}

/// The proof-of-concept spec's numbers.
///
/// Every replay recorded before content became data was played with these, so
/// changing them changes the outcome of those replays.
impl Default for Content {
    fn default() -> Content {
        Content {
            // 2.5 tiles per second at 20 Hz.
            villager: UnitStats {
                max_hp: 40,
                speed: Fx::from_frac(1, 8),
                damage: 0,
                range: Fx::ZERO,
                attack_cooldown: 0,
                radius: Fx::from_frac(1, 4),
                cost: Resources {
                    gold: 50,
                    ..Resources::default()
                },
                train_ticks: 100,
            },
            soldier: UnitStats {
                max_hp: 60,
                speed: Fx::from_frac(1, 8),
                damage: 8,
                range: Fx::from_frac(3, 2),
                attack_cooldown: 20,
                radius: Fx::from_frac(1, 4),
                cost: Resources {
                    gold: 60,
                    ..Resources::default()
                },
                train_ticks: 120,
            },
            town_center: BuildingStats {
                max_hp: 1000,
                drop_off: true,
                trains: vec![UnitKind::Villager],
            },
            barracks: BuildingStats {
                max_hp: 600,
                drop_off: false,
                trains: vec![UnitKind::Soldier],
            },
            // One gold every two seconds at 20 Hz, ten to a load.
            gather: GatherStats {
                carry_capacity: 10,
                ticks_per_gold: 40,
            },
        }
    }
}
