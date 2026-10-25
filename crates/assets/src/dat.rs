//! The original's game data file, `data/empires2_x1_p1.dat`.
//!
//! The file is raw deflate data (zlib's stream without its header). Inflated,
//! it starts with an 8-byte version string; The Conquerors writes `VER 5.7`.
//! This module reads the container; the sections inside come in later steps
//! of plan Phase 1.6.

use miniz_oxide::inflate::{decompress_to_vec_with_limit, TINFLStatus};
use std::fmt;

/// The largest inflated size accepted, in bytes. The real file inflates to a
/// few tens of megabytes; a damaged or hostile file that inflates without end
/// stops here instead of exhausting memory.
pub const MAX_DECOMPRESSED: usize = 256 << 20;

/// The only version this reader understands: The Conquerors.
pub const SUPPORTED_VERSION: &str = "VER 5.7";

/// Why bytes could not be read as the game's data file.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DatError {
    /// Not a raw deflate stream, or a damaged one.
    NotCompressed,
    /// Inflates to more than the limit.
    TooLarge,
    /// Shorter than the version string.
    Truncated,
    /// Does not start with `VER `.
    NotDat,
    /// A version this reader does not know, such as a later game's.
    UnsupportedVersion(String),
}

impl fmt::Display for DatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DatError::NotCompressed => write!(f, "not a compressed .dat file"),
            DatError::TooLarge => {
                write!(f, "the .dat inflates to more than {MAX_DECOMPRESSED} bytes")
            }
            DatError::Truncated => write!(f, "the .dat is truncated"),
            DatError::NotDat => write!(f, "not a game data file (no version string)"),
            DatError::UnsupportedVersion(version) => {
                write!(
                    f,
                    "data file version {version:?}; this reader understands {SUPPORTED_VERSION:?}"
                )
            }
        }
    }
}

impl std::error::Error for DatError {}

/// Inflates the file, up to [`MAX_DECOMPRESSED`] bytes.
pub fn decompress(bytes: &[u8]) -> Result<Vec<u8>, DatError> {
    decompress_with_limit(bytes, MAX_DECOMPRESSED)
}

/// Inflates the file, failing with [`DatError::TooLarge`] past `limit` bytes.
pub fn decompress_with_limit(bytes: &[u8], limit: usize) -> Result<Vec<u8>, DatError> {
    decompress_to_vec_with_limit(bytes, limit).map_err(|error| match error.status {
        TINFLStatus::HasMoreOutput => DatError::TooLarge,
        _ => DatError::NotCompressed,
    })
}

/// The version string at the start of inflated data, if this reader supports
/// it.
pub fn version(data: &[u8]) -> Result<&str, DatError> {
    let field = data.get(..8).ok_or(DatError::Truncated)?;
    if !field.starts_with(b"VER ") {
        return Err(DatError::NotDat);
    }
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    let text = std::str::from_utf8(&field[..end]).map_err(|_| DatError::NotDat)?;
    if text != SUPPORTED_VERSION {
        return Err(DatError::UnsupportedVersion(text.to_string()));
    }
    Ok(text)
}
