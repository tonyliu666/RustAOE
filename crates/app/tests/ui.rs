//! Where the original's interface puts its panels, for any window size.

use app::{Layout, Rect};
use render::Vec2;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(Vec2::new(x, y), Vec2::new(w, h))
}

fn close(a: Rect, b: Rect) -> bool {
    let near = |p: f32, q: f32| (p - q).abs() < 1e-3;
    near(a.min.x, b.min.x)
        && near(a.min.y, b.min.y)
        && near(a.size.x, b.size.x)
        && near(a.size.y, b.size.y)
}

#[test]
fn at_800_by_600_the_panels_sit_where_the_originals_image_draws_them() {
    let layout = Layout::new(Vec2::new(800.0, 600.0));

    assert_eq!(layout.top_bar, rect(0.0, 0.0, 800.0, 25.0));
    assert_eq!(layout.bottom, rect(0.0, 455.0, 800.0, 145.0));
    assert_eq!(layout.command_panel, rect(4.0, 461.0, 206.0, 127.0));
    assert_eq!(layout.info_panel, rect(219.0, 467.0, 280.0, 128.0));
    assert_eq!(layout.minimap, rect(507.0, 461.0, 290.0, 136.0));
}

#[test]
fn the_command_panel_holds_three_rows_of_five_icon_sized_buttons() {
    let layout = Layout::new(Vec2::new(800.0, 600.0));

    assert_eq!(layout.buttons.len(), 15);
    assert_eq!(layout.buttons[0], rect(7.0, 465.0, 36.0, 36.0));
    assert_eq!(layout.buttons[4], rect(171.0, 465.0, 36.0, 36.0));
    assert_eq!(layout.buttons[5], rect(7.0, 506.0, 36.0, 36.0));
    assert_eq!(layout.buttons[14], rect(171.0, 547.0, 36.0, 36.0));
    for button in layout.buttons {
        assert!(layout.command_panel.contains_rect(button), "{button:?}");
    }
}

#[test]
fn the_progress_bar_is_the_originals_bar_size_inside_the_information_panel() {
    let layout = Layout::new(Vec2::new(800.0, 600.0));

    assert_eq!(layout.progress_bar.size, Vec2::new(100.0, 12.0));
    assert!(layout.info_panel.contains_rect(layout.progress_bar));
}

#[test]
fn a_bigger_window_stretches_every_rectangle_with_the_image() {
    let small = Layout::new(Vec2::new(800.0, 600.0));
    let big = Layout::new(Vec2::new(1600.0, 900.0));
    let stretch = |r: Rect| rect(r.min.x * 2.0, r.min.y * 1.5, r.size.x * 2.0, r.size.y * 1.5);

    assert!(close(big.top_bar, stretch(small.top_bar)));
    assert!(close(big.command_panel, stretch(small.command_panel)));
    assert!(close(big.info_panel, stretch(small.info_panel)));
    assert!(close(big.minimap, stretch(small.minimap)));
    assert!(close(big.progress_bar, stretch(small.progress_bar)));
    assert!(close(big.buttons[14], stretch(small.buttons[14])));
}

#[test]
fn the_bars_block_the_map_and_the_open_middle_does_not() {
    let layout = Layout::new(Vec2::new(800.0, 600.0));

    assert!(layout.blocks(Vec2::new(400.0, 10.0)), "top bar");
    assert!(layout.blocks(Vec2::new(100.0, 500.0)), "command panel");
    assert!(layout.blocks(Vec2::new(650.0, 599.0)), "minimap corner");
    assert!(
        layout.blocks(Vec2::new(212.0, 456.0)),
        "frame between panels"
    );
    assert!(!layout.blocks(Vec2::new(400.0, 300.0)), "middle");
    assert!(
        !layout.blocks(Vec2::new(400.0, 25.0)),
        "just below the top bar"
    );
    assert!(
        !layout.blocks(Vec2::new(400.0, 454.9)),
        "just above the panels"
    );
}

#[test]
fn a_point_outside_the_window_is_not_on_the_interface() {
    let layout = Layout::new(Vec2::new(800.0, 600.0));

    assert!(!layout.blocks(Vec2::new(-5.0, 500.0)));
    assert!(!layout.blocks(Vec2::new(400.0, 650.0)));
}

#[test]
fn the_information_panel_has_room_for_the_building_and_what_it_trains() {
    let layout = Layout::new(Vec2::new(800.0, 600.0));
    let info = layout.info_panel;

    assert_eq!(layout.building_icon.size, Vec2::new(36.0, 36.0));
    assert_eq!(layout.training_icon.size, Vec2::new(36.0, 36.0));
    let mut parts = vec![
        layout.building_icon,
        layout.training_icon,
        layout.progress_bar,
    ];
    parts.extend(layout.queue_icons);
    for part in &parts {
        assert!(info.contains_rect(*part), "{part:?}");
    }
    for point in [layout.name_at, layout.hit_points_at] {
        assert!(info.contains(point), "{point:?}");
    }
    // Nothing overlaps: each part is drawn on its own patch of parchment.
    for (i, a) in parts.iter().enumerate() {
        for b in &parts[i + 1..] {
            let apart = a.max().x <= b.min.x
                || b.max().x <= a.min.x
                || a.max().y <= b.min.y
                || b.max().y <= a.min.y;
            assert!(apart, "{a:?} overlaps {b:?}");
        }
    }
}

#[test]
fn there_is_a_queue_slot_for_every_unit_waiting_behind_the_one_in_training() {
    let layout = Layout::new(Vec2::new(800.0, 600.0));

    assert_eq!(layout.queue_icons.len(), sim::Building::QUEUE_LIMIT - 1);
}

#[test]
fn the_panel_text_grows_with_the_window() {
    let small = Layout::new(Vec2::new(800.0, 600.0));
    let big = Layout::new(Vec2::new(1600.0, 1200.0));

    assert!((big.text_px - 2.0 * small.text_px).abs() < 1e-3);
    assert!(close(
        Rect::new(big.name_at, Vec2::ZERO),
        Rect::new(small.name_at * 2.0, Vec2::ZERO)
    ));
}

#[test]
fn each_resource_is_written_in_its_slot_of_the_top_bar() {
    // The slots' dark text areas, after each icon, on the 800x600 image:
    // wood, food, gold, stone, left to right, from y 4 to 21.
    let layout = Layout::new(Vec2::new(800.0, 600.0));
    for (resource, from, to) in [
        (sim::Resource::Wood, 28.0, 77.0),
        (sim::Resource::Food, 104.0, 154.0),
        (sim::Resource::Gold, 182.0, 231.0),
        (sim::Resource::Stone, 256.0, 308.0),
    ] {
        let at = layout.resource_at(resource);
        assert!((from..to - 20.0).contains(&at.x), "{resource:?} at {at:?}");
        assert!(
            (4.0 + layout.bar_text_px * 0.7..=21.0).contains(&at.y),
            "{resource:?} at {at:?}"
        );
    }
}

#[test]
fn a_units_load_is_written_on_the_information_panel_below_its_hit_points() {
    let layout = Layout::new(Vec2::new(800.0, 600.0));

    assert!(layout.info_panel.contains(layout.carrying_at));
    assert!(layout.carrying_at.y > layout.hit_points_at.y);
}
