//! Reading content files.
//!
//! The file format is a mirror of `sim::Content` in units a person can write:
//! fixed-point numbers are fractions, `(1, 8)`, rather than raw 16.16 bit
//! patterns. Parsing, conversion, and validation all happen here, once, at
//! startup; the simulation only ever sees a checked `Content`.

use serde::Deserialize;
use sim::{BuildingStats, Content, ContentError, Fx, GatherStats, Resources, UnitKind, UnitStats};
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContentFile {
    villager: UnitFile,
    soldier: UnitFile,
    town_center: BuildingFile,
    barracks: BuildingFile,
    gather: GatherFile,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitFile {
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
    max_hp: i32,
    drop_off: bool,
    trains: Vec<UnitKind>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GatherFile {
    carry_capacity: i32,
    ticks_per_gold: u32,
}

impl ContentFile {
    fn into_content(self) -> Result<Content, ContentError> {
        Ok(Content {
            villager: self.villager.into_stats("villager")?,
            soldier: self.soldier.into_stats("soldier")?,
            town_center: self.town_center.into_stats(),
            barracks: self.barracks.into_stats(),
            gather: GatherStats {
                carry_capacity: self.gather.carry_capacity,
                ticks_per_gold: self.gather.ticks_per_gold,
            },
        })
    }
}

impl UnitFile {
    fn into_stats(self, owner: &str) -> Result<UnitStats, ContentError> {
        Ok(UnitStats {
            max_hp: self.max_hp,
            speed: fraction(self.speed, owner, "speed")?,
            damage: self.damage,
            range: fraction(self.range, owner, "range")?,
            attack_cooldown: self.attack_cooldown,
            radius: fraction(self.radius, owner, "radius")?,
            cost: Resources {
                food: self.cost.food,
                wood: self.cost.wood,
                gold: self.cost.gold,
                stone: self.cost.stone,
            },
            train_ticks: self.train_ticks,
        })
    }
}

impl BuildingFile {
    fn into_stats(self) -> BuildingStats {
        BuildingStats {
            max_hp: self.max_hp,
            drop_off: self.drop_off,
            trains: self.trains,
        }
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
