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
/// Unit and building types are entries in lists, indexed by [`UnitKind`]
/// and [`BuildingKind`], so that a new type is a new entry rather than new
/// code. Entries refer to each other by ID; the application's loader turns
/// the names in a content file into IDs.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Content {
    pub units: Vec<UnitStats>,
    pub buildings: Vec<BuildingStats>,
    pub gather: Gathering,
}

/// Combat, movement, and training numbers for a unit type.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct UnitStats {
    /// Unique among unit types; how content files refer to this one.
    pub name: String,
    /// May take a gather order and carry a load.
    pub gathers: bool,
    pub max_hp: i32,
    /// Tiles per tick.
    pub speed: Fx,
    /// Zero for a unit that cannot attack.
    pub damage: i32,
    /// Attack reach in tiles, measured between edges as the original
    /// measures it: 0 for melee, which hits what it touches.
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
    /// Unique among building types.
    pub name: String,
    /// Footprint side in tiles; the footprint is square.
    pub size: i32,
    pub max_hp: i32,
    /// The resources villagers may deposit here: in the original, the
    /// town centre takes all four, a mill food, a lumber camp wood, and a
    /// mining camp gold and stone.
    pub drop_off: Vec<Resource>,
    /// Unit types this building can train.
    pub trains: Vec<UnitKind>,
}

impl BuildingStats {
    /// Whether villagers may deposit `resource` here.
    pub fn takes(&self, resource: Resource) -> bool {
        self.drop_off.contains(&resource)
    }
}

/// How a villager gathers one resource: in the original, the job unit it
/// turns into (gold miner, lumberjack, and so on).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct GatherStats {
    /// How much a villager holds before it must walk back to a drop-off.
    pub carry_capacity: i32,
    /// Ticks per unit gathered.
    ///
    /// Whole units on a tick counter rather than a fractional per-tick rate,
    /// so that every amount is an integer and the conservation property has
    /// nothing to round.
    pub ticks_per_unit: u32,
}

/// The gather numbers for each resource.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Gathering {
    pub food: GatherStats,
    pub wood: GatherStats,
    pub gold: GatherStats,
    pub stone: GatherStats,
}

impl Gathering {
    /// The same numbers for every resource.
    pub const fn uniform(stats: GatherStats) -> Gathering {
        Gathering {
            food: stats,
            wood: stats,
            gold: stats,
            stone: stats,
        }
    }

    pub const fn get(&self, resource: Resource) -> &GatherStats {
        match resource {
            Resource::Food => &self.food,
            Resource::Wood => &self.wood,
            Resource::Gold => &self.gold,
            Resource::Stone => &self.stone,
        }
    }

    pub fn get_mut(&mut self, resource: Resource) -> &mut GatherStats {
        match resource {
            Resource::Food => &mut self.food,
            Resource::Wood => &mut self.wood,
            Resource::Gold => &mut self.gold,
            Resource::Stone => &mut self.stone,
        }
    }

    fn validate(&self) -> Result<(), ContentError> {
        for resource in Resource::ALL {
            self.get(resource)
                .validate(&format!("gather.{}", resource.name()))?;
        }
        Ok(())
    }
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
/// The largest building footprint, in tiles a side.
pub const MAX_BUILDING_SIZE: i32 = 16;
/// The longest attack range, in tiles: more than any of the original's
/// units has, and short enough that reach, compared squared in 64 bits, can
/// never overflow.
pub const MAX_RANGE: i32 = 64;
/// The widest unit radius, in tiles: half the largest building.
pub const MAX_RADIUS: i32 = MAX_BUILDING_SIZE / 2;
/// The most entries a type list may have: as many as a `u16` ID can name.
const MAX_TYPES: usize = u16::MAX as usize + 1;

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
        check(
            self.range <= Fx::from_int(MAX_RANGE),
            owner,
            "range",
            "must be at most 64 tiles",
        )?;
        // A zero cooldown on an armed unit is a hit every tick: no rate limit.
        check(
            self.damage == 0 || self.attack_cooldown > 0,
            owner,
            "attack_cooldown",
            "must be positive when damage is positive",
        )?;
        check(self.radius > Fx::ZERO, owner, "radius", POSITIVE)?;
        check(
            self.radius <= Fx::from_int(MAX_RADIUS),
            owner,
            "radius",
            "must be at most 8 tiles",
        )?;
        // A negative cost would pay the player for queueing a unit.
        for resource in Resource::ALL {
            let name = format!("cost.{}", resource.name());
            check(self.cost.get(resource) >= 0, owner, &name, NOT_NEGATIVE)?;
        }
        check(self.train_ticks > 0, owner, "train_ticks", POSITIVE)
    }
}

impl BuildingStats {
    fn validate(&self, owner: &str, unit_types: usize) -> Result<(), ContentError> {
        check(self.max_hp > 0, owner, "max_hp", POSITIVE)?;
        check(self.size > 0, owner, "size", POSITIVE)?;
        // The original's largest building, a wonder, is 5x5. Far larger sizes
        // overflow the footprint arithmetic, which would panic a debug build
        // and wrap a release build: the two must play the same game.
        check(
            self.size <= MAX_BUILDING_SIZE,
            owner,
            "size",
            "must be at most 16",
        )?;
        check(
            self.trains.iter().all(|kind| kind.index() < unit_types),
            owner,
            "trains",
            "must name unit types in this content",
        )
    }
}

impl GatherStats {
    fn validate(&self, owner: &str) -> Result<(), ContentError> {
        check(self.carry_capacity > 0, owner, "carry_capacity", POSITIVE)?;
        check(self.ticks_per_unit > 0, owner, "ticks_per_unit", POSITIVE)
    }
}

/// Fails unless every name in `names` is non-empty and unique; `list` is
/// the field the names sit in, for the error.
fn check_names<'a>(list: &str, names: impl Iterator<Item = &'a str>) -> Result<(), ContentError> {
    let mut seen = std::collections::HashSet::new();
    for (index, name) in names.enumerate() {
        let field = || format!("{list}[{index}].name");
        if name.is_empty() {
            return Err(ContentError {
                field: field(),
                problem: "must not be empty",
            });
        }
        if !seen.insert(name) {
            return Err(ContentError {
                field: field(),
                problem: "must be unique",
            });
        }
    }
    Ok(())
}

impl Content {
    /// Checks every number and reference once, at load time.
    ///
    /// Reports the first problem in field order, naming the field by its
    /// entry's name, such as `soldier.max_hp`. The simulation trusts content
    /// that passed here and does not check it again per tick.
    pub fn validate(&self) -> Result<(), ContentError> {
        if self.units.is_empty() {
            return Err(ContentError {
                field: "units".to_string(),
                problem: "must not be empty",
            });
        }
        // A type ID is a `u16`; more entries would wrap round to type 0.
        for (list, len) in [
            ("units", self.units.len()),
            ("buildings", self.buildings.len()),
        ] {
            if len > MAX_TYPES {
                return Err(ContentError {
                    field: list.to_string(),
                    problem: "must have at most 65536 entries",
                });
            }
        }
        check_names("units", self.units.iter().map(|u| u.name.as_str()))?;
        check_names("buildings", self.buildings.iter().map(|b| b.name.as_str()))?;
        for unit in &self.units {
            unit.validate(&unit.name)?;
        }
        for building in &self.buildings {
            building.validate(&building.name, self.units.len())?;
        }
        self.gather.validate()
    }

    /// The numbers for a unit type. Panics on an ID this content does not
    /// have, which validated content and the world never produce.
    pub fn unit(&self, kind: UnitKind) -> &UnitStats {
        &self.units[kind.index()]
    }

    pub fn unit_mut(&mut self, kind: UnitKind) -> &mut UnitStats {
        &mut self.units[kind.index()]
    }

    pub fn building(&self, kind: BuildingKind) -> &BuildingStats {
        &self.buildings[kind.index()]
    }

    pub fn building_mut(&mut self, kind: BuildingKind) -> &mut BuildingStats {
        &mut self.buildings[kind.index()]
    }

    /// The unit type called `name`.
    pub fn unit_named(&self, name: &str) -> Option<UnitKind> {
        self.units
            .iter()
            .position(|u| u.name == name)
            .map(|index| UnitKind::new(index as u16))
    }

    /// The building type called `name`.
    pub fn building_named(&self, name: &str) -> Option<BuildingKind> {
        self.buildings
            .iter()
            .position(|b| b.name == name)
            .map(|index| BuildingKind::new(index as u16))
    }

    /// Every unit type, with its ID.
    pub fn unit_kinds(&self) -> impl Iterator<Item = UnitKind> + '_ {
        (0..self.units.len()).map(|index| UnitKind::new(index as u16))
    }

    /// Every building type, with its ID.
    pub fn building_kinds(&self) -> impl Iterator<Item = BuildingKind> + '_ {
        (0..self.buildings.len()).map(|index| BuildingKind::new(index as u16))
    }
}

/// The proof-of-concept spec's numbers.
///
/// Every replay recorded before content became data was played with these, so
/// changing them changes the outcome of those replays.
impl Default for Content {
    fn default() -> Content {
        // The order is the IDs: `UnitKind::VILLAGER` is 0, `SOLDIER` is 1,
        // `BuildingKind::TOWN_CENTER` is 0, `BARRACKS` is 1.
        Content {
            units: vec![
                // 2.5 tiles per second at 20 Hz.
                UnitStats {
                    name: "villager".to_string(),
                    gathers: true,
                    max_hp: 40,
                    speed: Fx::from_frac(1, 8),
                    damage: 0,
                    range: Fx::ZERO,
                    attack_cooldown: 0,
                    radius: Fx::from_frac(1, 4),
                    // 50 food, as the original's .dat has it.
                    cost: Resources {
                        food: 50,
                        ..Resources::default()
                    },
                    train_ticks: 100,
                },
                UnitStats {
                    name: "soldier".to_string(),
                    gathers: false,
                    max_hp: 60,
                    speed: Fx::from_frac(1, 8),
                    damage: 8,
                    // The militia's: melee.
                    range: Fx::ZERO,
                    attack_cooldown: 20,
                    radius: Fx::from_frac(1, 4),
                    cost: Resources {
                        gold: 60,
                        ..Resources::default()
                    },
                    train_ticks: 120,
                },
            ],
            buildings: vec![
                BuildingStats {
                    name: "town_center".to_string(),
                    size: 4,
                    max_hp: 1000,
                    drop_off: Resource::ALL.to_vec(),
                    trains: vec![UnitKind::VILLAGER],
                },
                BuildingStats {
                    name: "barracks".to_string(),
                    size: 3,
                    max_hp: 600,
                    drop_off: Vec::new(),
                    trains: vec![UnitKind::SOLDIER],
                },
                // The drop-off camps, from the .dat (units 68, 562, 584):
                // two tiles a side, 1000 hit points, each taking what its
                // jobs bring.
                BuildingStats {
                    name: "mill".to_string(),
                    size: 2,
                    max_hp: 1000,
                    drop_off: vec![Resource::Food],
                    trains: Vec::new(),
                },
                BuildingStats {
                    name: "lumber_camp".to_string(),
                    size: 2,
                    max_hp: 1000,
                    drop_off: vec![Resource::Wood],
                    trains: Vec::new(),
                },
                BuildingStats {
                    name: "mining_camp".to_string(),
                    size: 2,
                    max_hp: 1000,
                    drop_off: vec![Resource::Gold, Resource::Stone],
                    trains: Vec::new(),
                },
            ],
            // One unit every two seconds at 20 Hz, ten to a load, for every
            // resource until Phase 2 step 5 adopts the original's numbers.
            gather: Gathering::uniform(GatherStats {
                carry_capacity: 10,
                ticks_per_unit: 40,
            }),
        }
    }
}
