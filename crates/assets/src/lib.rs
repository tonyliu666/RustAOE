//! Readers for the original game's asset formats.
//!
//! The files themselves are never committed: the binary reads them from a
//! user-supplied install directory (see the pre-stage spec,
//! `docs/superpowers/specs/2026-10-24-prestage-edge-scroll-and-real-sprites.md`).
//! Every reader takes a byte slice and returns errors as values, because the
//! bytes come from a file on disk that may be damaged or not what it claims.

pub mod dat;
mod drs;
mod palette;
mod slp;

pub use drs::{Drs, DrsError, DrsTable};
pub use palette::{Palette, PaletteError};
pub use slp::{Image, Slp, SlpError, SlpFrame, MAX_FRAME_SIDE, SHADOW};
