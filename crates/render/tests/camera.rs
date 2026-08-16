use render::{Camera, Vec2};

/// A camera centred on the origin at 1:1 zoom puts world zero at the middle of
/// the window, not at its top-left corner.
#[test]
fn the_camera_centre_maps_to_the_middle_of_the_viewport() {
    let cam = Camera::new(Vec2::ZERO, 1.0, Vec2::new(800.0, 600.0));
    assert_eq!(cam.world_to_screen(Vec2::ZERO), Vec2::new(400.0, 300.0));
}

#[test]
fn zoom_scales_distance_from_the_camera_centre() {
    let cam = Camera::new(Vec2::new(10.0, 10.0), 2.0, Vec2::new(800.0, 600.0));
    assert_eq!(cam.world_to_screen(Vec2::new(11.0, 10.0)), Vec2::new(402.0, 300.0));
}

/// Screen to world is the inverse of world to screen, which is what makes a
/// mouse click resolvable to a tile.
#[test]
fn screen_and_world_conversions_are_inverses() {
    let cam = Camera::new(Vec2::new(-3.5, 12.0), 24.0, Vec2::new(1280.0, 720.0));
    let world = Vec2::new(7.25, -2.5);
    let round_tripped = cam.screen_to_world(cam.world_to_screen(world));
    assert!((round_tripped.x - world.x).abs() < 1e-4);
    assert!((round_tripped.y - world.y).abs() < 1e-4);
}
