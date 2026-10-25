//! Reading content files.
//!
//! The file format is a mirror of `sim::Content` in units a person can write:
//! fixed-point numbers are fractions, `(1, 8)`, rather than raw 16.16 bit
//! patterns. Parsing, conversion, and validation all happen here, once, at
//! startup; the simulation only ever sees a checked `Content`.

use crate::looks::{Look, Looks, Part};
use serde::Deserialize;
use sim::{
    BuildingStats, Content, ContentError, Fx, GatherStats, Gathering, Resources, UnitKind,
    UnitStats,
};
use std::fmt;
use std::fs;
use std::path::Path;

/// The content the binary ships with, embedded so that it runs from any
/// working directory.
pub const DEFAULT_CONTENT: &str = include_str!("../content/default.ron");

/// Something went wrong reading a content file.
#[derive(Debug)]
pub enum ContentFileError {
    Io(std::io::Error),
    /// Not RON, a section or field missing, or a field the format does not
    /// have.
    Malformed(String),
    /// Well-formed, but holds a number the game cannot run with.
    Invalid(ContentError),
}

impl fmt::Display for ContentFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentFileError::Io(error) => write!(f, "content file: {error}"),
            ContentFileError::Malformed(detail) => write!(f, "content is not readable: {detail}"),
            ContentFileError::Invalid(error) => write!(f, "content is invalid: {error}"),
        }
    }
}

impl std::error::Error for ContentFileError {}

impl From<std::io::Error> for ContentFileError {
    fn from(error: std::io::Error) -> ContentFileError {
        ContentFileError::Io(error)
    }
}

impl From<ContentError> for ContentFileError {
    fn from(error: ContentError) -> ContentFileError {
        ContentFileError::Invalid(error)
    }
}

/// Parses, converts, and validates content from RON text.
pub fn parse_content(text: &str) -> Result<Content, ContentFileError> {
    let file: ContentFile =
        ron::from_str(text).map_err(|error| ContentFileError::Malformed(error.to_string()))?;
    let content = file.into_content()?;
    content.validate()?;
    Ok(content)
}

/// Reads a content file from disk. See [`parse_content`].
pub fn load_content(path: &Path) -> Result<Content, ContentFileError> {
    parse_content(&fs::read_to_string(path)?)
}

/// How each type in RON text looks, by name. The same file as
/// [`parse_content`] reads: a content file says both how a type plays and
/// how it looks, and the two are kept apart once read (see `looks`).
pub fn parse_looks(text: &str) -> Result<Looks, ContentFileError> {
    let file: ContentFile =
        ron::from_str(text).map_err(|error| ContentFileError::Malformed(error.to_string()))?;
    Ok(file.looks())
}

/// How each type in the built-in content looks.
pub fn default_looks() -> Looks {
    parse_looks(DEFAULT_CONTENT).expect("the built-in content parses")
}

/// How the game draws each type: from `path` if given, otherwise the
/// built-in file. The same in a new game and in playback, which draws a
/// replay's types with these looks, matched by name.
pub fn game_looks(path: Option<&Path>) -> Result<Looks, ContentFileError> {
    match path {
        Some(path) => parse_looks(&fs::read_to_string(path)?),
        None => Ok(default_looks()),
    }
}

/// The content a new game starts with: `path` if given, otherwise the
/// built-in file. `None` during playback, which uses the content the replay
/// was recorded with.
pub fn new_game_content(
    playing: bool,
    path: Option<&Path>,
) -> Result<Option<Content>, ContentFileError> {
    if playing {
        return Ok(None);
    }
    let content = match path {
        Some(path) => load_content(path)?,
        None => parse_content(DEFAULT_CONTENT)?,
    };
    Ok(Some(content))
}

/// `(numerator, denominator)`.
type Fraction = (i32, i32);

/// `(slp, x, y)`: a part's SLP and its offset in pixels.
type PartFile = (u32, i32, i32);

fn parts(file: Vec<PartFile>) -> Vec<Part> {
    file.into_iter()
        .map(|(slp, x, y)| Part::new(slp, x, y))
        .collect()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContentFile {
    units: Vec<UnitFile>,
    buildings: Vec<BuildingFile>,
    gather: GatherFile,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitFile {
    name: String,
    sprite: u32,
    parts: Vec<PartFile>,
    icon: u32,
    gathers: bool,
    max_hp: i32,
    speed: Fraction,
    damage: i32,
    range: Fraction,
    attack_cooldown: u32,
    radius: Fraction,
    cost: ResourcesFile,
    train_ticks: u32,
}

/// `(food: 50)`: a resource left out is zero.
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct ResourcesFile {
    food: i32,
    wood: i32,
    gold: i32,
    stone: i32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildingFile {
    name: String,
    sprite: u32,
    parts: Vec<PartFile>,
    icon: u32,
    size: i32,
    max_hp: i32,
    drop_off: bool,
    /// Unit types by name; turned into IDs when loaded.
    trains: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GatherFile {
    food: GatherStatsFile,
    wood: GatherStatsFile,
    gold: GatherStatsFile,
    stone: GatherStatsFile,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GatherStatsFile {
    carry_capacity: i32,
    ticks_per_unit: u32,
}

impl GatherStatsFile {
    fn stats(&self) -> GatherStats {
        GatherStats {
            carry_capacity: self.carry_capacity,
            ticks_per_unit: self.ticks_per_unit,
        }
    }
}

impl ContentFile {
    fn looks(&self) -> Looks {
        let look = |sprite: u32, file: &[PartFile], icon: u32| Look {
            sprite,
            parts: parts(file.to_vec()),
            icon,
        };
        Looks {
            units: self
                .units
                .iter()
                .map(|u| (u.name.clone(), look(u.sprite, &u.parts, u.icon)))
                .collect(),
            buildings: self
                .buildings
                .iter()
                .map(|b| (b.name.clone(), look(b.sprite, &b.parts, b.icon)))
                .collect(),
        }
    }

    fn into_content(self) -> Result<Content, ContentError> {
        let units = self
            .units
            .into_iter()
            .map(UnitFile::into_stats)
            .collect::<Result<Vec<_>, _>>()?;
        // A unit type's ID is its place in the list, so a name resolves to
        // the position of the first entry with it. Duplicate names are caught
        // by `Content::validate` afterwards.
        let kind_named = |name: &str| {
            units
                .iter()
                .position(|u| u.name == name)
                .map(|index| UnitKind::new(index as u16))
        };
        let buildings = self
            .buildings
            .into_iter()
            .map(|b| b.into_stats(&kind_named))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Content {
            units,
            buildings,
            gather: Gathering {
                food: self.gather.food.stats(),
                wood: self.gather.wood.stats(),
                gold: self.gather.gold.stats(),
                stone: self.gather.stone.stats(),
            },
        })
    }
}

impl UnitFile {
    fn into_stats(self) -> Result<UnitStats, ContentError> {
        let owner = self.name.as_str();
        Ok(UnitStats {
            speed: fraction(self.speed, owner, "speed")?,
            range: fraction(self.range, owner, "range")?,
            radius: fraction(self.radius, owner, "radius")?,
            max_hp: self.max_hp,
            damage: self.damage,
            attack_cooldown: self.attack_cooldown,
            cost: Resources {
                food: self.cost.food,
                wood: self.cost.wood,
                gold: self.cost.gold,
                stone: self.cost.stone,
            },
            train_ticks: self.train_ticks,
            gathers: self.gathers,
            name: self.name,
        })
    }
}

impl BuildingFile {
    fn into_stats(
        self,
        kind_named: &impl Fn(&str) -> Option<UnitKind>,
    ) -> Result<BuildingStats, ContentError> {
        let trains = self
            .trains
            .iter()
            .map(|name| {
                kind_named(name).ok_or_else(|| ContentError {
                    field: format!("{}.trains", self.name),
                    problem: "names a unit type the content does not have",
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(BuildingStats {
            name: self.name,
            size: self.size,
            max_hp: self.max_hp,
            drop_off: self.drop_off,
            trains,
        })
    }
}

/// Converts a written fraction to fixed point.
///
/// `Fx::from_frac` panics on a zero denominator and silently wraps a result
/// too large for 16.16, so both are checked here first: a content file is
/// user input and must not be able to do either.
fn fraction((num, den): Fraction, owner: &str, name: &str) -> Result<Fx, ContentError> {
    let error = |problem| ContentError {
        field: format!("{owner}.{name}"),
        problem,
    };
    if den <= 0 {
        return Err(error("denominator must be positive"));
    }
    let bits = (i64::from(num) << 16) / i64::from(den);
    if bits <= i64::from(i32::MIN) || bits > i64::from(i32::MAX) {
        return Err(error("too large for fixed point"));
    }
    Ok(Fx::from_frac(num, den))
}
