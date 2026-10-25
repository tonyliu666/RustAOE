//! Reading and writing replay files.
//!
//! File handling lives here rather than in `sim`, which never touches the
//! filesystem: a simulation that can fail on I/O is a simulation whose step
//! function can fail, and that is the door determinism bugs walk through.

use sim::{ContentError, MapError, Replay};
use std::fmt;
use std::fs;
use std::path::Path;

/// Something went wrong reading or writing a replay.
#[derive(Debug)]
pub enum ReplayError {
    Io(std::io::Error),
    /// The file is not a replay, or was written by an incompatible build.
    Malformed(String),
    /// A well-formed replay whose content holds a number the game cannot run
    /// with.
    Invalid(ContentError),
    /// Written in a replay format this build does not play.
    Version {
        found: u32,
        expected: u32,
    },
    /// A well-formed replay whose map cannot be played.
    InvalidMap(MapError),
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplayError::Io(error) => write!(f, "replay file: {error}"),
            ReplayError::Malformed(detail) => write!(f, "replay is not readable: {detail}"),
            ReplayError::Invalid(error) => write!(f, "replay content is invalid: {error}"),
            ReplayError::InvalidMap(error) => write!(f, "replay {error}"),
            ReplayError::Version { found, expected } => write!(
                f,
                "replay is format version {found}; this build plays version {expected}"
            ),
        }
    }
}

impl std::error::Error for ReplayError {}

impl From<std::io::Error> for ReplayError {
    fn from(error: std::io::Error) -> ReplayError {
        ReplayError::Io(error)
    }
}

/// Writes a replay as RON.
///
/// Text rather than a binary encoding: a replay is a bug report, and being
/// able to read one in a terminal is worth more than the bytes it costs.
pub fn save(path: &Path, replay: &Replay) -> Result<(), ReplayError> {
    let text = ron::ser::to_string_pretty(replay, ron::ser::PrettyConfig::default())
        .map_err(|error| ReplayError::Malformed(error.to_string()))?;
    fs::write(path, text)?;
    Ok(())
}

/// Reads a replay written by [`save`].
///
/// The content it carries is validated like a `--content` file: a replay is
/// a shared bug report, so it is user input, and unchecked numbers in it
/// could make playback misbehave instead of reporting an error.
pub fn load(path: &Path) -> Result<Replay, ReplayError> {
    let text = fs::read_to_string(path)?;
    let found = format_version(&text)?;
    if found != Replay::FORMAT_VERSION {
        return Err(ReplayError::Version {
            found,
            expected: Replay::FORMAT_VERSION,
        });
    }
    let replay: Replay =
        ron::from_str(&text).map_err(|error| ReplayError::Malformed(error.to_string()))?;
    replay.content.validate().map_err(ReplayError::Invalid)?;
    replay
        .map
        .validate_for(&replay.content)
        .map_err(ReplayError::InvalidMap)?;
    Ok(replay)
}

/// Only the version field of a replay file.
///
/// Read before the rest, so that a file from another build is refused by
/// version even when the rest of it is in a shape this build cannot parse.
/// Unknown fields are skipped, which is serde's default.
#[derive(serde::Deserialize)]
struct VersionProbe {
    /// Files written before versions existed have no field: version 1.
    #[serde(default = "version_one")]
    version: u32,
}

const fn version_one() -> u32 {
    1
}

fn format_version(text: &str) -> Result<u32, ReplayError> {
    let probe: VersionProbe =
        ron::from_str(text).map_err(|error| ReplayError::Malformed(error.to_string()))?;
    Ok(probe.version)
}
