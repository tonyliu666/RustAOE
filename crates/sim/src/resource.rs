//! The four resources and amounts of them.
//!
//! A closed enum, unlike unit and building kinds: food, wood, gold, and stone
//! are a fixed rule of the game, not content that grows.

use serde::{Deserialize, Serialize};

/// One of the four resources.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Resource {
    Food,
    Wood,
    Gold,
    Stone,
}

impl Resource {
    /// Every resource, in the order the checksum and the user interface use.
    pub const ALL: [Resource; 4] = [
        Resource::Food,
        Resource::Wood,
        Resource::Gold,
        Resource::Stone,
    ];

    /// The lower-case name, as written in content files and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            Resource::Food => "food",
            Resource::Wood => "wood",
            Resource::Gold => "gold",
            Resource::Stone => "stone",
        }
    }
}

/// An amount of each resource: a stockpile, a cost, or a load.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Resources {
    pub food: i32,
    pub wood: i32,
    pub gold: i32,
    pub stone: i32,
}

impl Resources {
    pub const fn get(&self, resource: Resource) -> i32 {
        match resource {
            Resource::Food => self.food,
            Resource::Wood => self.wood,
            Resource::Gold => self.gold,
            Resource::Stone => self.stone,
        }
    }

    pub fn get_mut(&mut self, resource: Resource) -> &mut i32 {
        match resource {
            Resource::Food => &mut self.food,
            Resource::Wood => &mut self.wood,
            Resource::Gold => &mut self.gold,
            Resource::Stone => &mut self.stone,
        }
    }

    /// Whether this stockpile holds at least `cost` of every resource.
    pub fn covers(&self, cost: &Resources) -> bool {
        Resource::ALL
            .into_iter()
            .all(|r| self.get(r) >= cost.get(r))
    }

    /// Saturating, so that extreme content cannot panic a debug build or wrap
    /// a release build: the two must play the same game.
    pub fn saturating_sub(self, other: Resources) -> Resources {
        self.zip(other, i32::saturating_sub)
    }

    /// Saturating for the same reason as [`Resources::saturating_sub`].
    pub fn saturating_add(self, other: Resources) -> Resources {
        self.zip(other, i32::saturating_add)
    }

    fn zip(self, other: Resources, f: fn(i32, i32) -> i32) -> Resources {
        Resources {
            food: f(self.food, other.food),
            wood: f(self.wood, other.wood),
            gold: f(self.gold, other.gold),
            stone: f(self.stone, other.stone),
        }
    }
}
