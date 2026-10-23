//! Application wiring: the only place `sim` and `render` meet.
//!
//! `render` cannot see `sim`, so translating world state into draw calls
//! happens here. Everything in this crate is presentation or input mapping and
//! may use floating point freely; nothing here is part of the simulation.
//!
//! See `docs/superpowers/specs/2026-08-04-rts-poc-design.md`.

mod camera_controller;
mod draw;
mod pick;
mod replay_file;
mod session;

pub use camera_controller::CameraController;
pub use draw::{
    draw_world, Hud, HEALTH, HEALTH_LOST, MINE, PLAYER_ONE, PLAYER_TWO, SELECTION, TILE_BLOCKED,
    TILE_OPEN,
};
pub use pick::{building_at, entity_at, mine_at, tile_at, to_screen_space, unit_at, units_in_box};
pub use replay_file::{load, save, ReplayError};
pub use session::{PlayerUi, Session};
