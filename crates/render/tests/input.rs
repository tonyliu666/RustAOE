use render::{InputState, Key, MouseButton, Vec2};

#[test]
fn a_freshly_built_input_state_reports_nothing_held() {
    let input = InputState::default();
    assert!(!input.is_down(MouseButton::Left));
    assert!(!input.key_down(Key::Tab));
    assert!(!input.key_pressed(Key::Tab));
}

/// "Pressed" is the edge and "down" is the level. Selection needs the edge, and
/// camera panning needs the level, so the distinction cannot be collapsed.
#[test]
fn a_press_implies_the_button_is_also_down() {
    let mut input = InputState::default();
    input.press(MouseButton::Left);

    assert!(input.is_pressed(MouseButton::Left));
    assert!(input.is_down(MouseButton::Left));
    assert!(!input.is_released(MouseButton::Left));
}

#[test]
fn a_release_clears_the_down_state() {
    let mut input = InputState::default();
    input.press(MouseButton::Left);
    input.release(MouseButton::Left);

    assert!(!input.is_down(MouseButton::Left));
    assert!(input.is_released(MouseButton::Left));
}

#[test]
fn keys_track_edges_and_levels_separately() {
    let mut input = InputState::default();
    input.press_key(Key::Tab);
    input.hold_key(Key::W);

    assert!(input.key_pressed(Key::Tab));
    assert!(input.key_down(Key::Tab));
    assert!(input.key_down(Key::W));
    assert!(!input.key_pressed(Key::W));
}

#[test]
fn cursor_and_scroll_are_carried_verbatim() {
    let mut input = InputState::default();
    input.cursor = Vec2::new(120.0, 45.0);
    input.scroll = -3.0;

    assert_eq!(input.cursor, Vec2::new(120.0, 45.0));
    assert_eq!(input.scroll, -3.0);
}
