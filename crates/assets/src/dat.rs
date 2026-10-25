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
    /// Where the random maps begin, and how many there are.
    pub random_maps_at: usize,
    pub random_maps: usize,
    /// Where the effects (the technologies' lists of changes) begin, and how
    /// many there are.
    pub effects_at: usize,
    pub effects: usize,
    /// Where the unit headers (each unit type's task list) begin, how many
    /// unit IDs they cover, and how many of those exist.
    pub unit_headers_at: usize,
    pub unit_headers: usize,
    pub units_present: usize,
    /// Where the civilisations begin, at their count, and how many there are.
    pub civs_at: usize,
    pub civs: usize,
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

    /// An `i32` count, which must not be negative.
    fn count32(&mut self) -> Result<usize, DatError> {
        usize::try_from(self.i32()?).map_err(|_| DatError::Corrupt("negative count"))
    }

    /// A fixed-size text field, up to its first zero byte.
    fn text(&mut self, size: usize) -> Result<String, DatError> {
        let field = self.bytes(size)?;
        let end = field.iter().position(|&b| b == 0).unwrap_or(size);
        Ok(String::from_utf8_lossy(&field[..end]).into_owned())
    }
}

impl Dat {
    /// Reads inflated data (see [`decompress`]) up to the civilisations.
    pub fn parse(data: &[u8]) -> Result<Dat, DatError> {
        version(data)?;
        let mut r = Reader { data, at: 8 };
        skip_terrain_restrictions(&mut r)?;
        // Player colours: nine 32-bit fields each.
        let colours = r.count()?;
        r.skip(colours * 36)?;
        skip_sounds(&mut r)?;
        let graphics = read_graphics(&mut r)?;
        let after_graphics = r.at;
        r.skip(TERRAIN_BLOCK_SIZE)?;
        let random_maps_at = r.at;
        let random_maps = skip_random_maps(&mut r)?;
        let effects_at = r.at;
        let effects = skip_effects(&mut r)?;
        let unit_headers_at = r.at;
        let (unit_headers, units_present) = skip_unit_headers(&mut r)?;
        let civs_at = r.at;
        let civs = r.count()?;
        Ok(Dat {
            graphics,
            after_graphics,
            random_maps_at,
            random_maps,
            effects_at,
            effects,
            unit_headers_at,
            unit_headers,
            units_present,
            civs_at,
            civs,
        })
    }
}

/// The Conquerors' terrain block, which holds no counts: its size is fixed.
/// A frame of map fields (two pointers and four sizes, 24 bytes), 19 tile
/// sizes of three `i16`, an `i16` of padding, 42 terrains, 16 borders, a
/// map row offset, six `f32` map bounds, 14 `i16` tile and block fields, two
/// pointers and three flag bytes, then 21 bytes and 157 `i32` the game fills
/// in at run time.
const TERRAIN_BLOCK_SIZE: usize = 24
    + 19 * 6
    + 2
    + 42 * TERRAIN_SIZE
    + 16 * BORDER_SIZE
    + 4
    + 6 * 4
    + 14 * 2
    + 4
    + 4
    + 3
    + 21
    + 157 * 4;
/// One terrain: flags, two 13-byte names, sprite, shape pointer, sound,
/// blend priority and type, colours, animation fields, 19
/// elevation graphics of three `i16`, the terrain to draw, its dimensions, a
/// border per terrain (42), 30 terrain units of ID, density, and centring,
/// their count, and a phantom `i16`.
const TERRAIN_SIZE: usize = 2
    + 26
    + 12
    + 8
    + 3
    + 2
    + 2
    + ANIMATION
    + 19 * 6
    + 2
    + 4
    + 42 * 2
    + 30 * 2
    + 30 * 2
    + 30
    + 2
    + 2;
/// One terrain border: flags, two 13-byte names, sprite, shape pointer,
/// sound, colours, animation fields, 19 times 12 frame records of three
/// `i16`, and three `i16`.
const BORDER_SIZE: usize = 2 + 26 + 12 + 3 + ANIMATION + 19 * 12 * 6 + 3 * 2;
/// A terrain's or border's animation fields: a flag, frame and pause
/// counts, interval and pause time, frame and draw frame, last animation
/// time, and two flags.
const ANIMATION: usize = 1 + 2 * 2 + 2 * 4 + 2 * 2 + 4 + 2;

/// The random maps: a count and a pointer, a header per map, then each map
/// again with its lists' entries. The two copies must agree on every list's
/// length. Returns the count.
fn skip_random_maps(r: &mut Reader<'_>) -> Result<usize, DatError> {
    /// Bytes per entry of each list: lands, terrains, units, elevations.
    const ENTRY: [usize; 4] = [44, 24, 44, 24];
    let maps = r.count32()?;
    r.skip(4)?; // pointer
                // Grown as headers are read rather than sized from the count, so a
                // damaged count runs out of file instead of memory.
    let mut lists = Vec::new();
    for _ in 0..maps {
        r.skip(4 + 9 * 4)?; // ID and nine map fields
        let mut counts = [0; 4];
        for count in &mut counts {
            *count = r.count32()?;
            r.skip(4)?; // pointer
        }
        lists.push(counts);
    }
    for counts in lists {
        r.skip(9 * 4)?;
        for (expected, size) in counts.into_iter().zip(ENTRY) {
            if r.count32()? != expected {
                return Err(DatError::Corrupt("random map lists differ between copies"));
            }
            r.skip(4)?;
            r.skip(expected.checked_mul(size).ok_or(DatError::Truncated)?)?;
        }
    }
    Ok(maps)
}

/// Effects: per effect a 31-byte name (The Conquerors' file stores
/// `CHUN4` in every one) and its commands: a type, three `i16`, and an `f32`.
fn skip_effects(r: &mut Reader<'_>) -> Result<usize, DatError> {
    let effects = r.count32()?;
    for _ in 0..effects {
        r.skip(31)?;
        let commands = r.count()?;
        r.skip(commands * 11)?;
    }
    Ok(effects)
}

/// Unit headers: per unit ID a flag and, when the unit exists, its task
/// list, 59 bytes a task. Returns the count and how many exist.
fn skip_unit_headers(r: &mut Reader<'_>) -> Result<(usize, usize), DatError> {
    let units = r.count32()?;
    let mut present = 0;
    for _ in 0..units {
        if r.u8()? != 0 {
            present += 1;
            let tasks = r.count()?;
            r.skip(tasks * 59)?;
        }
    }
    Ok((units, present))
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
