//! Deterministic simulation core.
//!
//! This crate must stay free of floating point, I/O, wall-clock time, and
//! threads. See `docs/superpowers/specs/2026-08-04-rts-poc-design.md`.

#![deny(clippy::float_arithmetic)]

mod content;
mod entity;
mod fx;
mod grid;
mod path;
mod replay;
mod resource;
mod rng;
mod spatial;
mod vec2;
mod world;

pub use content::{BuildingStats, Content, ContentError, GatherStats, Part, UnitStats};
pub use entity::{
    Building, BuildingId, BuildingKind, EntityId, GatherPhase, Mine, MineId, Order, Player,
    PlayerId, Route, Unit, UnitId, UnitKind,
};
pub use fx::Fx;
pub use grid::{Grid, TilePos};
pub use path::{astar, Dir8, FlowField, FlowFieldCache, PathRequest};
pub use replay::{verify, Desync, Replay};
pub use resource::{Resource, Resources};
pub use rng::Rng;
pub use spatial::SpatialHash;
pub use vec2::FxVec2;
pub use world::{Command, MapError, MapSpec, Outcome, Tile, World, WorldView};
