//! How each unit and building type looks: its sprite, the parts it is drawn
//! in, and its button's icon.
//!
//! Kept out of `sim::Content` on purpose. A replay carries the content it
//! was recorded with, because the content decides how the game plays; how a
//! type looks does not, so a replay leaves it to the game playing it back,
//! which matches looks to the replay's types by name. Changing a picture
//! therefore never breaks a recording.

use std::collections::BTreeMap;

/// One sprite of a type drawn in parts: an SLP, and how far from the unit's
/// ground point it is drawn, in sprite pixels, right and down.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Part {
    pub slp: u32,
    pub offset: [i32; 2],
}

impl Part {
    pub const fn new(slp: u32, x: i32, y: i32) -> Part {
        Part {
            slp,
            offset: [x, y],
        }
    }
}

/// How one type looks.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Look {
    /// The SLP it is drawn with, found in the original's archives in the
    /// original's search order.
    pub sprite: u32,
    /// When not empty, the SLPs drawn instead of `sprite` alone, in order,
    /// as the original draws a graphic's pieces and a building's annexes:
    /// each at the unit's ground point plus its offset, placed by its own
    /// hotspot.
    pub parts: Vec<Part>,
    /// Its button: a frame of the original's unit icons (`interfac.drs`
    /// 50730) for a unit, of the building icons (50705) for a building.
    pub icon: u32,
}

/// How every type looks, by type name.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Looks {
    pub units: BTreeMap<String, Look>,
    pub buildings: BTreeMap<String, Look>,
}

impl Looks {
    pub fn unit(&self, name: &str) -> Option<&Look> {
        self.units.get(name)
    }

    pub fn building(&self, name: &str) -> Option<&Look> {
        self.buildings.get(name)
    }
}
