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
    /// Each unit ID's tasks, shared by every civilisation; `None` where the
    /// ID has no unit.
    pub unit_tasks: Vec<Option<Vec<Task>>>,
    /// Where the civilisations begin, at their count, and each of them.
    pub civs_at: usize,
    pub civs: Vec<Civ>,
    /// Where the researches begin, at their count, and how many there are.
    pub researches_at: usize,
    pub researches: usize,
}

/// One task a unit can do: gather, build, attack, and so on.
#[derive(Clone, PartialEq, Debug)]
pub struct Task {
    /// What the task does; 5 is gathering.
    pub action: i16,
    /// The unit class and unit the task applies to, if limited to one.
    pub class: i16,
    pub unit: i16,
    /// The resource taken and the one carried: a gatherer takes a site's
    /// resource and carries its own kind.
    pub resource_in: i16,
    pub resource_out: i16,
    /// The task's rate, such as resource gathered per second.
    pub work_value: f32,
    pub work_range: f32,
}

/// One civilisation and its copy of every unit.
#[derive(Clone, PartialEq, Debug)]
pub struct Civ {
    pub name: String,
    /// Indexed by unit ID; `None` where the civilisation has no such unit.
    pub units: Vec<Option<Unit>>,
}

/// A unit, as far as the reader keeps it. Which parts it has depends on its
/// type: 10 and 15 are scenery, 20 and 25 flags, 30 dead units and fish, 40
/// birds (the first that work), 50 fighters, 60 projectiles, 70 trainable
/// units, 80 buildings, 90 trees.
#[derive(Clone, PartialEq, Debug)]
pub struct Unit {
    pub kind: u8,
    pub id: i16,
    pub name: String,
    pub class: i16,
    /// A graphic ID: see [`Dat::slp_of`].
    pub standing_graphic: i16,
    pub hit_points: i16,
    /// Half-sizes in tiles, x, y, and height.
    pub collision: [f32; 3],
    /// Half-sizes in tiles of the space kept clear, x and y.
    pub clearance: [f32; 2],
    /// A frame of the unit or building icon sprite.
    pub icon: i16,
    /// How much of a resource it carries.
    pub resource_capacity: i16,
    /// Tiles per second; types 20 and up.
    pub speed: Option<f32>,
    /// Types 40 and up.
    pub work_rate: Option<f32>,
    /// The units it may drop resources at; types 40 and up.
    pub drop_sites: Option<[i16; 2]>,
    /// Types 50 and up.
    pub combat: Option<Combat>,
    /// Types 70 and 80.
    pub creatable: Option<Creatable>,
    /// A building's annexes: further units drawn as parts of it, such as
    /// the town centre's sections. Empty for every other unit.
    pub annexes: Vec<Annex>,
}

/// A unit drawn as part of a building, offset from its centre.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Annex {
    pub unit: i16,
    /// In tiles, x and y.
    pub offset: [f32; 2],
}

/// How a unit fights.
#[derive(Clone, PartialEq, Debug)]
pub struct Combat {
    /// (armour class, amount) for each attack.
    pub attacks: Vec<(i16, i16)>,
    /// In tiles.
    pub max_range: f32,
    /// Seconds between attacks.
    pub reload_time: f32,
}

/// How a unit is made.
#[derive(Clone, PartialEq, Debug)]
pub struct Creatable {
    pub costs: [Cost; 3],
    /// Seconds.
    pub train_time: i16,
    /// The unit ID of the building that trains it.
    pub train_location: i16,
    /// Its button on that building's command panel.
    pub button: u8,
}

/// One part of a cost.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Cost {
    /// The resource: 0 food, 1 wood, 2 stone, 3 gold, 4 population.
    pub resource: i16,
    pub amount: i16,
    /// Whether this part is charged at all.
    pub used: bool,
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

    fn f32(&mut self) -> Result<f32, DatError> {
        let b = self.bytes(4)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
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
        let unit_tasks = read_unit_headers(&mut r)?;
        let unit_headers = unit_tasks.len();
        let units_present = unit_tasks.iter().flatten().count();
        let civs_at = r.at;
        let civ_count = r.count()?;
        // Each civilisation holds a slot per unit ID, most of them empty and
        // four bytes of file each, so the count is held to what any version
        // of the game has.
        if civ_count > MAX_CIVS {
            return Err(DatError::Corrupt("more civilisations than any version has"));
        }
        let civs = (0..civ_count)
            .map(|_| read_civ(&mut r))
            .collect::<Result<Vec<_>, _>>()?;
        let researches_at = r.at;
        let researches = r.count()?;
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
            unit_tasks,
            civs_at,
            civs,
            researches_at,
            researches,
        })
    }

    /// The SLP sprite a graphic draws, if the graphic exists and names one.
    pub fn slp_of(&self, graphic: i16) -> Option<i32> {
        let index = usize::try_from(graphic).ok()?;
        let slp = self.graphics.get(index)?.as_ref()?.slp;
        (slp >= 0).then_some(slp)
    }
}

/// Unit IDs are `i16`: no file has more unit headers than this.
const MAX_UNIT_IDS: usize = 1 << 15;
/// More civilisations than any version of the game has (The Conquerors has
/// 19, Gaia included).
const MAX_CIVS: usize = 64;

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
                // damaged count runs out of file instead of memory: each entry is
                // smaller than the 72 bytes of file it is read from.
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
/// list, 59 bytes a task.
fn read_unit_headers(r: &mut Reader<'_>) -> Result<Vec<Option<Vec<Task>>>, DatError> {
    let units = r.count32()?;
    // An empty header is one byte of file but a whole entry in the list, so
    // running out of file is not enough of a limit: the count must also fit
    // the unit IDs, which are `i16`.
    if units > MAX_UNIT_IDS {
        return Err(DatError::Corrupt("more unit headers than unit IDs"));
    }
    let mut headers = Vec::new();
    for _ in 0..units {
        if r.u8()? == 0 {
            headers.push(None);
            continue;
        }
        let tasks = r.count()?;
        let mut list = Vec::new();
        for _ in 0..tasks {
            list.push(read_task(r)?);
        }
        headers.push(Some(list));
    }
    Ok(headers)
}

fn read_task(r: &mut Reader<'_>) -> Result<Task, DatError> {
    r.skip(2 + 2 + 1)?; // task type, ID, default flag
    let action = r.i16()?;
    let class = r.i16()?;
    let unit = r.i16()?;
    r.skip(2)?; // terrain
    let resource_in = r.i16()?;
    r.skip(2)?; // resource multiplier
    let resource_out = r.i16()?;
    r.skip(2)?; // unused resource
    let work_value = r.f32()?;
    r.skip(4)?; // second work value
    let work_range = r.f32()?;
    // Search, targeting, gather type, diplomacy, carry and construction
    // flags, four graphics, and two sounds.
    r.skip(1 + 4 + 1 + 1 + 2 + 2 + 1 + 1 + 1 + 4 * 2 + 2 * 2)?;
    Ok(Task {
        action,
        class,
        unit,
        resource_in,
        resource_out,
        work_value,
        work_range,
    })
}

/// A civilisation: player type, 20-byte name, its starting resources, tech
/// tree and team bonus, icon set, then a pointer per unit slot and each
/// unit whose pointer is not zero.
fn read_civ(r: &mut Reader<'_>) -> Result<Civ, DatError> {
    r.skip(1)?; // player type
    let name = r.text(20)?;
    let resources = r.count()?;
    r.skip(2 + 2)?; // tech tree, team bonus
    r.skip(resources * 4)?;
    r.skip(1)?; // icon set
    let slots = r.count()?;
    let mut present = Vec::new();
    for _ in 0..slots {
        present.push(r.i32()? != 0);
    }
    let units = present
        .into_iter()
        .map(|here| {
            if here {
                read_unit(r).map(Some)
            } else {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Civ { name, units })
}

/// One unit in The Conquerors' layout: the fields every unit has, then the
/// parts its type adds, each type including the parts of the types below it.
fn read_unit(r: &mut Reader<'_>) -> Result<Unit, DatError> {
    let kind = r.u8()?;
    if ![10, 15, 20, 25, 30, 40, 50, 60, 70, 80, 90].contains(&kind) {
        return Err(DatError::Corrupt("unknown unit type"));
    }
    let name_len = r.count()?;
    let id = r.i16()?;
    r.skip(2 + 2)?; // language strings
    let class = r.i16()?;
    let standing_graphic = r.i16()?;
    r.skip(2 + 2 + 2 + 1)?; // second standing graphic, dying, undead, undead mode
    let hit_points = r.i16()?;
    r.skip(4 + 1)?; // line of sight, garrison capacity
    let collision = [r.f32()?, r.f32()?, r.f32()?];
    r.skip(2 + 2 + 2 + 1 + 1)?; // train and damage sounds, dead unit, sort, built on
    let icon = r.i16()?;
    r.skip(1 + 2 + 1 + 1)?; // hidden, portrait, enabled, disabled
    r.skip(4 * 2)?; // placement side and placement terrains
    let clearance = [r.f32()?, r.f32()?];
    r.skip(1 + 1 + 2 + 1)?; // hill mode, fog, terrain restriction, fly mode
    let resource_capacity = r.i16()?;
    // Resource decay, blast defence, combat level, interaction, minimap
    // mode, interface kind, attribute mode, minimap colour.
    r.skip(4 + 1 + 1 + 1 + 1 + 1 + 4 + 1)?;
    // Help and hotkey strings, hotkey.
    r.skip(4 * 3)?;
    // Recyclable, auto gather, doppelganger, gather group, occlusion,
    // obstruction type and class, trait, civilisation, an i16, selection
    // effect and colour, outline size.
    r.skip(4 + 1 + 2 + 2 + 2 + 2 + 3 * 4)?;
    r.skip(3 * (2 + 4 + 1))?; // resource storages
    let damage_graphics = usize::from(r.u8()?);
    r.skip(damage_graphics * 5)?;
    r.skip(2 + 2 + 1 + 1)?; // selection and dying sounds, attack reaction, convert terrain
    let name = r.text(name_len)?;
    r.skip(2 + 2)?; // copy and base IDs

    let mut unit = Unit {
        kind,
        id,
        name,
        class,
        standing_graphic,
        hit_points,
        collision,
        clearance,
        icon,
        resource_capacity,
        speed: None,
        work_rate: None,
        drop_sites: None,
        combat: None,
        creatable: None,
        annexes: Vec::new(),
    };
    if kind < 20 || kind == 90 {
        return Ok(unit);
    }
    unit.speed = Some(r.f32()?);
    if kind < 30 {
        return Ok(unit);
    }
    // Walking and running graphics, rotation, size class, tracking unit,
    // mode, and density, move algorithm, then five turning fields.
    r.skip(2 + 2 + 4 + 1 + 2 + 1 + 4 + 1 + 5 * 4)?;
    if kind < 40 {
        return Ok(unit);
    }
    r.skip(2 + 4)?; // default task, search radius
    unit.work_rate = Some(r.f32()?);
    unit.drop_sites = Some([r.i16()?, r.i16()?]);
    r.skip(1 + 2 + 2 + 1)?; // task swap group, attack and move sounds, run pattern
    if kind < 50 {
        return Ok(unit);
    }
    r.skip(2)?; // base armour
    let attack_count = r.count()?;
    let mut attacks = Vec::new();
    for _ in 0..attack_count {
        attacks.push((r.i16()?, r.i16()?));
    }
    let armours = r.count()?;
    r.skip(armours * 4)?;
    r.skip(2)?; // terrain defence bonus
    let max_range = r.f32()?;
    r.skip(4)?; // blast width
    let reload_time = r.f32()?;
    // Projectile unit, accuracy, break-off, frame delay, graphic
    // displacement, blast level, minimum range, dispersion, attack graphic,
    // and the displayed armour, attack, range, and reload time.
    r.skip(2 + 2 + 1 + 2 + 3 * 4 + 1 + 4 + 4 + 2 + 2 + 2 + 4 + 4)?;
    unit.combat = Some(Combat {
        attacks,
        max_range,
        reload_time,
    });
    if kind == 60 {
        r.skip(5 + 4)?; // projectile type and modes, arc
    }
    if kind < 70 || kind == 90 {
        return Ok(unit);
    }
    let mut costs = [Cost {
        resource: -1,
        amount: 0,
        used: false,
    }; 3];
    for cost in &mut costs {
        *cost = Cost {
            resource: r.i16()?,
            amount: r.i16()?,
            used: r.i16()? != 0,
        };
    }
    let train_time = r.i16()?;
    let train_location = r.i16()?;
    let button = r.u8()?;
    // Rear and flank attack, creatable type, hero mode, garrison graphic,
    // projectile count and limit, spawning area, secondary projectile,
    // special graphic and ability, displayed pierce armour.
    r.skip(4 + 4 + 1 + 1 + 4 + 4 + 1 + 3 * 4 + 4 + 4 + 1 + 2)?;
    unit.creatable = Some(Creatable {
        costs,
        train_time,
        train_location,
        button,
    });
    if kind == 80 {
        // Construction and snow graphics, adjacent mode, graphics angle,
        // disappears, stack unit, foundation, overlay, tech, can burn.
        r.skip(2 + 2 + 1 + 2 + 1 + 2 + 2 + 2 + 2 + 1)?;
        for _ in 0..4 {
            let annex = r.i16()?;
            let offset = [r.f32()?, r.f32()?];
            if annex >= 0 {
                unit.annexes.push(Annex {
                    unit: annex,
                    offset,
                });
            }
        }
        // Head and transform units, transform and construction sounds,
        // garrison type, heal and repair rates, pile unit, and the looting
        // table.
        r.skip(2 + 2 + 2 + 2 + 1 + 4 + 4 + 2 + 6)?;
    }
    Ok(unit)
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
