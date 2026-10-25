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
    pub gather: GatherStats,
}

/// Combat, movement, and training numbers for a unit type.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct UnitStats {
    /// Unique among unit types; how content files refer to this one.
    pub name: String,
    /// The sprite the application draws it with: an SLP ID in the
    /// original's `data/graphics.drs`. Not an entry in the original's `.dat`
    /// graphics table, which names an SLP plus extra pieces; the Phase 1.6
    /// converter resolves those to SLP IDs.
    pub sprite: u32,
    /// May take a gather order and carry a load.
    pub gathers: bool,
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
    /// Unique among building types.
    pub name: String,
    /// SLP ID in the original's `data/graphics.drs`, as for units.
    pub sprite: u32,
    /// Footprint side in tiles; the footprint is square.
    pub size: i32,
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
/// The largest building footprint, in tiles a side.
pub const MAX_BUILDING_SIZE: i32 = 16;
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
                    sprite: 1473,
                    gathers: true,
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
                // The militia's sprite.
                UnitStats {
                    name: "soldier".to_string(),
                    sprite: 1181,
                    gathers: false,
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
            ],
            buildings: vec![
                BuildingStats {
                    name: "town_center".to_string(),
                    sprite: 901,
                    size: 4,
                    max_hp: 1000,
                    drop_off: true,
                    trains: vec![UnitKind::VILLAGER],
                },
                BuildingStats {
                    name: "barracks".to_string(),
                    sprite: 131,
                    size: 3,
                    max_hp: 600,
                    drop_off: false,
                    trains: vec![UnitKind::SOLDIER],
                },
            ],
            // One gold every two seconds at 20 Hz, ten to a load.
            gather: GatherStats {
                carry_capacity: 10,
                ticks_per_gold: 40,
            },
        }
    }
}
