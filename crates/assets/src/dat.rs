//! The original's game data file, `data/empires2_x1_p1.dat`.
//!
//! The file is raw deflate data (zlib's stream without its header). Inflated,
//! it starts with an 8-byte version string; The Conquerors writes `VER 5.7`.
//! The sections inside follow, in this order: terrain restrictions, player
//! colours, sounds, graphics, the terrain block, random maps, technology
//! effects, unit headers, civilisations with their units, then research and
//! the tech tree. The layouts are The Conquerors' (`VER 5.7`), as documented
//! by the genieutils project's readers for this file; each one here was
//! checked against the real file (`tests/dat.rs`).

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
    /// A value no real file holds, such as a negative count.
    Corrupt(&'static str),
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
            DatError::Corrupt(what) => write!(f, "the .dat is damaged: {what}"),
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

/// One graphic: a sprite and how to play it.
#[derive(Clone, PartialEq, Debug)]
pub struct Graphic {
    /// Its index in the graphics table.
    pub id: i16,
    pub name: String,
    /// The SLP in the archives that holds its frames; -1 for none.
    pub slp: i32,
    pub frame_count: i16,
    pub angle_count: i16,
    /// Further graphics drawn with this one, such as the parts of a town
    /// centre.
    pub deltas: Vec<GraphicDelta>,
}

/// A further graphic drawn as part of another, offset in pixels.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct GraphicDelta {
    pub graphic_id: i16,
    pub offset_x: i16,
    pub offset_y: i16,
}

/// The game data, read as far as this reader goes so far.
#[derive(Clone, PartialEq, Debug)]
pub struct Dat {
    /// Indexed by graphic ID; `None` where the table has no graphic.
    pub graphics: Vec<Option<Graphic>>,
    /// Where the section after the graphics table, the terrain block, begins.
    pub after_graphics: usize,
}

/// Reads little-endian fields from inflated data, failing with `Truncated`
/// rather than reading past the end.
struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn bytes(&mut self, n: usize) -> Result<&[u8], DatError> {
        let end = self.at.checked_add(n).ok_or(DatError::Truncated)?;
        let field = self.data.get(self.at..end).ok_or(DatError::Truncated)?;
        self.at = end;
        Ok(field)
    }

    fn skip(&mut self, n: usize) -> Result<(), DatError> {
        self.bytes(n).map(|_| ())
    }

    fn u8(&mut self) -> Result<u8, DatError> {
        Ok(self.bytes(1)?[0])
    }

    fn i16(&mut self) -> Result<i16, DatError> {
        let b = self.bytes(2)?;
        Ok(i16::from_le_bytes([b[0], b[1]]))
    }

    fn i32(&mut self) -> Result<i32, DatError> {
        let b = self.bytes(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// An `i16` count, which must not be negative.
    fn count(&mut self) -> Result<usize, DatError> {
        usize::try_from(self.i16()?).map_err(|_| DatError::Corrupt("negative count"))
    }

    /// A fixed-size text field, up to its first zero byte.
    fn text(&mut self, size: usize) -> Result<String, DatError> {
        let field = self.bytes(size)?;
        let end = field.iter().position(|&b| b == 0).unwrap_or(size);
        Ok(String::from_utf8_lossy(&field[..end]).into_owned())
    }
}

impl Dat {
    /// Reads inflated data (see [`decompress`]) up to the end of the graphics
    /// table.
    pub fn parse(data: &[u8]) -> Result<Dat, DatError> {
        version(data)?;
        let mut r = Reader { data, at: 8 };
        skip_terrain_restrictions(&mut r)?;
        // Player colours: nine 32-bit fields each.
        let colours = r.count()?;
        r.skip(colours * 36)?;
        skip_sounds(&mut r)?;
        let graphics = read_graphics(&mut r)?;
        Ok(Dat {
            graphics,
            after_graphics: r.at,
        })
    }
}

/// Terrain restrictions: two pointer tables, then per restriction a
/// multiplier and a pass graphic (four 32-bit fields) for each terrain.
fn skip_terrain_restrictions(r: &mut Reader<'_>) -> Result<(), DatError> {
    let restrictions = r.count()?;
    let terrains = r.count()?;
    r.skip(restrictions * 4 * 2)?;
    r.skip(restrictions * terrains * (4 + 16))
}

/// Sounds: ID, delay, file count, cache time, then per file a 13-byte name,
/// resource ID, probability, civilisation, and icon set.
fn skip_sounds(r: &mut Reader<'_>) -> Result<(), DatError> {
    let sounds = r.count()?;
    for _ in 0..sounds {
        r.skip(4)?;
        let files = r.count()?;
        r.skip(4)?;
        r.skip(files * (13 + 4 + 2 + 2 + 2))?;
    }
    Ok(())
}

/// The graphics table: a pointer per slot, then a graphic for every slot
/// whose pointer is not zero.
fn read_graphics(r: &mut Reader<'_>) -> Result<Vec<Option<Graphic>>, DatError> {
    let slots = r.count()?;
    let mut present = Vec::with_capacity(slots);
    for _ in 0..slots {
        present.push(r.i32()? != 0);
    }
    present
        .into_iter()
        .map(|here| {
            if here {
                read_graphic(r).map(Some)
            } else {
                Ok(None)
            }
        })
        .collect()
}

fn read_graphic(r: &mut Reader<'_>) -> Result<Graphic, DatError> {
    let name = r.text(21)?;
    r.skip(13)?; // file name
    let slp = r.i32()?;
    r.skip(1 + 1 + 1 + 2 + 1 + 8)?; // loaded, colour flag, layer, player colour, selection, coordinates
    let delta_count = r.count()?;
    r.skip(2)?; // sound
    let angle_sounds = r.u8()?;
    let frame_count = r.i16()?;
    let angle_count = r.i16()?;
    r.skip(4 * 3 + 1)?; // speed, frame duration, replay delay, sequence type
    let id = r.i16()?;
    r.skip(1 + 1)?; // mirroring, editor flag
    let mut deltas = Vec::with_capacity(delta_count);
    for _ in 0..delta_count {
        let graphic_id = r.i16()?;
        r.skip(2 + 4)?; // padding, sprite pointer
        let offset_x = r.i16()?;
        let offset_y = r.i16()?;
        r.skip(2 + 2)?; // display angle, padding
        deltas.push(GraphicDelta {
            graphic_id,
            offset_x,
            offset_y,
        });
    }
    if angle_sounds != 0 {
        let angles =
            usize::try_from(angle_count).map_err(|_| DatError::Corrupt("negative angle count"))?;
        r.skip(angles * 12)?;
    }
    Ok(Graphic {
        id,
        name,
        slp,
        frame_count,
        angle_count,
        deltas,
    })
}
