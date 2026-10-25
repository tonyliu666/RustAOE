//! Translating world state into draw calls, asserted headlessly.

mod common;

use app::{draw_world, Hud};
use render::{Camera, DrawCall, RecordingRenderer, Vec2};
use sim::{FxVec2, MapSpec, PlayerId, UnitId, World};
use std::collections::HashMap;

/// Draws a frame with no selection, no drag, and no interpolation.
///
/// Most of these tests are about terrain and the overlay, and threading the
/// same four empty arguments through every one of them would bury what each
/// is actually asserting.
fn draw(renderer: &mut RecordingRenderer, camera: &Camera, world: &World) {
    let previous: HashMap<UnitId, FxVec2> = HashMap::new();
    let sprites = common::sprites(renderer);
    draw_world(
        renderer,
        camera,
        world.view(),
        &previous,
        0.0,
        &plain_hud(),
        &sprites,
    );
}

/// The ground tiles drawn: one 1x1 grass sprite per tile.
fn ground(renderer: &RecordingRenderer) -> Vec<(Vec2, Vec2)> {
    renderer
        .sprites()
        .filter(|(_, _, size)| *size == Vec2::new(1.0, 1.0))
        .map(|(_, pos, size)| (pos, size))
        .collect()
}

fn plain_hud() -> Hud<'static> {
    Hud {
        active_player: PlayerId::ONE,
        selection: &[],
        drag_box: None,
        replaying: false,
    }
}

/// A camera wide enough that every tile of an 8x8 map is on screen.
fn wide_camera() -> Camera {
    Camera::new(Vec2::new(4.0, 4.0), 10.0, Vec2::new(400.0, 400.0))
}

fn world(map: MapSpec) -> World {
    World::new(1, map)
}

#[test]
fn a_frame_is_opened_with_the_camera_and_closed_again() {
    let mut renderer = RecordingRenderer::new();
    let world = world(MapSpec::new(8, 8));
    let camera = wide_camera();

    draw(&mut renderer, &camera, &world);

    assert_eq!(
        renderer.calls().first(),
        Some(&DrawCall::BeginFrame(camera))
    );
    assert_eq!(renderer.calls().last(), Some(&DrawCall::EndFrame));
    assert_eq!(renderer.frame_count(), 1);
}

#[test]
fn every_visible_tile_is_drawn_once() {
    let mut renderer = RecordingRenderer::new();
    let world = world(MapSpec::new(8, 8));

    draw(&mut renderer, &wide_camera(), &world);

    assert_eq!(ground(&renderer).len(), 64);
}

#[test]
fn a_tile_is_drawn_one_by_one_at_its_own_coordinates() {
    let mut renderer = RecordingRenderer::new();
    let world = world(MapSpec::new(8, 8));

    draw(&mut renderer, &wide_camera(), &world);

    assert!(
        ground(&renderer).contains(&(Vec2::new(3.0, 5.0), Vec2::new(1.0, 1.0))),
        "tile 3,5 should be drawn at world 3,5"
    );
}

#[test]
fn plain_ground_draws_no_filled_rectangles() {
    let mut renderer = RecordingRenderer::new();
    let world = world(MapSpec::new(8, 8));

    draw(&mut renderer, &wide_camera(), &world);

    assert_eq!(renderer.quads().count(), 0);
}

#[test]
fn tiles_outside_the_viewport_are_not_drawn() {
    let mut renderer = RecordingRenderer::new();
    let world = world(MapSpec::new(128, 128));
    // 400 pixels at 40 pixels per tile shows 10 tiles across, so at most 12x12
    // once partly visible edge tiles are included.
    let camera = Camera::new(Vec2::new(64.0, 64.0), 40.0, Vec2::new(400.0, 400.0));

    draw(&mut renderer, &camera, &world);

    let drawn = ground(&renderer).len();
    assert!(
        drawn >= 100,
        "the visible 10x10 must be covered, got {drawn}"
    );
    assert!(drawn <= 144, "off-screen tiles must be culled, got {drawn}");
}

#[test]
fn a_partly_visible_tile_at_the_viewport_edge_is_still_drawn() {
    let mut renderer = RecordingRenderer::new();
    let world = world(MapSpec::new(16, 16));
    // Half a tile of offset puts the left column half off screen.
    let camera = Camera::new(Vec2::new(8.5, 8.0), 100.0, Vec2::new(400.0, 400.0));

    draw(&mut renderer, &camera, &world);

    assert!(
        ground(&renderer)
            .iter()
            .any(|(pos, _)| *pos == Vec2::new(6.0, 6.0)),
        "the half-visible tile 6,6 should be drawn, not culled"
    );
}

#[test]
fn nothing_is_drawn_beyond_the_map_edge() {
    let mut renderer = RecordingRenderer::new();
    let world = world(MapSpec::new(4, 4));
    let camera = Camera::new(Vec2::new(0.0, 0.0), 10.0, Vec2::new(400.0, 400.0));

    draw(&mut renderer, &camera, &world);

    for (pos, _) in ground(&renderer) {
        assert!(
            pos.x >= 0.0 && pos.y >= 0.0 && pos.x < 4.0 && pos.y < 4.0,
            "drew a tile at {pos:?}, which is off the map"
        );
    }
    assert_eq!(ground(&renderer).len(), 16);
}

#[test]
fn the_frame_carries_a_debug_overlay_naming_the_tick() {
    let mut renderer = RecordingRenderer::new();
    let mut world = world(MapSpec::new(8, 8));
    world.step(&[]);
    world.step(&[]);
    world.step(&[]);

    draw(&mut renderer, &wide_camera(), &world);

    let (text, _) = renderer.texts().next().expect("an overlay should be drawn");
    assert!(text.contains("tick 3"), "overlay was {text:?}");
}

#[test]
fn the_overlay_is_drawn_inside_the_frame_so_the_camera_still_applies() {
    let mut renderer = RecordingRenderer::new();
    let world = world(MapSpec::new(8, 8));

    draw(&mut renderer, &wide_camera(), &world);

    let overlay = renderer
        .calls()
        .iter()
        .position(|call| matches!(call, DrawCall::Text { .. }))
        .expect("an overlay should be drawn");
    let end = renderer
        .calls()
        .iter()
        .position(|call| *call == DrawCall::EndFrame)
        .expect("the frame should be closed");
    assert!(overlay < end);
}

#[test]
fn the_overlay_sits_at_a_fixed_corner_of_the_screen_whatever_the_camera_shows() {
    let mut renderer = RecordingRenderer::new();
    let world = world(MapSpec::new(64, 64));
    let near = Camera::new(Vec2::new(10.0, 10.0), 40.0, Vec2::new(400.0, 400.0));
    let far = Camera::new(Vec2::new(50.0, 30.0), 8.0, Vec2::new(400.0, 400.0));

    draw(&mut renderer, &near, &world);
    let near_pos = renderer.texts().next().expect("overlay").1;
    renderer.clear();
    draw(&mut renderer, &far, &world);
    let far_pos = renderer.texts().next().expect("overlay").1;

    let to_screen = |cam: &Camera, pos: Vec2| cam.world_to_screen(pos);
    let near_screen = to_screen(&near, near_pos);
    let far_screen = to_screen(&far, far_pos);
    assert!((near_screen.x - far_screen.x).abs() < 1e-3);
    assert!((near_screen.y - far_screen.y).abs() < 1e-3);
}

#[test]
fn the_interface_is_drawn_last_over_the_whole_window() {
    let mut renderer = RecordingRenderer::new();
    let world = world(MapSpec::new(8, 8).with_unit(
        sim::UnitKind::VILLAGER,
        PlayerId::ONE,
        sim::TilePos::new(3, 3),
    ));
    let camera = wide_camera();
    let previous: HashMap<UnitId, FxVec2> = HashMap::new();
    let sprites = common::sprites(&mut renderer);
    draw_world(
        &mut renderer,
        &camera,
        world.view(),
        &previous,
        0.0,
        &plain_hud(),
        &sprites,
    );

    let interface: Vec<(Vec2, Vec2)> = renderer
        .sprites()
        .filter(|(texture, _, _)| *texture == sprites.interface.texture)
        .map(|(_, pos, size)| (pos, size))
        .collect();
    assert_eq!(
        interface,
        [(
            camera.screen_to_world(Vec2::ZERO),
            camera.viewport * (1.0 / camera.zoom)
        )]
    );
    let (last, _, _) = renderer.sprites().last().expect("sprites drawn");
    assert_eq!(last, sprites.interface.texture);
}

#[test]
fn the_drag_box_goes_under_the_interface_as_in_the_original() {
    let mut renderer = RecordingRenderer::new();
    let world = world(MapSpec::new(8, 8));
    let camera = wide_camera();
    let previous: HashMap<UnitId, FxVec2> = HashMap::new();
    let sprites = common::sprites(&mut renderer);
    let hud = Hud {
        drag_box: Some((Vec2::new(1.0, 1.0), Vec2::new(7.0, 7.0))),
        ..plain_hud()
    };
    draw_world(
        &mut renderer,
        &camera,
        world.view(),
        &previous,
        0.0,
        &hud,
        &sprites,
    );

    let calls = renderer.calls();
    let last_line = calls
        .iter()
        .rposition(|c| matches!(c, DrawCall::Line { .. }))
        .expect("the drag box is drawn");
    let interface = calls
        .iter()
        .position(|c| matches!(c, DrawCall::Sprite { texture, .. } if *texture == sprites.interface.texture))
        .expect("the interface is drawn");
    assert!(last_line < interface);
}
