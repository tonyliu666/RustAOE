//! Camera panning and zooming, driven by synthesised input.

use app::CameraController;
use render::{Camera, InputState, Key, Vec2};

const MAP: i32 = 128;

fn controller() -> CameraController {
    let camera = Camera::new(Vec2::new(64.0, 64.0), 16.0, Vec2::new(800.0, 600.0));
    CameraController::new(camera, MAP, MAP)
}

fn holding(keys: &[Key]) -> InputState {
    let mut input = InputState::default();
    for key in keys {
        input.hold_key(*key);
    }
    input
}

fn scrolling(amount: f32) -> InputState {
    let mut input = InputState::default();
    input.scroll = amount;
    input
}

#[test]
fn no_input_leaves_the_camera_where_it_was() {
    let mut cam = controller();
    let before = cam.camera();

    cam.update(&holding(&[]), 0.1);

    assert_eq!(before, cam.camera());
}

#[test]
fn w_and_s_pan_the_camera_up_and_down() {
    let mut up = controller();
    let mut down = controller();
    let start = up.camera().center.y;

    up.update(&holding(&[Key::W]), 0.1);
    down.update(&holding(&[Key::S]), 0.1);

    assert!(up.camera().center.y < start, "W should pan towards lower y");
    assert!(
        down.camera().center.y > start,
        "S should pan towards higher y"
    );
}

#[test]
fn a_and_d_pan_the_camera_left_and_right() {
    let mut left = controller();
    let mut right = controller();
    let start = left.camera().center.x;

    left.update(&holding(&[Key::A]), 0.1);
    right.update(&holding(&[Key::D]), 0.1);

    assert!(left.camera().center.x < start);
    assert!(right.camera().center.x > start);
}

#[test]
fn opposing_keys_cancel_out() {
    let mut cam = controller();
    let before = cam.camera();

    cam.update(&holding(&[Key::W, Key::S, Key::A, Key::D]), 0.1);

    assert_eq!(before, cam.camera());
}

#[test]
fn panning_twice_as_long_pans_twice_as_far() {
    let mut short = controller();
    let mut long = controller();
    let start = short.camera().center.x;

    short.update(&holding(&[Key::D]), 0.1);
    long.update(&holding(&[Key::D]), 0.2);

    let short_delta = short.camera().center.x - start;
    let long_delta = long.camera().center.x - start;
    assert!((long_delta - short_delta * 2.0).abs() < 1e-4);
}

#[test]
fn a_long_frame_cannot_teleport_the_camera_across_the_map() {
    let mut cam = controller();
    let start = cam.camera().center.x;

    // A stalled frame — a breakpoint, a disk hitch — must not translate into a
    // jump of hundreds of tiles.
    cam.update(&holding(&[Key::D]), 30.0);

    let delta = cam.camera().center.x - start;
    assert!(delta > 0.0);
    assert!(delta <= CameraController::PAN_TILES_PER_SECOND * 0.25);
}

#[test]
fn panning_stops_at_the_map_edge() {
    let mut cam = controller();

    for _ in 0..1000 {
        cam.update(&holding(&[Key::D, Key::S]), 0.1);
    }

    assert_eq!(cam.camera().center.x, MAP as f32);
    assert_eq!(cam.camera().center.y, MAP as f32);
}

#[test]
fn panning_stops_at_the_map_origin() {
    let mut cam = controller();

    for _ in 0..1000 {
        cam.update(&holding(&[Key::A, Key::W]), 0.1);
    }

    assert_eq!(cam.camera().center.x, 0.0);
    assert_eq!(cam.camera().center.y, 0.0);
}

#[test]
fn scrolling_up_zooms_in_and_scrolling_down_zooms_out() {
    let mut cam = controller();
    let start = cam.camera().zoom;

    cam.update(&scrolling(1.0), 0.016);
    let zoomed_in = cam.camera().zoom;
    cam.update(&scrolling(-1.0), 0.016);

    assert!(zoomed_in > start);
    assert!(
        (cam.camera().zoom - start).abs() < 1e-3,
        "zoom should return"
    );
}

#[test]
fn q_and_e_zoom_as_well_as_the_wheel() {
    let mut out = controller();
    let mut in_ = controller();
    let start = out.camera().zoom;

    out.update(&holding(&[Key::Q]), 0.1);
    in_.update(&holding(&[Key::E]), 0.1);

    assert!(out.camera().zoom < start);
    assert!(in_.camera().zoom > start);
}

#[test]
fn zoom_is_clamped_to_a_usable_range() {
    let mut close = controller();
    let mut far = controller();

    for _ in 0..500 {
        close.update(&scrolling(1.0), 0.016);
        far.update(&scrolling(-1.0), 0.016);
    }

    assert_eq!(close.camera().zoom, CameraController::MAX_ZOOM);
    assert_eq!(far.camera().zoom, CameraController::MIN_ZOOM);
}

#[test]
fn the_viewport_follows_the_window_size() {
    let mut cam = controller();

    cam.resize(Vec2::new(1280.0, 720.0));

    assert_eq!(cam.camera().viewport, Vec2::new(1280.0, 720.0));
}
