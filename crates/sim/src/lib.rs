//! Deterministic simulation core.
//!
//! This crate must stay free of floating point, I/O, wall-clock time, and
//! threads. See `docs/superpowers/specs/2026-08-04-rts-poc-design.md`.

#![deny(clippy::float_arithmetic)]

mod fx;
mod grid;
mod rng;

pub use fx::Fx;
pub use grid::{Grid, TilePos};
pub use rng::Rng;
