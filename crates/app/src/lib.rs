//! Application wiring: the only place `sim` and `render` meet.
//!
//! `render` cannot see `sim`, so translating world state into draw calls
//! happens here. Everything in this crate is presentation or input mapping and
//! may use floating point freely; nothing here is part of the simulation.
//!
//! See `docs/superpowers/specs/2026-08-04-rts-poc-design.md`.

mod camera_controller;
mod draw;

pub use camera_controller::CameraController;
pub use draw::{draw_world, TILE_BLOCKED, TILE_OPEN};
