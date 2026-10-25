//! Turning a point or a rectangle on screen into the entities under it.
//!
//! Picking is presentation, not simulation: the answer depends on where the
//! camera is, which `sim` has no business knowing.

use render::Vec2;
use sim::{
    Building, BuildingId, BuildingKind, EntityId, FxVec2, MineId, PlayerId, TilePos, UnitId,
    UnitKind, WorldView,
};

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

    /// This body grown, about its centre, to at least `min` tiles each way.
    pub fn at_least(&self, min: f32) -> Body {
        let grow = |size: f32| (min - size).max(0.0);
        let (dx, dy) = (grow(self.size.x), grow(self.size.y));
        Body {
            offset: self.offset - Vec2::new(dx, dy) * 0.5,
            size: self.size + Vec2::new(dx, dy),
        }
    }

    fn at(&self, ground: Vec2) -> (Vec2, Vec2) {
        let min = ground + self.offset;
        (min, min + self.size)
    }
}

/// Each unit type's body, indexed by its ID. The application builds these
/// from the sprites, so that picking matches drawing; a type with no entry
/// uses the fallback, by default a square around the feet.
///
/// Buildings too, but measured from the centre of the footprint, where the
/// sprite's ground point is drawn. A building type with no body is picked by
/// its footprint alone.
#[derive(Clone, PartialEq, Debug)]
pub struct Bodies {
    pub units: Vec<Body>,
    pub fallback: Body,
    pub buildings: Vec<Body>,
}

impl Default for Bodies {
    fn default() -> Bodies {
        Bodies::all(Body::around_feet(0.6))
    }
}

impl Bodies {
    /// The same body for every unit type.
    pub const fn all(body: Body) -> Bodies {
        Bodies {
            units: Vec::new(),
            fallback: body,
            buildings: Vec::new(),
        }
    }

    /// Every body grown to at least `min` tiles each way.
    pub fn at_least(&self, min: f32) -> Bodies {
        Bodies {
            units: self.units.iter().map(|b| b.at_least(min)).collect(),
            fallback: self.fallback.at_least(min),
            buildings: self.buildings.clone(),
        }
    }

    pub fn building(&self, kind: BuildingKind) -> Option<Body> {
        self.buildings.get(kind.index()).copied()
    }

    pub fn of(&self, kind: UnitKind) -> Body {
        self.units
            .get(kind.index())
            .copied()
            .unwrap_or(self.fallback)
    }
}

/// The tile a world position falls in.
pub fn tile_at(world: Vec2) -> TilePos {
    TilePos::new(world.x.floor() as i32, world.y.floor() as i32)
}

/// The unit under `world`.
///
/// A click on a drawn body picks that unit; where drawn bodies overlap, the
/// one drawn in front (lower on screen) wins. Off every drawn body, each
/// target is grown to at least `min` tiles each way, so that a unit drawn a
/// few pixels wide when zoomed out can still be clicked, and the unit whose
/// body centre is nearest wins. The grown targets never take a click from a
/// body the player actually clicked on.
pub fn unit_at(view: WorldView<'_>, world: Vec2, bodies: &Bodies, min: f32) -> Option<UnitId> {
    let inside = |(lo, hi): (Vec2, Vec2)| {
        (lo.x..=hi.x).contains(&world.x) && (lo.y..=hi.y).contains(&world.y)
    };
    // `total_cmp` throughout because `f32` has no total order, and the same
    // click must resolve the same way every frame.
    let on_body = view
        .units()
        .filter_map(|(id, unit)| {
            let ground = to_screen_space(unit.pos);
            inside(bodies.of(unit.kind).at(ground)).then_some((id, ground.y))
        })
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(id, _)| id);
    on_body.or_else(|| {
        view.units()
            .filter_map(|(id, unit)| {
                let body = bodies.of(unit.kind);
                let ground = to_screen_space(unit.pos);
                inside(body.at_least(min).at(ground)).then(|| {
                    let centre = ground + body.offset + body.size * 0.5;
                    let gap = centre - world;
                    (id, gap.x * gap.x + gap.y * gap.y)
                })
            })
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(id, _)| id)
    })
}

/// The building under `world`: on its footprint or on its drawn body.
///
/// A tall building's roof is drawn over the tiles behind it, so where bodies
/// overlap, the one drawn in front (its footprint lower on screen) wins.
pub fn building_at(view: WorldView<'_>, world: Vec2, bodies: &Bodies) -> Option<BuildingId> {
    let tile = tile_at(world);
    view.buildings()
        .filter(|(_, b)| {
            b.footprint().any(|t| t == tile)
                || bodies.building(b.kind).is_some_and(|body| {
                    let (lo, hi) = body.at(footprint_centre(b));
                    (lo.x..=hi.x).contains(&world.x) && (lo.y..=hi.y).contains(&world.y)
                })
        })
        .max_by(|(_, a), (_, b)| footprint_centre(a).y.total_cmp(&footprint_centre(b).y))
        .map(|(id, _)| id)
}

/// The centre of a building's footprint, where its sprite stands.
pub fn footprint_centre(building: &Building) -> Vec2 {
    let half = building.size as f32 / 2.0;
    Vec2::new(
        building.origin.x as f32 + half,
        building.origin.y as f32 + half,
    )
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
pub fn entity_at(view: WorldView<'_>, world: Vec2, bodies: &Bodies, min: f32) -> Option<EntityId> {
    unit_at(view, world, bodies, min)
        .map(EntityId::Unit)
        .or_else(|| building_at(view, world, bodies).map(EntityId::Building))
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
