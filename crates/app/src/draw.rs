//! World state to draw calls.
//!
//! This is the translation layer the crate split exists for: `render` knows
//! sprites and lines, `sim` knows tiles, and neither can see the other.
//!
//! Everything on the map is drawn with the original game's sprites. The only
//! filled rectangles are health bars, and the only lines are the selection
//! outline and the drag box, as in the original.

use crate::pick::to_screen_space;
use crate::sprites::{Sprite, Sprites, UNIT_PIXELS_PER_TILE};
use render::{Camera, Color, Renderer, Vec2};
use sim::{FxVec2, Outcome, PlayerId, Resource, Resources, TilePos, UnitId, WorldView};
use std::collections::{HashMap, HashSet};

/// The outline around a selected unit and the drag rectangle.
pub const SELECTION: Color = Color::rgb(0.95, 0.95, 0.95);
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
    sprites: &Sprites,
) {
    renderer.begin_frame(camera);
    draw_terrain(renderer, camera, view, sprites);
    draw_mines(renderer, view, sprites);
    draw_buildings(renderer, view, sprites);
    draw_units(renderer, view, previous, alpha, hud, sprites);
    draw_drag_box(renderer, hud);
    draw_overlay(renderer, camera, view, hud);
    renderer.end_frame();
}

/// `food 0  wood 0  gold 200  stone 0`, in the fixed resource order.
fn resource_line(stock: Resources) -> String {
    Resource::ALL
        .into_iter()
        .map(|r| format!("{} {}", r.name(), stock.get(r)))
        .collect::<Vec<_>>()
        .join("  ")
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
            "player {}  {}  selected {}",
            hud.active_player.number(),
            resource_line(view.resources(hud.active_player)),
            hud.selection.len()
        ),
        "tab switch  1 villager  2 soldier  right-click order".to_string(),
    ];
    match view.outcome() {
        Some(Outcome::Won(winner)) => lines.push(format!("player {} wins", winner.number())),
        Some(Outcome::Draw) => lines.push("draw".to_string()),
        None => {}
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

/// Draws the ground the camera can see, then the trees standing on it.
///
/// Culling is by tile range rather than per-tile testing: a 128x128 map is
/// 16k tiles, and at close zoom fewer than a hundred of them are on screen.
fn draw_terrain<R: Renderer>(
    renderer: &mut R,
    camera: &Camera,
    view: WorldView<'_>,
    sprites: &Sprites,
) {
    let (min, max) = visible_tiles(camera, view.width(), view.height());
    for y in min.y..=max.y {
        for x in min.x..=max.x {
            if view.tile(TilePos::new(x, y)).is_some() {
                let at = Vec2::new(x as f32, y as f32);
                renderer.sprite(sprites.grass_at(x, y), at, Vec2::new(1.0, 1.0));
            }
        }
    }

    // Rock from the map is drawn as forest. Buildings and mines also block
    // their tiles for pathing, but stand on open ground, so their footprints
    // get no trees.
    let covered: HashSet<TilePos> = view
        .buildings()
        .flat_map(|(_, b)| b.footprint().collect::<Vec<_>>())
        .chain(
            view.mines()
                .flat_map(|(_, m)| m.footprint().collect::<Vec<_>>()),
        )
        .collect();
    // A tree is taller and wider than its tile, so trees just outside the
    // view can still reach into it. Rows are drawn top to bottom so that
    // nearer trees overlap farther ones.
    let (min, max) = (
        TilePos::new(min.x - TREE_REACH, min.y),
        TilePos::new(max.x + TREE_REACH, max.y + TREE_REACH),
    );
    for y in min.y..=max.y {
        for x in min.x..=max.x {
            let pos = TilePos::new(x, y);
            let Some(tile) = view.tile(pos) else { continue };
            if !tile.passable && !covered.contains(&pos) {
                let centre = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                stand(renderer, &sprites.tree, centre, 1.0 / UNIT_PIXELS_PER_TILE);
            }
        }
    }
}

/// Tiles beyond the view whose trees may still reach into it.
const TREE_REACH: i32 = 3;

/// Draws `sprite` with its ground point on `ground`, at `scale` tiles per
/// sprite pixel. Returns where the sprite was drawn and its size.
fn stand<R: Renderer>(renderer: &mut R, sprite: &Sprite, ground: Vec2, scale: f32) -> (Vec2, Vec2) {
    let origin = ground - sprite.hotspot * scale;
    let size = Vec2::new(sprite.width, sprite.height) * scale;
    renderer.sprite(sprite.texture, origin, size);
    (origin, size)
}

fn draw_mines<R: Renderer>(renderer: &mut R, view: WorldView<'_>, sprites: &Sprites) {
    for (_, mine) in view.mines() {
        stand(
            renderer,
            &sprites.gold_mine,
            to_screen_space(mine.center()),
            1.0 / UNIT_PIXELS_PER_TILE,
        );
    }
}

fn draw_buildings<R: Renderer>(renderer: &mut R, view: WorldView<'_>, sprites: &Sprites) {
    for (_, building) in view.buildings() {
        let size = building.kind.size() as f32;
        let origin = Vec2::new(building.origin.x as f32, building.origin.y as f32);
        // Scaled to the footprint's width, with the sprite's ground point on
        // the footprint's centre, so that it covers the tiles it blocks.
        let sprite = sprites.building(building.kind, building.owner);
        let centre = origin + Vec2::new(size, size) * 0.5;
        stand(renderer, sprite, centre, size / sprite.width);
        health_bar(
            renderer,
            origin,
            size,
            building.hp,
            view.content().building(building.kind).max_hp,
        );
    }
}

fn draw_units<R: Renderer>(
    renderer: &mut R,
    view: WorldView<'_>,
    previous: &HashMap<UnitId, FxVec2>,
    alpha: f32,
    hud: &Hud<'_>,
    sprites: &Sprites,
) {
    for (id, unit) in view.units() {
        let now = to_screen_space(unit.pos);
        // A unit that did not exist last tick has nothing to interpolate from,
        // so it appears at its spawn position rather than sliding in from the
        // origin.
        let before = previous.get(&id).map_or(now, |pos| to_screen_space(*pos));
        let at = before + (now - before) * alpha;
        let sprite = sprites.unit(unit.kind, unit.owner);
        let (origin, size) = stand(renderer, sprite, at, 1.0 / UNIT_PIXELS_PER_TILE);

        if hud.selection.contains(&id) {
            outline(renderer, origin, origin + size);
        }
        health_bar(
            renderer,
            origin,
            size.x,
            unit.hp,
            view.content().unit(unit.kind).max_hp,
        );
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

/// Draws the rectangle the player is dragging.
fn draw_drag_box<R: Renderer>(renderer: &mut R, hud: &Hud<'_>) {
    if let Some((from, to)) = hud.drag_box {
        outline(renderer, from, to);
    }
}

/// A thin rectangle between two corners, as four lines.
fn outline<R: Renderer>(renderer: &mut R, from: Vec2, to: Vec2) {
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
