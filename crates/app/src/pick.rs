//! Turning a point or a rectangle on screen into the entities under it.
//!
//! Picking is presentation, not simulation: the answer depends on where the
//! camera is, which `sim` has no business knowing.

use render::Vec2;
use sim::{BuildingId, EntityId, FxVec2, MineId, PlayerId, TilePos, UnitId, UnitKind, WorldView};

/// Converts a fixed-point world position into the renderer's floats.
///
/// One-way on purpose. Values cross from `sim` to `render` for drawing and
/// never come back, so no float ever reaches the simulation.
pub fn to_screen_space(pos: FxVec2) -> Vec2 {
    const SCALE: f32 = 1.0 / 65536.0;
    Vec2::new(
        pos.x.to_bits() as f32 * SCALE,
        pos.y.to_bits() as f32 * SCALE,
    )
}

/// Where a unit's drawn body is, relative to the point it stands on, in
/// tiles: what a click or a drag box has to touch to pick it.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Body {
    /// From the ground point to the body's top-left corner.
    pub offset: Vec2,
    pub size: Vec2,
}

impl Body {
    /// A square of `half` tiles either side of the ground point: forgiving
    /// enough that a click at low zoom does not test the player's aim.
    pub const fn around_feet(half: f32) -> Body {
        Body {
            offset: Vec2::new(-half, -half),
            size: Vec2::new(half * 2.0, half * 2.0),
        }
    }

    fn at(&self, ground: Vec2) -> (Vec2, Vec2) {
        let min = ground + self.offset;
        (min, min + self.size)
    }
}

/// Each unit kind's body. The application builds these from the sprites, so
/// that picking matches drawing; the default is a square around the feet.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Bodies {
    pub villager: Body,
    pub soldier: Body,
}

impl Default for Bodies {
    fn default() -> Bodies {
        Bodies {
            villager: Body::around_feet(0.6),
            soldier: Body::around_feet(0.6),
        }
    }
}

impl Bodies {
    pub const fn of(&self, kind: UnitKind) -> Body {
        match kind {
            UnitKind::Villager => self.villager,
            UnitKind::Soldier => self.soldier,
        }
    }
}

/// The tile a world position falls in.
pub fn tile_at(world: Vec2) -> TilePos {
    TilePos::new(world.x.floor() as i32, world.y.floor() as i32)
}

/// The unit whose drawn body is under `world`.
///
/// Where bodies overlap, the one drawn in front wins: units are drawn
/// standing on their positions, so the one lower on screen covers the other.
pub fn unit_at(view: WorldView<'_>, world: Vec2, bodies: &Bodies) -> Option<UnitId> {
    view.units()
        .filter_map(|(id, unit)| {
            let ground = to_screen_space(unit.pos);
            let (min, max) = bodies.of(unit.kind).at(ground);
            let inside = (min.x..=max.x).contains(&world.x) && (min.y..=max.y).contains(&world.y);
            inside.then_some((id, ground.y))
        })
        // `total_cmp` because `f32` has no total order, and the same click
        // must resolve the same way every frame.
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(id, _)| id)
}

/// The building covering `world`, if any.
pub fn building_at(view: WorldView<'_>, world: Vec2) -> Option<BuildingId> {
    let tile = tile_at(world);
    view.buildings()
        .find(|(_, b)| b.footprint().any(|t| t == tile))
        .map(|(id, _)| id)
}

/// The mine covering `world`, if any.
pub fn mine_at(view: WorldView<'_>, world: Vec2) -> Option<MineId> {
    let tile = tile_at(world);
    view.mines()
        .find(|(_, m)| m.footprint().any(|t| t == tile))
        .map(|(id, _)| id)
}

/// Whatever is under `world`, preferring the thing a player most likely meant.
///
/// Units first, because they are small, on top, and the usual target; a mine
/// last, because it is large and mostly scenery until a villager is selected.
pub fn entity_at(view: WorldView<'_>, world: Vec2, bodies: &Bodies) -> Option<EntityId> {
    unit_at(view, world, bodies)
        .map(EntityId::Unit)
        .or_else(|| building_at(view, world).map(EntityId::Building))
        .or_else(|| mine_at(view, world).map(EntityId::Mine))
}

/// The player's own units whose drawn bodies touch a world-space rectangle.
///
/// The corners arrive in whatever order the drag happened, so they are
/// normalised here rather than at every call site.
pub fn units_in_box(
    view: WorldView<'_>,
    owner: PlayerId,
    corner: Vec2,
    opposite: Vec2,
    bodies: &Bodies,
) -> Vec<UnitId> {
    let (min_x, max_x) = min_max(corner.x, opposite.x);
    let (min_y, max_y) = min_max(corner.y, opposite.y);

    view.units()
        .filter(|(_, unit)| unit.owner == owner)
        .filter(|(_, unit)| {
            let (min, max) = bodies.of(unit.kind).at(to_screen_space(unit.pos));
            min.x <= max_x && max.x >= min_x && min.y <= max_y && max.y >= min_y
        })
        .map(|(id, _)| id)
        .collect()
}

fn min_max(a: f32, b: f32) -> (f32, f32) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}
