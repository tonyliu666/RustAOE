//! Camera panning and zooming, driven by synthesised input.

use app::CameraController;
use render::{Camera, InputState, Key, MouseButton, Vec2};

const MAP: i32 = 128;

fn controller() -> CameraController {
    let camera = Camera::new(Vec2::new(64.0, 64.0), 16.0, Vec2::new(800.0, 600.0));
    CameraController::new(camera, MAP, MAP)
}

/// The middle of the 800x600 viewport, well away from every edge.
const MIDDLE: Vec2 = Vec2::new(400.0, 300.0);

/// Input with the cursor resting mid-screen, so that only what the test
/// adds can move the camera.
fn idle() -> InputState {
    let mut input = InputState::default();
    input.cursor = MIDDLE;
    input
}

fn holding(keys: &[Key]) -> InputState {
    let mut input = idle();
    for key in keys {
        input.hold_key(*key);
    }
    input
}

fn scrolling(amount: f32) -> InputState {
    let mut input = idle();
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

// --- edge scrolling -------------------------------------------------------

/// Input with the cursor at `cursor`, in screen pixels.
fn cursor_at(x: f32, y: f32) -> InputState {
    let mut input = InputState::default();
    input.cursor = Vec2::new(x, y);
    input.cursor_seen = true;
    input
}

/// How far the camera centre moves in one 0.1 s frame of `input`.
fn pan(input: &InputState) -> Vec2 {
    let mut cam = controller();
    let before = cam.camera().center;
    cam.update(input, 0.1);
    cam.camera().center - before
}

/// One frame of panning at full speed, in tiles.
const STEP: f32 = CameraController::PAN_TILES_PER_SECOND * 0.1;

fn close(a: Vec2, b: Vec2) -> bool {
    (a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4
}

#[test]
fn the_cursor_at_each_edge_pans_that_way() {
    for (cursor, expected) in [
        (cursor_at(0.0, 300.0), Vec2::new(-STEP, 0.0)),
        (cursor_at(799.0, 300.0), Vec2::new(STEP, 0.0)),
        (cursor_at(400.0, 0.0), Vec2::new(0.0, -STEP)),
        (cursor_at(400.0, 599.0), Vec2::new(0.0, STEP)),
    ] {
        let moved = pan(&cursor);
        assert!(
            close(moved, expected),
            "cursor {:?} panned {moved:?}",
            cursor.cursor
        );
    }
}

#[test]
fn the_cursor_a_few_pixels_from_an_edge_still_pans() {
    let moved = pan(&cursor_at(
        CameraController::EDGE_SCROLL_PIXELS - 1.0,
        300.0,
    ));

    assert!(close(moved, Vec2::new(-STEP, 0.0)), "panned {moved:?}");
}

#[test]
fn the_cursor_in_the_middle_does_not_pan() {
    assert_eq!(pan(&idle()), Vec2::ZERO);
}

#[test]
fn the_cursor_in_a_corner_pans_diagonally_at_normal_speed() {
    let moved = pan(&cursor_at(0.0, 0.0));

    assert!(moved.x < 0.0 && moved.y < 0.0, "panned {moved:?}");
    let length = (moved.x * moved.x + moved.y * moved.y).sqrt();
    assert!((length - STEP).abs() < 1e-4, "panned {moved:?}");
}

#[test]
fn a_cursor_outside_the_window_does_not_pan() {
    // In a window, the mouse leaving it means the player is using another
    // program, not asking to scroll.
    for (x, y) in [
        (-40.0, 300.0),
        (840.0, 300.0),
        (400.0, -10.0),
        (400.0, 650.0),
    ] {
        assert_eq!(
            pan(&cursor_at(x, y)),
            Vec2::ZERO,
            "cursor at ({x}, {y}) panned"
        );
    }
}

#[test]
fn an_edge_and_a_key_together_are_no_faster_than_the_key() {
    let mut both = cursor_at(0.0, 300.0);
    both.hold_key(Key::A);

    assert!(
        close(pan(&both), Vec2::new(-STEP, 0.0)),
        "panned {:?}",
        pan(&both)
    );
}

#[test]
fn dragging_a_selection_to_the_edge_does_not_pan() {
    let mut dragging = cursor_at(0.0, 300.0);
    dragging.hold(MouseButton::Left);

    assert_eq!(pan(&dragging), Vec2::ZERO);
}

#[test]
fn the_edges_do_nothing_until_the_cursor_has_been_seen() {
    // Before the mouse first moves over the window the backend reports the
    // cursor at (0, 0), a corner, which would drift the camera on its own.
    let mut unseen = cursor_at(0.0, 0.0);
    unseen.cursor_seen = false;

    assert_eq!(pan(&unseen), Vec2::ZERO);
}

#[test]
fn the_upper_part_of_the_left_edge_pans_left_and_up() {
    let moved = pan(&cursor_at(0.0, 100.0));

    assert!(moved.x < 0.0 && moved.y < 0.0, "panned {moved:?}");
}

#[test]
fn the_left_part_of_the_top_edge_pans_up_and_left() {
    let moved = pan(&cursor_at(150.0, 0.0));

    assert!(moved.x < 0.0 && moved.y < 0.0, "panned {moved:?}");
}

#[test]
fn an_edge_pans_along_the_line_from_the_screen_centre_to_the_cursor() {
    // From the centre (400, 300) to (0, 100) is (-400, -200): twice as far
    // across as up, so the pan should be too, at full speed.
    let moved = pan(&cursor_at(0.0, 100.0));

    assert!((moved.x / moved.y - 2.0).abs() < 1e-3, "panned {moved:?}");
    let length = (moved.x * moved.x + moved.y * moved.y).sqrt();
    assert!((length - STEP).abs() < 1e-4, "panned {moved:?}");
}

#[test]
fn a_cursor_at_the_centre_of_a_tiny_window_does_not_break_the_camera() {
    // A window under 16 pixels across, as while it is being minimised, puts
    // its centre inside the edge band, where the direction would be 0 / 0.
    let camera = Camera::new(Vec2::new(64.0, 64.0), 16.0, Vec2::new(10.0, 10.0));
    let mut cam = CameraController::new(camera, MAP, MAP);

    cam.update(&cursor_at(5.0, 5.0), 0.1);

    assert_eq!(cam.camera().center, Vec2::new(64.0, 64.0));
}
