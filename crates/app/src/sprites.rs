//! The original game's sprites: loading them from an install and where to
//! draw them.
//!
//! Read from the user's own extracted installer at runtime (`--assets DIR`),
//! never committed. Decoding happens once at startup; drawing uses uploaded
//! textures. See `docs/superpowers/specs/2026-10-24-prestage-edge-scroll-and-real-sprites.md`
//! for how the sprite IDs below were chosen.

use crate::looks::{Looks, Part};
use crate::pick::{Bodies, Body};
use assets::{
    Drs, DrsError, Image, Palette, PaletteError, Slp, SlpError, ARCHIVE_ORDER, MAX_FRAME_SIDE,
};
use render::{Renderer, TextureId, Vec2};
use sim::{BuildingKind, Content, PlayerId, Resource, UnitKind};
use std::collections::HashMap;
use std::fmt;
use std::ops::Range;
use std::path::{Path, PathBuf};

/// Sprite pixels per tile for units. The original's tile is 97 pixels across
/// its diagonal, about 68 along a side; units are drawn a little larger than
/// that scale so that they read well on a top-down grid.
pub const UNIT_PIXELS_PER_TILE: f32 = 48.0;

/// The game palette, in `interfac.drs`.
const PALETTE: u32 = 50500;
/// Grass, in `terrain.drs`. Each frame is a variation of the same terrain.
const GRASS: u32 = 15001;
/// How many variations of a ground texture to load.
const GROUND_VARIATIONS: usize = 16;
/// Side of an unskewed terrain texture, in pixels.
const TILE_PIXELS: u32 = 32;
// Unit and building sprites come from each type's `sprite` in the content.
/// Farmland, in `terrain.drs`: a farm is drawn as ground, as grass is.
const FARM: u32 = 15004;
// Each kind of site is drawn as the original draws the unit it stands for:
// its graphic's pieces, each SLP's frame 0, in order, one over another.
/// The forest oak (unit 349), drawn for every tree and on every rock tile.
const TREE: [u32; 2] = [2296, 4652];
/// The gold mine (unit 66). Later frames are the mine running out.
const GOLD_MINE: [u32; 2] = [4479, 2561];
/// The stone mine (unit 102).
const STONE_MINE: [u32; 2] = [4482, 1034];
/// The forage bush (unit 59).
const BUSH: [u32; 1] = [2560];
/// Gaia plus eight players: one decoded copy of each sprite per player
/// colour.
const PLAYER_SLOTS: usize = 9;
/// The in-game interface for an 800x600 window, in `interfac.drs`: the top
/// bar and the bottom row of panels, with the middle left transparent for
/// the map. One image per civilisation; this is the first, used for every
/// player until civilisations exist.
const INTERFACE: u32 = 51101;
/// The unit icons, in `interfac.drs`: 36x36 frames, one per unit type; each
/// type's content entry names its frame.
const UNIT_ICONS: u32 = 50730;
/// The building icons for the first architecture, in `interfac.drs`, used
/// for every civilisation until civilisations exist.
const BUILDING_ICONS: u32 = 50705;
/// The progress bar, in `interfac.drs`: frame n is n percent full.
const PROGRESS_BAR: u32 = 50764;
/// The mouse cursors, in `interfac.drs`: the original's `mcursors`, one
/// sprite of 19 frames that the exe loads as a whole.
const CURSORS: u32 = 51000;
/// Which frame of [`CURSORS`] each of `Sprites::cursors` is: the arrow, then
/// the job cursor for food, wood, gold, and stone. Frame 8 is the axe; the
/// sprite has no pickaxe or sickle, and which frame the original shows for
/// the other jobs is not yet read from the exe, so they take frame 3, the
/// hand, provisionally.
const CURSOR_FRAMES: [usize; 5] = [0, 3, 8, 3, 3];

/// An uploaded sprite and where its ground point is.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Sprite {
    pub texture: TextureId,
    /// Size in sprite pixels.
    pub width: f32,
    pub height: f32,
    /// The pixel that stands on the ground point, from the top-left.
    pub hotspot: Vec2,
}

/// Every sprite the game draws, uploaded. Units and buildings have one
/// sprite per player slot (0 is Gaia), each in that player's colours.
#[derive(Clone, PartialEq, Debug)]
pub struct Sprites {
    pub grass: Vec<TextureId>,
    /// The farm ground's variations, one tile each.
    pub farm: Vec<TextureId>,
    pub tree: Sprite,
    pub gold_mine: Sprite,
    pub stone_mine: Sprite,
    pub bush: Sprite,
    /// The interface image, drawn stretched over the whole window.
    pub interface: Sprite,
    /// Each unit type's icon, indexed by type ID.
    pub unit_icons: Vec<Sprite>,
    /// Each building type's icon, indexed by type ID.
    pub building_icons: Vec<Sprite>,
    /// The progress bar, one sprite per frame: frame n is n percent full.
    pub progress: Vec<Sprite>,
    /// Job cursors, indexed by resource (default, food, wood, gold, stone).
    pub cursors: [Sprite; 5],
    /// Indexed by unit type ID, then player slot.
    pub units: Vec<[Sprite; PLAYER_SLOTS]>,
    /// Indexed by building type ID, then player slot.
    pub buildings: Vec<[Sprite; PLAYER_SLOTS]>,
}

impl Sprites {
    /// The sprite for a unit type in a player's colours. The sprites are
    /// loaded for the content the world plays by, so every type has one.
    pub fn unit(&self, kind: UnitKind, owner: PlayerId) -> &Sprite {
        &self.units[kind.index()][slot(owner)]
    }

    /// The icon for a unit type, as on the command panel's buttons.
    pub fn unit_icon(&self, kind: UnitKind) -> &Sprite {
        &self.unit_icons[kind.index()]
    }

    pub fn building_icon(&self, kind: BuildingKind) -> &Sprite {
        &self.building_icons[kind.index()]
    }

    pub fn building(&self, kind: BuildingKind, owner: PlayerId) -> &Sprite {
        &self.buildings[kind.index()][slot(owner)]
    }

    /// Each unit and building kind's drawn body, for picking: the same
    /// placement as drawing, so a click lands on what the player sees.
    /// Buildings are drawn scaled to their footprint, which `content` gives.
    pub fn bodies(&self, content: &Content) -> Bodies {
        let body = |sprite: &Sprite, scale: f32| Body {
            offset: Vec2::new(-sprite.hotspot.x, -sprite.hotspot.y) * scale,
            size: Vec2::new(sprite.width, sprite.height) * scale,
        };
        Bodies {
            units: self
                .units
                .iter()
                .map(|set| body(&set[1], 1.0 / UNIT_PIXELS_PER_TILE))
                .collect(),
            buildings: self
                .buildings
                .iter()
                .zip(&content.buildings)
                .map(|(set, stats)| body(&set[1], building_scale(&set[1], stats.size)))
                .collect(),
            sites: Resource::ALL
                .map(|resource| Some(body(self.site(resource), 1.0 / UNIT_PIXELS_PER_TILE))),
            ..Bodies::default()
        }
    }

    /// The picture of a site of `resource` other than a farm, which is
    /// drawn as ground.
    pub fn site(&self, resource: Resource) -> &Sprite {
        match resource {
            Resource::Food => &self.bush,
            Resource::Wood => &self.tree,
            Resource::Gold => &self.gold_mine,
            Resource::Stone => &self.stone_mine,
        }
    }

    /// The grass texture for a tile. Varies from tile to tile, the same way
    /// every frame, so the ground does not flicker.
    pub fn grass_at(&self, x: i32, y: i32) -> TextureId {
        self.grass[variation(x, y, self.grass.len())]
    }

    /// The farm ground texture for a tile, varied as [`Self::grass_at`] is.
    pub fn farm_at(&self, x: i32, y: i32) -> TextureId {
        self.farm[variation(x, y, self.farm.len())]
    }

    /// The cursor for a resource: 0=default, 1=food, 2=wood, 3=gold, 4=stone.
    pub fn cursor(&self, resource: Option<Resource>) -> &Sprite {
        let idx = match resource {
            None => 0,
            Some(Resource::Food) => 1,
            Some(Resource::Wood) => 2,
            Some(Resource::Gold) => 3,
            Some(Resource::Stone) => 4,
        };
        &self.cursors[idx]
    }
}

/// Which of `count` variations of a ground texture tile (x, y) gets.
fn variation(x: i32, y: i32, count: usize) -> usize {
    let hash = (x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663);
    hash as usize % count
}

/// Tiles per sprite pixel for a building: its sprite is drawn as wide as its
/// footprint.
pub(crate) fn building_scale(sprite: &Sprite, size: i32) -> f32 {
    size as f32 / sprite.width
}

fn slot(player: PlayerId) -> usize {
    usize::from(player.number()).min(PLAYER_SLOTS - 1)
}

/// A decoded frame and its hotspot, before upload.
#[derive(Clone, Debug)]
struct Frame {
    image: Image,
    hotspot: Vec2,
}

/// Every sprite decoded, before upload. Decoding needs no window, so the
/// slow part of loading can be tested and timed on its own.
#[derive(Clone, Debug)]
pub struct GameImages {
    grass: Vec<Image>,
    farm: Vec<Image>,
    tree: Frame,
    gold_mine: Frame,
    stone_mine: Frame,
    bush: Frame,
    interface: Frame,
    unit_icons: Vec<Frame>,
    building_icons: Vec<Frame>,
    progress: Vec<Frame>,
    cursors: [Frame; 5],
    /// Per unit type ID, one frame per player slot.
    units: Vec<Vec<Frame>>,
    buildings: Vec<Vec<Frame>>,
}

impl GameImages {
    /// Uploads every image as a texture.
    pub fn upload<R: Renderer>(&self, renderer: &mut R) -> Sprites {
        fn upload_one<R: Renderer>(renderer: &mut R, frame: &Frame) -> Sprite {
            let Frame { image, hotspot } = frame;
            Sprite {
                texture: renderer.load_texture(image.width, image.height, &image.rgba),
                width: image.width as f32,
                height: image.height as f32,
                hotspot: *hotspot,
            }
        }
        let tree = upload_one(renderer, &self.tree);
        let gold_mine = upload_one(renderer, &self.gold_mine);
        let stone_mine = upload_one(renderer, &self.stone_mine);
        let bush = upload_one(renderer, &self.bush);
        let interface = upload_one(renderer, &self.interface);
        let mut upload_all = |frames: &[Frame]| -> Vec<Sprite> {
            frames.iter().map(|f| upload_one(renderer, f)).collect()
        };
        let unit_icons = upload_all(&self.unit_icons);
        let building_icons = upload_all(&self.building_icons);
        let progress = upload_all(&self.progress);
        let cursors = std::array::from_fn(|i| upload_one(renderer, &self.cursors[i]));
        let mut upload_set = |frames: &[Frame]| -> [Sprite; PLAYER_SLOTS] {
            std::array::from_fn(|i| upload_one(renderer, &frames[i]))
        };
        let units = self.units.iter().map(|frames| upload_set(frames)).collect();
        let buildings = self
            .buildings
            .iter()
            .map(|frames| upload_set(frames))
            .collect();
        let mut ground = |images: &[Image]| -> Vec<TextureId> {
            images
                .iter()
                .map(|image| renderer.load_texture(image.width, image.height, &image.rgba))
                .collect()
        };
        let grass = ground(&self.grass);
        let farm = ground(&self.farm);
        Sprites {
            grass,
            farm,
            tree,
            gold_mine,
            stone_mine,
            bush,
            interface,
            unit_icons,
            building_icons,
            progress,
            cursors,
            units,
            buildings,
        }
    }
}

/// Why the game's sprites could not be loaded.
#[derive(Debug)]
pub enum AssetError {
    Io {
        path: PathBuf,
        error: std::io::Error,
    },
    Drs {
        path: PathBuf,
        error: DrsError,
    },
    Palette(PaletteError),
    /// An archive does not hold a file the game needs.
    Missing {
        path: PathBuf,
        id: u32,
    },
    /// A sprite with no frames, or a frame with no pixels: nothing to draw,
    /// and grass with no frames would divide by zero when choosing a tile.
    EmptySprite {
        id: u32,
    },
    Slp {
        id: u32,
        error: SlpError,
    },
    /// A content entry names an icon frame the install's icon sprite does not
    /// have.
    MissingIcon {
        entry: String,
        frame: u32,
    },
    /// A content entry's icon frame has no pixels to draw.
    EmptyIcon {
        entry: String,
        frame: u32,
    },
    /// A type has no look: a replay's type the content the game was started
    /// with does not have, by name.
    NoLook {
        entry: String,
    },
    /// A content entry's parts are placed so far apart that together they
    /// would make a sprite larger than any may be: a damaged hotspot.
    PartsTooFarApart {
        entry: String,
    },
    /// A content entry names a sprite the install does not have.
    MissingGraphic {
        entry: String,
        id: u32,
    },
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetError::Io { path, error } => write!(f, "{}: {error}", path.display()),
            AssetError::Drs { path, error } => write!(f, "{}: {error}", path.display()),
            AssetError::Palette(error) => write!(f, "game palette: {error}"),
            AssetError::EmptySprite { id } => {
                write!(f, "sprite {id} has no frames or no pixels to draw")
            }
            AssetError::Missing { path, id } => {
                write!(
                    f,
                    "{}: no file {id}; is this an Age of Empires II install?",
                    path.display()
                )
            }
            AssetError::Slp { id, error } => write!(f, "sprite {id}: {error}"),
            AssetError::MissingIcon { entry, frame } => write!(
                f,
                "content entry `{entry}` names icon {frame}, which the icon sprite does not have"
            ),
            AssetError::EmptyIcon { entry, frame } => write!(
                f,
                "content entry `{entry}` names icon {frame}, which has no pixels to draw"
            ),
            AssetError::NoLook { entry } => write!(
                f,
                "type `{entry}` has no sprite or icon in the content the game was started with"
            ),
            AssetError::PartsTooFarApart { entry } => write!(
                f,
                "content entry `{entry}`: its parts lie too far apart to draw together"
            ),
            AssetError::MissingGraphic { entry, id } => write!(
                f,
                "content entry `{entry}` names sprite {id}, which data/graphics.drs does not have"
            ),
        }
    }
}

impl std::error::Error for AssetError {}

/// The archives the game cannot draw without, read first so that a missing
/// install is reported by the first one a player would miss.
const REQUIRED: [&str; 3] = ["interfac.drs", "graphics.drs", "terrain.drs"];

/// One archive in memory, parsed once into an index of its files.
struct Archive {
    bytes: Vec<u8>,
    /// (file type, ID) to the file's bytes.
    index: HashMap<(String, u32), Range<usize>>,
}

/// Every archive of an install, in the original's search order.
struct Archives {
    dir: PathBuf,
    /// In [`ARCHIVE_ORDER`]; archives the install lacks are left out.
    list: Vec<Archive>,
}

impl Archives {
    fn read(dir: &Path) -> Result<Archives, AssetError> {
        let read = |name: &str| -> Result<Option<Archive>, AssetError> {
            let path = dir.join("data").join(name);
            let bytes = match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error)
                    if error.kind() == std::io::ErrorKind::NotFound
                        && !REQUIRED.contains(&name) =>
                {
                    return Ok(None);
                }
                Err(error) => return Err(AssetError::Io { path, error }),
            };
            let index = Drs::parse(&bytes)
                .map_err(|error| AssetError::Drs {
                    path: path.clone(),
                    error,
                })?
                .entries()
                .map(|(kind, id, range)| ((kind.to_string(), id), range))
                .collect();
            Ok(Some(Archive { bytes, index }))
        };
        let mut loaded: HashMap<&str, Archive> = HashMap::new();
        for name in REQUIRED
            .iter()
            .chain(ARCHIVE_ORDER.iter().filter(|n| !REQUIRED.contains(n)))
        {
            if let Some(archive) = read(name)? {
                loaded.insert(name, archive);
            }
        }
        let list = ARCHIVE_ORDER
            .iter()
            .filter_map(|name| loaded.remove(name))
            .collect();
        Ok(Archives {
            dir: dir.to_path_buf(),
            list,
        })
    }

    /// The first archive's file of type `kind` (`slp`, `bina`) and `id`.
    fn get(&self, kind: &str, id: u32) -> Result<&[u8], AssetError> {
        let key = (kind.to_string(), id);
        self.list
            .iter()
            .find_map(|a| a.index.get(&key).map(|range| &a.bytes[range.clone()]))
            .ok_or_else(|| AssetError::Missing {
                path: self.dir.join("data"),
                id,
            })
    }

    fn sprite(&self, id: u32) -> Result<Slp<'_>, AssetError> {
        Slp::parse(self.get("slp", id)?).map_err(|error| AssetError::Slp { id, error })
    }
}

/// Reads and decodes every sprite the game draws from an install directory:
/// the folder holding `data/interfac.drs`, `data/graphics.drs`, and
/// `data/terrain.drs`, plus the original's other archives where present,
/// searched in the original's order. Each unit and building type is drawn with the sprite
/// its `content` entry names.
pub fn load_images(dir: &Path, content: &Content, looks: &Looks) -> Result<GameImages, AssetError> {
    let archives = Archives::read(dir)?;
    let palette = Palette::parse(archives.get("bina", PALETTE)?).map_err(AssetError::Palette)?;

    // Frame 0 of each sprite: a unit's first facing; animation is a later
    // step. `player` picks the player colours.
    let decode = |id: u32, player: u8| -> Result<Frame, AssetError> {
        let slp = archives.sprite(id)?;
        let first = *slp.frames().next().ok_or(AssetError::EmptySprite { id })?;
        if first.width == 0 || first.height == 0 {
            return Err(AssetError::EmptySprite { id });
        }
        let image = slp
            .decode(0, &palette, player)
            .map_err(|error| AssetError::Slp { id, error })?;
        Ok(Frame {
            image,
            hotspot: Vec2::new(first.hotspot_x as f32, first.hotspot_y as f32),
        })
    };
    // One frame per player colour, for the type called `entry`: its
    // `sprite`, or its `parts` composed into one when it has any.
    let per_player = |entry: &str, sprite: u32, parts: &[Part]| -> Result<Vec<Frame>, AssetError> {
        let alone = [Part::new(sprite, 0, 0)];
        let ids = if parts.is_empty() { &alone[..] } else { parts };
        (0..PLAYER_SLOTS as u8)
            .map(|player| {
                let frames = ids
                    .iter()
                    .map(|part| {
                        let id = part.slp;
                        let mut frame = decode(id, player).map_err(|error| match error {
                            AssetError::Missing { .. } => AssetError::MissingGraphic {
                                entry: entry.to_string(),
                                id,
                            },
                            other => other,
                        })?;
                        // Drawn `offset` pixels from the ground point: as if
                        // its hotspot were that much the other way.
                        let [x, y] = part.offset;
                        frame.hotspot = frame.hotspot - Vec2::new(x as f32, y as f32);
                        Ok(frame)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                compose(&frames).ok_or_else(|| AssetError::PartsTooFarApart {
                    entry: entry.to_string(),
                })
            })
            .collect()
    };
    // Frame `index` of an interface sprite, drawn in player one's colours
    // until each player's panel shows their own. `None` when the sprite has
    // no such frame.
    let interface_frame =
        |slp: &Slp<'_>, id: u32, index: usize| -> Result<Option<Frame>, AssetError> {
            let Some(header) = slp.frames().nth(index) else {
                return Ok(None);
            };
            if header.width == 0 || header.height == 0 {
                return Err(AssetError::EmptySprite { id });
            }
            let image = slp
                .decode(index, &palette, 1)
                .map_err(|error| AssetError::Slp { id, error })?;
            Ok(Some(Frame {
                image,
                hotspot: Vec2::ZERO,
            }))
        };
    // A content entry's icon: its frame of the icon sprite `id`.
    let icon = |slp: &Slp<'_>, id: u32, entry: &str, frame: u32| -> Result<Frame, AssetError> {
        let missing = || AssetError::MissingIcon {
            entry: entry.to_string(),
            frame,
        };
        let index = usize::try_from(frame).map_err(|_| missing())?;
        match interface_frame(slp, id, index) {
            Err(AssetError::EmptySprite { .. }) => Err(AssetError::EmptyIcon {
                entry: entry.to_string(),
                frame,
            }),
            other => other?.ok_or_else(missing),
        }
    };
    let unit_icons = archives.sprite(UNIT_ICONS)?;
    let building_icons = archives.sprite(BUILDING_ICONS)?;
    let progress_bar = archives.sprite(PROGRESS_BAR)?;
    let progress = (0..progress_bar.frames().count())
        .filter_map(|i| interface_frame(&progress_bar, PROGRESS_BAR, i).transpose())
        .collect::<Result<Vec<_>, _>>()?;
    if progress.is_empty() {
        return Err(AssetError::EmptySprite { id: PROGRESS_BAR });
    }
    // The cursors keep their hotspots: the point of each is what is on the
    // mouse.
    let cursor_sprite = archives.sprite(CURSORS)?;
    let mut cursors = Vec::with_capacity(CURSOR_FRAMES.len());
    for index in CURSOR_FRAMES {
        let header = cursor_sprite
            .frames()
            .nth(index)
            .ok_or(AssetError::EmptySprite { id: CURSORS })?;
        let mut frame = interface_frame(&cursor_sprite, CURSORS, index)?
            .ok_or(AssetError::EmptySprite { id: CURSORS })?;
        // interface_frame sets hotspot to ZERO (for UI elements), but cursors
        // need the SLP's actual hotspot so the game cursor is drawn with its
        // hotspot on the mouse. Overwrite with the SLP header's hotspot.
        frame.hotspot = Vec2::new(header.hotspot_x as f32, header.hotspot_y as f32);
        cursors.push(frame);
    }
    let cursors: [Frame; 5] = cursors.try_into().expect("one frame per cursor");
    // Neutral objects belong to Gaia: player slot 0's colours, each drawn
    // from its pieces.
    let neutral = |name: &str, ids: &[u32]| -> Result<Frame, AssetError> {
        let frames = ids
            .iter()
            .map(|&id| decode(id, 0))
            .collect::<Result<Vec<_>, _>>()?;
        compose(&frames).ok_or_else(|| AssetError::PartsTooFarApart {
            entry: name.to_string(),
        })
    };

    // A ground texture: its first variations, each unskewed to a square tile.
    let ground = |id: u32| -> Result<Vec<Image>, AssetError> {
        let slp = archives.sprite(id)?;
        let variations = slp.frames().count().min(GROUND_VARIATIONS);
        if variations == 0 || slp.frames().any(|f| f.width == 0 || f.height == 0) {
            return Err(AssetError::EmptySprite { id });
        }
        (0..variations)
            .map(|frame| {
                slp.decode(frame, &palette, 0)
                    .map(|diamond| unskew_tile(&diamond, TILE_PIXELS))
                    .map_err(|error| AssetError::Slp { id, error })
            })
            .collect()
    };
    let grass = ground(GRASS)?;
    let farm = ground(FARM)?;

    // Each type's look, found by its name, in the content's order.
    let no_look = |entry: &str| AssetError::NoLook {
        entry: entry.to_string(),
    };
    let unit_looks = content
        .units
        .iter()
        .map(|u| {
            looks
                .unit(&u.name)
                .map(|l| (u.name.as_str(), l))
                .ok_or_else(|| no_look(&u.name))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let building_looks = content
        .buildings
        .iter()
        .map(|b| {
            looks
                .building(&b.name)
                .map(|l| (b.name.as_str(), l))
                .ok_or_else(|| no_look(&b.name))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(GameImages {
        grass,
        farm,
        tree: neutral("tree", &TREE)?,
        gold_mine: neutral("gold mine", &GOLD_MINE)?,
        stone_mine: neutral("stone mine", &STONE_MINE)?,
        bush: neutral("forage bush", &BUSH)?,
        interface: neutral("interface", &[INTERFACE])?,
        unit_icons: unit_looks
            .iter()
            .map(|(name, look)| icon(&unit_icons, UNIT_ICONS, name, look.icon))
            .collect::<Result<_, _>>()?,
        building_icons: building_looks
            .iter()
            .map(|(name, look)| icon(&building_icons, BUILDING_ICONS, name, look.icon))
            .collect::<Result<_, _>>()?,
        progress,
        cursors,
        units: unit_looks
            .iter()
            .map(|(name, look)| per_player(name, look.sprite, &look.parts))
            .collect::<Result<_, _>>()?,
        buildings: building_looks
            .iter()
            .map(|(name, look)| per_player(name, look.sprite, &look.parts))
            .collect::<Result<_, _>>()?,
    })
}

/// Draws `frames` over one another, in order, each with its hotspot on one
/// shared point, into one frame just large enough to hold them all. `None`
/// when that frame would be larger than any sprite may be, as only damaged
/// hotspots make it.
///
/// This is how the original draws a graphic's pieces and a building's
/// annexes: every piece at the unit's position, placed by its own hotspot.
/// A later piece is drawn over the earlier ones, so a half-transparent
/// shadow darkens what it falls on.
fn compose(frames: &[Frame]) -> Option<Frame> {
    if let [only] = frames {
        return Some(only.clone());
    }
    // Each frame's corners relative to the shared point, in whole pixels.
    let corners = |f: &Frame| {
        let (x, y) = (-(f.hotspot.x as i64), -(f.hotspot.y as i64));
        (
            x,
            y,
            x + i64::from(f.image.width),
            y + i64::from(f.image.height),
        )
    };
    let (mut left, mut top, mut right, mut bottom) = (0, 0, 0, 0);
    for (i, f) in frames.iter().enumerate() {
        let (l, t, r, b) = corners(f);
        if i == 0 {
            (left, top, right, bottom) = (l, t, r, b);
        } else {
            (left, top, right, bottom) = (left.min(l), top.min(t), right.max(r), bottom.max(b));
        }
    }
    let side = i64::from(MAX_FRAME_SIDE);
    if right - left > side || bottom - top > side {
        return None;
    }
    let (width, height) = ((right - left) as u32, (bottom - top) as u32);
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for f in frames {
        let (l, t, _, _) = corners(f);
        for y in 0..f.image.height {
            for x in 0..f.image.width {
                let pixel = f.image.pixel(x, y);
                if pixel[3] == 0 {
                    continue;
                }
                let tx = (l - left) as u32 + x;
                let ty = (t - top) as u32 + y;
                let at = ((ty * width + tx) * 4) as usize;
                let under = [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]];
                rgba[at..at + 4].copy_from_slice(&over(pixel, under));
            }
        }
    }
    Some(Frame {
        image: Image {
            width,
            height,
            rgba,
        },
        hotspot: Vec2::new(-left as f32, -top as f32),
    })
}

/// `top` drawn over `under`, both straight (not premultiplied) RGBA.
fn over(top: [u8; 4], under: [u8; 4]) -> [u8; 4] {
    let (ta, ua) = (u32::from(top[3]), u32::from(under[3]));
    // Alphas out of 255; the result's alpha, out of 255 * 255.
    let out = ta * 255 + ua * (255 - ta);
    if out == 0 {
        return [0; 4];
    }
    let channel = |i: usize| {
        let mixed = u32::from(top[i]) * ta * 255 + u32::from(under[i]) * ua * (255 - ta);
        (mixed / out) as u8
    };
    [channel(0), channel(1), channel(2), (out / 255) as u8]
}

/// Turns one of the original's diamond terrain tiles into a `size` x `size`
/// square, for this game's top-down grid.
///
/// A point (u, v) of the square, each 0 to 1 with v downwards, sits in the
/// diamond at x = (u - v) * w / 2 + w / 2 and y = (u + v) * h / 2: the
/// square's top-left corner comes from the diamond's top vertex, its
/// top-right from the right vertex, and so on round.
pub fn unskew_tile(diamond: &Image, size: u32) -> Image {
    // Nothing to sample from: a transparent square rather than a clamp with
    // its bounds reversed.
    if diamond.width == 0 || diamond.height == 0 {
        return Image {
            width: size,
            height: size,
            rgba: vec![0; (size * size * 4) as usize],
        };
    }
    let (w, h) = (diamond.width as f32, diamond.height as f32);
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let u = (x as f32 + 0.5) / size as f32;
            let v = (y as f32 + 0.5) / size as f32;
            let sx = ((u - v) * w / 2.0 + w / 2.0).clamp(0.0, w - 1.0) as u32;
            let sy = ((u + v) * h / 2.0).clamp(0.0, h - 1.0) as u32;
            rgba.extend(diamond.pixel(sx, sy));
        }
    }
    Image {
        width: size,
        height: size,
        rgba,
    }
}

/// The install to take sprites from: the `--assets` option if given, else
/// the `AOE_ASSETS` environment variable. The game draws nothing without the
/// original sprites, so having neither is an error that says how to fix it.
pub fn assets_dir(
    option: Option<&Path>,
    env: Option<std::ffi::OsString>,
) -> Result<PathBuf, String> {
    match (option, env) {
        (Some(dir), _) => Ok(dir.to_path_buf()),
        (None, Some(dir)) => Ok(PathBuf::from(dir)),
        (None, None) => Err(
            "no Age of Empires II install to draw with: pass --assets DIR \
             (the folder holding data/graphics.drs) or set AOE_ASSETS"
                .to_string(),
        ),
    }
}
