//! World state to draw calls.
//!
//! This is the translation layer the crate split exists for: `render` knows
//! sprites and lines, `sim` knows tiles, and neither can see the other.
//!
//! Everything on the map is drawn with the original game's sprites. The only
//! filled rectangles are health bars, and the only lines are the selection
//! outline and the drag box, as in the original.

use crate::pick::footprint_centre;
use crate::pick::to_screen_space;
use crate::sprites::{building_scale, Sprite, Sprites, UNIT_PIXELS_PER_TILE};
use crate::ui::{trains_on_buttons, Layout, Rect};
use render::{Camera, Color, Renderer, Vec2};
use sim::{BuildingId, FxVec2, Outcome, PlayerId, Resource, TilePos, UnitId, WorldView};
use std::collections::{HashMap, HashSet};

/// The outline around a selected unit and the drag rectangle.
pub const SELECTION: Color = Color::rgb(0.95, 0.95, 0.95);
/// The filled part of a health bar.
pub const HEALTH: Color = Color::rgb(0.25, 0.85, 0.35);
/// The empty part of a health bar.
pub const HEALTH_LOST: Color = Color::rgba(0.1, 0.1, 0.1, 0.7);

/// Debug text height in screen pixels.
const OVERLAY_PX: f32 = 16.0;
/// Vertical gap between overlay lines, in screen pixels.
const OVERLAY_LINE: f32 = 22.0;
/// The information panel's text: dark ink on the parchment.
const INK: Color = Color::rgb(0.12, 0.08, 0.04);
/// Height of a health bar, in tiles.
const HEALTH_BAR_HEIGHT: f32 = 0.12;

/// What the application knows that the world does not: whose turn it is and
/// what they have selected.
#[derive(Clone, PartialEq, Debug)]
pub struct Hud<'a> {
    pub active_player: PlayerId,
    pub selection: &'a [UnitId],
    /// The building this player has selected, if any: then no units are.
    pub building: Option<BuildingId>,
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
    draw_buildings(renderer, view, hud, sprites);
    draw_units(renderer, view, previous, alpha, hud, sprites);
    draw_drag_box(renderer, hud);
    draw_interface(renderer, camera, sprites);
    draw_command_panel(renderer, camera, view, hud, sprites);
    draw_info_panel(renderer, camera, view, hud, sprites);
    draw_overlay(renderer, camera, view, hud);
    renderer.end_frame();
}

/// Draws the interface image over the whole window, on top of the map.
///
/// The renderer draws in world units, so the window's corner and size are
/// turned into world units through the camera, as the overlay's text is.
fn draw_interface<R: Renderer>(renderer: &mut R, camera: &Camera, sprites: &Sprites) {
    renderer.sprite(
        sprites.interface.texture,
        camera.screen_to_world(Vec2::ZERO),
        camera.viewport * (1.0 / camera.zoom),
    );
}

/// A screen rectangle as the renderer's world-unit position and size.
fn screen_rect(camera: &Camera, rect: Rect) -> (Vec2, Vec2) {
    (
        camera.screen_to_world(rect.min),
        rect.size * (1.0 / camera.zoom),
    )
}

/// Draws the selected building's buttons: the icon of each unit type it
/// trains, in order, on the command panel.
fn draw_command_panel<R: Renderer>(
    renderer: &mut R,
    camera: &Camera,
    view: WorldView<'_>,
    hud: &Hud<'_>,
    sprites: &Sprites,
) {
    let layout = Layout::new(camera.viewport);
    let trains = trains_on_buttons(view, hud.building, hud.active_player);
    for (kind, button) in trains.into_iter().zip(layout.buttons) {
        let (at, size) = screen_rect(camera, button);
        renderer.sprite(sprites.unit_icon(kind).texture, at, size);
    }
}

/// Draws what the information panel says about the selected building: its
/// icon, name, and hit points, and, for your own building, the unit in
/// training with its progress bar and the units queued behind it.
fn draw_info_panel<R: Renderer>(
    renderer: &mut R,
    camera: &Camera,
    view: WorldView<'_>,
    hud: &Hud<'_>,
    sprites: &Sprites,
) {
    let Some(building) = hud.building.and_then(|id| view.building(id)) else {
        draw_unit_info(renderer, camera, view, hud, sprites);
        return;
    };
    let layout = Layout::new(camera.viewport);
    let stats = view.content().building(building.kind);
    let mut icon = |texture, rect| {
        let (at, size) = screen_rect(camera, rect);
        renderer.sprite(texture, at, size);
    };
    icon(
        sprites.building_icon(building.kind).texture,
        layout.building_icon,
    );
    for (text, at) in [
        (stats.name.clone(), layout.name_at),
        (
            format!("{}/{}", building.hp.max(0), stats.max_hp),
            layout.hit_points_at,
        ),
    ] {
        renderer.text(&text, camera.screen_to_world(at), layout.text_px, INK);
    }

    // What a building trains is its owner's business.
    if building.owner != hud.active_player {
        return;
    }
    let Some((&head, waiting)) = building.queue.split_first() else {
        return;
    };
    let mut icon = |texture, rect| {
        let (at, size) = screen_rect(camera, rect);
        renderer.sprite(texture, at, size);
    };
    icon(sprites.unit_icon(head).texture, layout.training_icon);
    let total = view.content().unit(head).train_ticks.max(1);
    let done = total.saturating_sub(building.train_ticks);
    // Frame n is n percent full; a unit counted down to its last tick shows
    // the fullest frame there is.
    let frame =
        (done as usize * 100 / total as usize).min(sprites.progress.len().saturating_sub(1));
    if let Some(bar) = sprites.progress.get(frame) {
        icon(bar.texture, layout.progress_bar);
    }
    for (&kind, slot) in waiting.iter().zip(layout.queue_icons) {
        icon(sprites.unit_icon(kind).texture, slot);
    }
}

/// Draws what the information panel says about the first selected unit:
/// its icon, name, and hit points, and the load a gatherer carries.
fn draw_unit_info<R: Renderer>(
    renderer: &mut R,
    camera: &Camera,
    view: WorldView<'_>,
    hud: &Hud<'_>,
    sprites: &Sprites,
) {
    let Some(unit) = hud.selection.first().and_then(|id| view.unit(*id)) else {
        return;
    };
    let layout = Layout::new(camera.viewport);
    let stats = view.content().unit(unit.kind);
    let (at, size) = screen_rect(camera, layout.building_icon);
    renderer.sprite(sprites.unit_icon(unit.kind).texture, at, size);
    let mut lines = vec![
        (stats.name.clone(), layout.name_at),
        (
            format!("{}/{}", unit.hp.max(0), stats.max_hp),
            layout.hit_points_at,
        ),
    ];
    if unit.carrying > 0 {
        lines.push((
            format!("carrying {} {}", unit.carrying, unit.carried.name()),
            layout.carrying_at,
        ));
    }
    for (text, at) in lines {
        renderer.text(&text, camera.screen_to_world(at), layout.text_px, INK);
    }
}

/// The top bar's text: light on the dark slots.
const BAR_TEXT: Color = Color::rgb(0.95, 0.93, 0.85);

/// Draws the active player's resources in the top bar's slots, and the debug
/// readout just below it.
///
/// Positioned by converting a screen offset back into world units, so it stays
/// pinned to the window while the camera moves under it.
fn draw_overlay<R: Renderer>(
    renderer: &mut R,
    camera: &Camera,
    view: WorldView<'_>,
    hud: &Hud<'_>,
) {
    let layout = Layout::new(camera.viewport);
    let stock = view.resources(hud.active_player);
    for resource in Resource::ALL {
        renderer.text(
            &stock.get(resource).to_string(),
            camera.screen_to_world(layout.resource_at(resource)),
            layout.bar_text_px,
            BAR_TEXT,
        );
    }

    let mut lines = vec![format!(
        "player {}  tick {}  zoom {:.1}  {}",
        hud.active_player.number(),
        view.tick(),
        camera.zoom,
        if hud.replaying { "replay" } else { "live" }
    )];
    match view.outcome() {
        Some(Outcome::Won(winner)) => lines.push(format!("player {} wins", winner.number())),
        Some(Outcome::Draw) => lines.push("draw".to_string()),
        None => {}
    }
    for (row, line) in lines.iter().enumerate() {
        let at = layout.debug_at + Vec2::new(0.0, OVERLAY_LINE * row as f32);
        renderer.text(line, camera.screen_to_world(at), OVERLAY_PX, Color::WHITE);
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
            view.sites()
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
    for (_, mine) in view.sites() {
        stand(
            renderer,
            &sprites.gold_mine,
            to_screen_space(mine.center()),
            1.0 / UNIT_PIXELS_PER_TILE,
        );
    }
}

fn draw_buildings<R: Renderer>(
    renderer: &mut R,
    view: WorldView<'_>,
    hud: &Hud<'_>,
    sprites: &Sprites,
) {
    // Back to front, as picking assumes: a building lower on the screen is
    // nearer the viewer, and its roof covers what stands behind it.
    let mut buildings: Vec<_> = view.buildings().collect();
    buildings.sort_by(|(_, a), (_, b)| footprint_centre(a).y.total_cmp(&footprint_centre(b).y));
    for (id, building) in buildings {
        let size = building.size as f32;
        let origin = Vec2::new(building.origin.x as f32, building.origin.y as f32);
        // Scaled to the footprint's width, with the sprite's ground point on
        // the footprint's centre, so that it covers the tiles it blocks.
        let sprite = sprites.building(building.kind, building.owner);
        let scale = building_scale(sprite, building.size);
        let (top_left, drawn) = stand(renderer, sprite, footprint_centre(building), scale);
        if hud.building == Some(id) {
            outline(renderer, top_left, top_left + drawn);
        }
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
    let mut units: Vec<_> = view
        .units()
        .map(|(id, unit)| {
            let now = to_screen_space(unit.pos);
            // A unit that did not exist last tick has nothing to interpolate
            // from, so it appears at its spawn position rather than sliding in
            // from the origin.
            let before = previous.get(&id).map_or(now, |pos| to_screen_space(*pos));
            (id, unit, before + (now - before) * alpha)
        })
        .collect();
    // Back to front, as picking assumes: the unit lower on the screen is in
    // front.
    units.sort_by(|(_, _, a), (_, _, b)| a.y.total_cmp(&b.y));
    for (id, unit, at) in units {
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
