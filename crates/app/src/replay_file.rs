//! Reading and writing replay files.
//!
//! File handling lives here rather than in `sim`, which never touches the
//! filesystem: a simulation that can fail on I/O is a simulation whose step
//! function can fail, and that is the door determinism bugs walk through.

use sim::{ContentError, Replay};
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
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplayError::Io(error) => write!(f, "replay file: {error}"),
            ReplayError::Malformed(detail) => write!(f, "replay is not readable: {detail}"),
            ReplayError::Invalid(error) => write!(f, "replay content is invalid: {error}"),
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
    let replay: Replay =
        ron::from_str(&text).map_err(|error| ReplayError::Malformed(error.to_string()))?;
    replay.content.validate().map_err(ReplayError::Invalid)?;
    Ok(replay)
}
