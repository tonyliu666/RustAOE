//! The original game's sprites: loading them from an install and where to
//! draw them.
//!
//! Read from the user's own extracted installer at runtime (`--assets DIR`),
//! never committed. Decoding happens once at startup; drawing uses uploaded
//! textures. See `docs/superpowers/specs/2026-10-24-prestage-edge-scroll-and-real-sprites.md`
//! for how the sprite IDs below were chosen.

use crate::pick::{Bodies, Body};
use assets::{Drs, DrsError, Image, Palette, PaletteError, Slp, SlpError};
use render::{Renderer, TextureId, Vec2};
use sim::{BuildingKind, Content, PlayerId, UnitKind};
use std::fmt;
use std::path::{Path, PathBuf};

/// Sprite pixels per tile for units. The original's tile is 97 pixels across
/// its diagonal, about 68 along a side; units are drawn a little larger than
/// that scale so that they read well on a top-down grid.
pub const UNIT_PIXELS_PER_TILE: f32 = 48.0;

/// The game palette, in `interfac.drs`.
const PALETTE: u32 = 50500;
/// Grass, in `terrain.drs`. Each frame is a variation of the same terrain.
const GRASS: u32 = 15001;
/// How many grass variations to load.
const GRASS_VARIATIONS: usize = 16;
/// Side of an unskewed terrain texture, in pixels.
const TILE_PIXELS: u32 = 32;
// Unit and building sprites come from each type's `sprite` in the content.
/// An oak, drawn on every rock tile so that blocked ground reads as forest.
const TREE: u32 = 1270;
/// Frame 0 of a full gold mine; later frames are the mine running out.
const GOLD_MINE: u32 = 4479;
/// Gaia plus eight players: one decoded copy of each sprite per player
/// colour.
const PLAYER_SLOTS: usize = 9;

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
    pub tree: Sprite,
    pub gold_mine: Sprite,
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

    pub fn building(&self, kind: BuildingKind, owner: PlayerId) -> &Sprite {
        &self.buildings[kind.index()][slot(owner)]
    }

    /// Each unit kind's drawn body, for picking: the same placement as
    /// drawing, so a click lands on what the player sees.
    pub fn bodies(&self) -> Bodies {
        let body = |sprite: &Sprite| {
            let scale = 1.0 / UNIT_PIXELS_PER_TILE;
            Body {
                offset: Vec2::new(-sprite.hotspot.x, -sprite.hotspot.y) * scale,
                size: Vec2::new(sprite.width, sprite.height) * scale,
            }
        };
        Bodies {
            units: self.units.iter().map(|set| body(&set[1])).collect(),
            ..Bodies::default()
        }
    }

    /// The grass texture for a tile. Varies from tile to tile, the same way
    /// every frame, so the ground does not flicker.
    pub fn grass_at(&self, x: i32, y: i32) -> TextureId {
        let hash = (x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663);
        self.grass[hash as usize % self.grass.len()]
    }
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
    tree: Frame,
    gold_mine: Frame,
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
        let mut upload_set = |frames: &[Frame]| -> [Sprite; PLAYER_SLOTS] {
            std::array::from_fn(|i| upload_one(renderer, &frames[i]))
        };
        let units = self.units.iter().map(|frames| upload_set(frames)).collect();
        let buildings = self
            .buildings
            .iter()
            .map(|frames| upload_set(frames))
            .collect();
        let grass = self
            .grass
            .iter()
            .map(|image| renderer.load_texture(image.width, image.height, &image.rgba))
            .collect();
        Sprites {
            grass,
            tree,
            gold_mine,
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
            AssetError::MissingGraphic { entry, id } => write!(
                f,
                "content entry `{entry}` names sprite {id}, which data/graphics.drs does not have"
            ),
        }
    }
}

impl std::error::Error for AssetError {}

/// An archive read into memory, remembering where it came from for errors.
struct Archive {
    path: PathBuf,
    bytes: Vec<u8>,
}

impl Archive {
    fn read(dir: &Path, name: &str) -> Result<Archive, AssetError> {
        let path = dir.join("data").join(name);
        let bytes = std::fs::read(&path).map_err(|error| AssetError::Io {
            path: path.clone(),
            error,
        })?;
        Drs::parse(&bytes).map_err(|error| AssetError::Drs {
            path: path.clone(),
            error,
        })?;
        Ok(Archive { path, bytes })
    }

    fn get(&self, id: u32) -> Result<&[u8], AssetError> {
        let drs = Drs::parse(&self.bytes).map_err(|error| AssetError::Drs {
            path: self.path.clone(),
            error,
        })?;
        drs.get(id).ok_or_else(|| AssetError::Missing {
            path: self.path.clone(),
            id,
        })
    }

    fn sprite(&self, id: u32) -> Result<Slp<'_>, AssetError> {
        Slp::parse(self.get(id)?).map_err(|error| AssetError::Slp { id, error })
    }
}

/// Reads and decodes every sprite the game draws from an install directory:
/// the folder holding `data/interfac.drs`, `data/graphics.drs`, and
/// `data/terrain.drs`. Each unit and building type is drawn with the sprite
/// its `content` entry names.
pub fn load_images(dir: &Path, content: &Content) -> Result<GameImages, AssetError> {
    let interfac = Archive::read(dir, "interfac.drs")?;
    let palette = Palette::parse(interfac.get(PALETTE)?).map_err(AssetError::Palette)?;
    let graphics = Archive::read(dir, "graphics.drs")?;
    let terrain = Archive::read(dir, "terrain.drs")?;

    // Frame 0 of each sprite: a unit's first facing; animation is a later
    // step. `player` picks the player colours.
    let decode = |id: u32, player: u8| -> Result<Frame, AssetError> {
        let slp = graphics.sprite(id)?;
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
    // One frame per player colour, for the type called `entry`.
    let per_player = |entry: &str, id: u32| -> Result<Vec<Frame>, AssetError> {
        (0..PLAYER_SLOTS as u8)
            .map(|player| {
                decode(id, player).map_err(|error| match error {
                    AssetError::Missing { .. } => AssetError::MissingGraphic {
                        entry: entry.to_string(),
                        id,
                    },
                    other => other,
                })
            })
            .collect()
    };
    // Neutral objects belong to Gaia: player slot 0's colours.
    let neutral = |id: u32| decode(id, 0);

    let grass_slp = terrain.sprite(GRASS)?;
    let variations = grass_slp.frames().count().min(GRASS_VARIATIONS);
    if variations == 0 || grass_slp.frames().any(|f| f.width == 0 || f.height == 0) {
        return Err(AssetError::EmptySprite { id: GRASS });
    }
    let grass = (0..variations)
        .map(|frame| {
            grass_slp
                .decode(frame, &palette, 0)
                .map(|diamond| unskew_tile(&diamond, TILE_PIXELS))
                .map_err(|error| AssetError::Slp { id: GRASS, error })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(GameImages {
        grass,
        tree: neutral(TREE)?,
        gold_mine: neutral(GOLD_MINE)?,
        units: content
            .units
            .iter()
            .map(|u| per_player(&u.name, u.sprite))
            .collect::<Result<_, _>>()?,
        buildings: content
            .buildings
            .iter()
            .map(|b| per_player(&b.name, b.sprite))
            .collect::<Result<_, _>>()?,
    })
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
