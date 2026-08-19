//! World state to draw calls.
//!
//! This is the translation layer the crate split exists for: `render` knows
//! quads, `sim` knows tiles, and neither can see the other.

use render::{Camera, Color, Renderer, Vec2};
use sim::{TilePos, WorldView};

/// A passable tile.
pub const TILE_OPEN: Color = Color::rgb(0.20, 0.36, 0.22);
/// A tile a unit cannot walk on.
pub const TILE_BLOCKED: Color = Color::rgb(0.32, 0.29, 0.26);

/// Screen-space offset of the debug overlay from the top-left corner, in pixels.
const OVERLAY_ORIGIN: Vec2 = Vec2::new(8.0, 24.0);
/// Overlay text height in screen pixels.
const OVERLAY_PX: f32 = 20.0;

/// Draws one complete frame of the world.
pub fn draw_world<R: Renderer>(renderer: &mut R, camera: &Camera, view: WorldView<'_>) {
    renderer.begin_frame(camera);
    draw_terrain(renderer, camera, view);
    draw_overlay(renderer, camera, view);
    renderer.end_frame();
}

/// Draws the debug readout.
///
/// Positioned by converting a screen offset back into world units, so it stays
/// pinned to the corner of the window while the camera moves under it.
fn draw_overlay<R: Renderer>(renderer: &mut R, camera: &Camera, view: WorldView<'_>) {
    let text = format!("tick {}  zoom {:.1}", view.tick(), camera.zoom);
    let pos = camera.screen_to_world(OVERLAY_ORIGIN);
    renderer.text(&text, pos, OVERLAY_PX, Color::WHITE);
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
