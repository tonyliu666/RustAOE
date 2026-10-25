//! Application wiring: the only place `sim` and `render` meet.
//!
//! `render` cannot see `sim`, so translating world state into draw calls
//! happens here. Everything in this crate is presentation or input mapping and
//! may use floating point freely; nothing here is part of the simulation.
//!
//! See `docs/superpowers/specs/2026-08-04-rts-poc-design.md`.

mod camera_controller;
mod content_file;
mod draw;
mod looks;
mod pick;
mod replay_file;
mod session;
mod sprites;
mod ui;

pub use camera_controller::CameraController;
pub use content_file::{
    default_looks, game_looks, load_content, new_game_content, parse_content, parse_looks,
    ContentFileError, DEFAULT_CONTENT,
};
pub use draw::{draw_world, Hud, HEALTH, HEALTH_LOST, SELECTION};
pub use looks::{Look, Looks, Part};
pub use pick::{
    building_at, entity_at, site_at, tile_at, to_screen_space, unit_at, units_in_box, Bodies, Body,
};
pub use replay_file::{load, save, ReplayError};
pub use session::{PlayerUi, Session};
pub use sprites::{
    assets_dir, load_images, unskew_tile, AssetError, GameImages, Sprite, Sprites,
    UNIT_PIXELS_PER_TILE,
};
pub use ui::{Layout, Rect};
