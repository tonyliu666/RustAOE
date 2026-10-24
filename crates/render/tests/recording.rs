use render::{Camera, Color, DrawCall, RecordingRenderer, Renderer, Vec2};

fn camera() -> Camera {
    Camera::new(Vec2::ZERO, 8.0, Vec2::new(640.0, 480.0))
}

#[test]
fn draw_calls_are_recorded_in_order() {
    let mut recorder = RecordingRenderer::new();

    recorder.begin_frame(&camera());
    recorder.quad(Vec2::ZERO, Vec2::new(1.0, 1.0), Color::WHITE);
    recorder.line(Vec2::ZERO, Vec2::new(2.0, 0.0), Color::WHITE);
    recorder.text("gold: 200", Vec2::new(4.0, 4.0), 16.0, Color::WHITE);
    recorder.end_frame();

    assert_eq!(
        recorder.calls(),
        &[
            DrawCall::BeginFrame(camera()),
            DrawCall::Quad {
                pos: Vec2::ZERO,
                size: Vec2::new(1.0, 1.0),
                color: Color::WHITE,
            },
            DrawCall::Line {
                a: Vec2::ZERO,
                b: Vec2::new(2.0, 0.0),
                color: Color::WHITE,
            },
            DrawCall::Text {
                text: "gold: 200".to_string(),
                pos: Vec2::new(4.0, 4.0),
                px: 16.0,
                color: Color::WHITE,
            },
            DrawCall::EndFrame,
        ]
    );
}

/// Assertions in application tests are usually about one kind of primitive, so
/// the fake offers a filtered view rather than making every test walk the log.
#[test]
fn quads_can_be_inspected_without_walking_every_call() {
    let mut recorder = RecordingRenderer::new();

    recorder.begin_frame(&camera());
    recorder.quad(Vec2::ZERO, Vec2::new(1.0, 1.0), Color::WHITE);
    recorder.text("ignored", Vec2::ZERO, 12.0, Color::WHITE);
    recorder.quad(Vec2::new(3.0, 0.0), Vec2::new(1.0, 1.0), Color::BLACK);
    recorder.end_frame();

    let quads: Vec<_> = recorder.quads().collect();
    assert_eq!(quads.len(), 2);
    assert_eq!(quads[1].0, Vec2::new(3.0, 0.0));
    assert_eq!(quads[1].2, Color::BLACK);
}

/// A test that draws several frames needs to assert on the latest one, so the
/// recorder counts frames and can be reset between them.
#[test]
fn frames_are_counted_and_the_log_can_be_cleared() {
    let mut recorder = RecordingRenderer::new();

    for _ in 0..3 {
        recorder.begin_frame(&camera());
        recorder.end_frame();
    }
    assert_eq!(recorder.frame_count(), 3);

    recorder.clear();
    assert_eq!(recorder.frame_count(), 0);
    assert!(recorder.calls().is_empty());
}

// --- textures and sprites ---------------------------------------------------

#[test]
fn each_loaded_texture_gets_its_own_id_and_keeps_its_size() {
    let mut recorder = RecordingRenderer::new();

    let grass = recorder.load_texture(97, 49, &[0; 97 * 49 * 4]);
    let villager = recorder.load_texture(21, 42, &[0; 21 * 42 * 4]);

    assert_ne!(grass, villager);
    assert_eq!(recorder.texture_size(grass), Some((97, 49)));
    assert_eq!(recorder.texture_size(villager), Some((21, 42)));
}

#[test]
fn a_sprite_is_recorded_with_its_texture_position_and_size() {
    let mut recorder = RecordingRenderer::new();
    let texture = recorder.load_texture(2, 2, &[0; 16]);

    recorder.begin_frame(&camera());
    recorder.sprite(texture, Vec2::new(3.0, 4.0), Vec2::new(1.0, 2.0));
    recorder.end_frame();

    assert_eq!(
        recorder.sprites().collect::<Vec<_>>(),
        [(texture, Vec2::new(3.0, 4.0), Vec2::new(1.0, 2.0))]
    );
}

#[test]
fn a_texture_that_was_never_loaded_has_no_size() {
    let mut other = RecordingRenderer::new();
    let foreign = other.load_texture(1, 1, &[0; 4]);
    let recorder = RecordingRenderer::new();

    assert_eq!(recorder.texture_size(foreign), None);
}

#[test]
fn a_loaded_texture_keeps_its_pixels_for_inspection() {
    let mut recorder = RecordingRenderer::new();
    let texture = recorder.load_texture(1, 1, &[10, 20, 30, 255]);

    assert_eq!(
        recorder.texture_pixels(texture),
        Some([10, 20, 30, 255].as_slice())
    );
}
