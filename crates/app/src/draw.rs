//! World state to draw calls.
//!
//! This is the translation layer the crate split exists for: `render` knows
//! quads, `sim` knows tiles, and neither can see the other.

use crate::pick::to_screen_space;
use render::{Camera, Color, Renderer, Vec2};
use sim::{FxVec2, PlayerId, TilePos, UnitId, UnitKind, WorldView};
use std::collections::HashMap;

/// A passable tile.
pub const TILE_OPEN: Color = Color::rgb(0.20, 0.36, 0.22);
/// A tile a unit cannot walk on.
pub const TILE_BLOCKED: Color = Color::rgb(0.32, 0.29, 0.26);
/// Player one's units and buildings.
pub const PLAYER_ONE: Color = Color::rgb(0.30, 0.55, 0.95);
/// Player two's units and buildings.
pub const PLAYER_TWO: Color = Color::rgb(0.90, 0.35, 0.30);
/// A gold mine.
pub const MINE: Color = Color::rgb(0.85, 0.72, 0.20);
/// The outline around a selected unit and the drag rectangle.
pub const SELECTION: Color = Color::rgb(0.95, 0.95, 0.40);
/// The filled part of a health bar.
pub const HEALTH: Color = Color::rgb(0.25, 0.85, 0.35);
/// The empty part of a health bar.
pub const HEALTH_LOST: Color = Color::rgba(0.1, 0.1, 0.1, 0.7);

/// Screen-space offset of the debug overlay from the top-left corner, in pixels.
const OVERLAY_ORIGIN: Vec2 = Vec2::new(8.0, 24.0);
/// Overlay text height in screen pixels.
const OVERLAY_PX: f32 = 20.0;
/// Vertical gap between overlay lines, in screen pixels.
const OVERLAY_LINE: f32 = 22.0;
/// Villager and soldier body size, in tiles.
const UNIT_SIZE: Vec2 = Vec2::new(0.5, 0.5);
/// Height of a health bar, in tiles.
const HEALTH_BAR_HEIGHT: f32 = 0.12;

/// What the application knows that the world does not: whose turn it is and
/// what they have selected.
#[derive(Clone, PartialEq, Debug)]
pub struct Hud<'a> {
    pub active_player: PlayerId,
    pub selection: &'a [UnitId],
    /// Corners of the selection rectangle being dragged, in world units.
    pub drag_box: Option<(Vec2, Vec2)>,
    /// Whether the simulation is being driven by a recording rather than input.
    pub replaying: bool,
}

/// Draws one complete frame.
///
/// `previous` holds unit positions as of the last completed tick and `alpha`
/// is how far the frame sits between that tick and the next one. Interpolating
/// between them is what lets a 20 Hz simulation look smooth at 144 Hz without
/// the simulation knowing anything about frame rate.
pub fn draw_world<R: Renderer>(
    renderer: &mut R,
    camera: &Camera,
    view: WorldView<'_>,
    previous: &HashMap<UnitId, FxVec2>,
    alpha: f32,
    hud: &Hud<'_>,
) {
    renderer.begin_frame(camera);
    draw_terrain(renderer, camera, view);
    draw_mines(renderer, view);
    draw_buildings(renderer, view);
    draw_units(renderer, view, previous, alpha, hud);
    draw_drag_box(renderer, hud);
    draw_overlay(renderer, camera, view, hud);
    renderer.end_frame();
}

fn player_color(player: PlayerId) -> Color {
    match player {
        PlayerId::One => PLAYER_ONE,
        PlayerId::Two => PLAYER_TWO,
    }
}

/// Draws the debug readout.
///
/// Positioned by converting a screen offset back into world units, so it stays
/// pinned to the corner of the window while the camera moves under it.
fn draw_overlay<R: Renderer>(
    renderer: &mut R,
    camera: &Camera,
    view: WorldView<'_>,
    hud: &Hud<'_>,
) {
    let mut lines = vec![
        format!(
            "tick {}  zoom {:.1}  {}",
            view.tick(),
            camera.zoom,
            if hud.replaying { "replay" } else { "live" }
        ),
        format!(
            "player {}  gold {}  selected {}",
            match hud.active_player {
                PlayerId::One => 1,
                PlayerId::Two => 2,
            },
            view.gold(hud.active_player),
            hud.selection.len()
        ),
        "tab switch  1 villager  2 soldier  right-click order".to_string(),
    ];
    if let Some(winner) = view.outcome() {
        lines.push(format!(
            "player {} wins",
            match winner {
                PlayerId::One => 1,
                PlayerId::Two => 2,
            }
        ));
    }

    for (row, line) in lines.iter().enumerate() {
        let offset = Vec2::new(
            OVERLAY_ORIGIN.x,
            OVERLAY_ORIGIN.y + OVERLAY_LINE * row as f32,
        );
        renderer.text(
            line,
            camera.screen_to_world(offset),
            OVERLAY_PX,
            Color::WHITE,
        );
    }
}

/// Draws the tiles the camera can see.
///
/// Culling is by tile range rather than per-tile testing: a 128x128 map is
/// 16k quads, and at close zoom fewer than a hundred of them are on screen.
fn draw_terrain<R: Renderer>(renderer: &mut R, camera: &Camera, view: WorldView<'_>) {
    let (min, max) = visible_tiles(camera, view.width(), view.height());

    for y in min.y..=max.y {
        for x in min.x..=max.x {
            let pos = TilePos::new(x, y);
            let Some(tile) = view.tile(pos) else { continue };
            let color = if tile.passable {
                TILE_OPEN
            } else {
                TILE_BLOCKED
            };
            renderer.quad(Vec2::new(x as f32, y as f32), Vec2::new(1.0, 1.0), color);
        }
    }
}

fn draw_mines<R: Renderer>(renderer: &mut R, view: WorldView<'_>) {
    for (_, mine) in view.mines() {
        let size = sim::Mine::SIZE as f32;
        renderer.quad(
            Vec2::new(mine.origin.x as f32, mine.origin.y as f32),
            Vec2::new(size, size),
            MINE,
        );
    }
}

fn draw_buildings<R: Renderer>(renderer: &mut R, view: WorldView<'_>) {
    for (_, building) in view.buildings() {
        let size = building.kind.size() as f32;
        let origin = Vec2::new(building.origin.x as f32, building.origin.y as f32);
        renderer.quad(origin, Vec2::new(size, size), player_color(building.owner));
        health_bar(renderer, origin, size, building.hp, building.kind.max_hp());
    }
}

fn draw_units<R: Renderer>(
    renderer: &mut R,
    view: WorldView<'_>,
    previous: &HashMap<UnitId, FxVec2>,
    alpha: f32,
    hud: &Hud<'_>,
) {
    for (id, unit) in view.units() {
        let now = to_screen_space(unit.pos);
        // A unit that did not exist last tick has nothing to interpolate from,
        // so it appears at its spawn position rather than sliding in from the
        // origin.
        let before = previous.get(&id).map_or(now, |pos| to_screen_space(*pos));
        let at = before + (now - before) * alpha;
        let origin = at - UNIT_SIZE * 0.5;

        if hud.selection.contains(&id) {
            renderer.quad(
                origin - Vec2::new(0.1, 0.1),
                UNIT_SIZE + Vec2::new(0.2, 0.2),
                SELECTION,
            );
        }
        renderer.quad(origin, UNIT_SIZE, player_color(unit.owner));
        health_bar(
            renderer,
            origin - Vec2::new(0.0, 0.25),
            UNIT_SIZE.x,
            unit.hp,
            unit.kind.stats().max_hp,
        );

        // A loaded villager carries a visible nugget, so that the gather loop
        // can be watched rather than inferred from the gold counter.
        if unit.kind == UnitKind::Villager && unit.carrying > 0 {
            renderer.quad(origin + Vec2::new(0.15, 0.15), Vec2::new(0.2, 0.2), MINE);
        }
    }
}

/// Draws a two-part bar above something that can be damaged.
fn health_bar<R: Renderer>(renderer: &mut R, origin: Vec2, width: f32, hp: i32, max_hp: i32) {
    if hp >= max_hp {
        return;
    }
    let top = origin - Vec2::new(0.0, HEALTH_BAR_HEIGHT + 0.05);
    let fraction = (hp.max(0) as f32) / (max_hp.max(1) as f32);
    renderer.quad(top, Vec2::new(width, HEALTH_BAR_HEIGHT), HEALTH_LOST);
    renderer.quad(top, Vec2::new(width * fraction, HEALTH_BAR_HEIGHT), HEALTH);
}

/// Draws the rectangle the player is dragging, as four lines.
fn draw_drag_box<R: Renderer>(renderer: &mut R, hud: &Hud<'_>) {
    let Some((from, to)) = hud.drag_box else {
        return;
    };
    let corners = [
        Vec2::new(from.x, from.y),
        Vec2::new(to.x, from.y),
        Vec2::new(to.x, to.y),
        Vec2::new(from.x, to.y),
    ];
    for i in 0..4 {
        renderer.line(corners[i], corners[(i + 1) % 4], SELECTION);
    }
}

/// The inclusive tile range the camera covers, clipped to the map.
///
/// The range is inclusive on both ends so that a tile straddling the edge of
/// the viewport is drawn rather than leaving a gap at the border.
fn visible_tiles(camera: &Camera, map_width: i32, map_height: i32) -> (TilePos, TilePos) {
    let top_left = camera.screen_to_world(Vec2::ZERO);
    let bottom_right = camera.screen_to_world(camera.viewport);

    let clamp = |value: f32, limit: i32| (value.floor() as i32).clamp(0, limit - 1);
    (
        TilePos::new(clamp(top_left.x, map_width), clamp(top_left.y, map_height)),
        TilePos::new(
            clamp(bottom_right.x, map_width),
            clamp(bottom_right.y, map_height),
        ),
    )
}
