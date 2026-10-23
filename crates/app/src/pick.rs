//! Turning a point or a rectangle on screen into the entities under it.
//!
//! Picking is presentation, not simulation: the answer depends on where the
//! camera is, which `sim` has no business knowing.

use render::Vec2;
use sim::{BuildingId, EntityId, FxVec2, MineId, PlayerId, TilePos, UnitId, WorldView};

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

/// How close to a unit's centre a click counts as hitting it, in tiles.
///
/// Larger than the unit's own radius, because clicking a quarter-tile dot at
/// low zoom is otherwise a test of the player's mouse hand.
const UNIT_PICK_RADIUS: f32 = 0.6;

/// The tile a world position falls in.
pub fn tile_at(world: Vec2) -> TilePos {
    TilePos::new(world.x.floor() as i32, world.y.floor() as i32)
}

/// The unit closest to `world`, within picking distance.
pub fn unit_at(view: WorldView<'_>, world: Vec2) -> Option<UnitId> {
    view.units()
        .filter_map(|(id, unit)| {
            let offset = to_screen_space(unit.pos) - world;
            let distance = offset.x * offset.x + offset.y * offset.y;
            (distance <= UNIT_PICK_RADIUS * UNIT_PICK_RADIUS).then_some((id, distance))
        })
        // Compared as bits after ordering by distance: `f32` has no total
        // order, and two candidates at identical distance must still resolve
        // the same way every frame.
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
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
pub fn entity_at(view: WorldView<'_>, world: Vec2) -> Option<EntityId> {
    unit_at(view, world)
        .map(EntityId::Unit)
        .or_else(|| building_at(view, world).map(EntityId::Building))
        .or_else(|| mine_at(view, world).map(EntityId::Mine))
}

/// The player's own units inside a screen-space rectangle.
///
/// The corners arrive in whatever order the drag happened, so they are
/// normalised here rather than at every call site.
pub fn units_in_box(
    view: WorldView<'_>,
    owner: PlayerId,
    corner: Vec2,
    opposite: Vec2,
) -> Vec<UnitId> {
    let (min_x, max_x) = min_max(corner.x, opposite.x);
    let (min_y, max_y) = min_max(corner.y, opposite.y);

    view.units()
        .filter(|(_, unit)| unit.owner == owner)
        .filter(|(_, unit)| {
            let pos = to_screen_space(unit.pos);
            (min_x..=max_x).contains(&pos.x) && (min_y..=max_y).contains(&pos.y)
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
