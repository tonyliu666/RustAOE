//! Drawing units, buildings, mines, and the interface on top of them.
//!
//! Asserted against `RecordingRenderer` rather than by eye: this is the payoff
//! of `render` not knowing what a unit is.

use app::{draw_world, Hud, MINE, PLAYER_ONE, PLAYER_TWO, SELECTION};
use render::{Camera, Color, DrawCall, RecordingRenderer, Vec2};
use sim::{BuildingKind, Command, FxVec2, MapSpec, PlayerId, TilePos, UnitId, UnitKind, World};
use smallvec::smallvec;
use std::collections::HashMap;

fn camera() -> Camera {
    Camera::new(Vec2::new(8.0, 8.0), 20.0, Vec2::new(640.0, 640.0))
}

fn hud<'a>(selection: &'a [UnitId], drag_box: Option<(Vec2, Vec2)>) -> Hud<'a> {
    Hud {
        active_player: PlayerId::One,
        selection,
        drag_box,
        replaying: false,
    }
}

fn draw(renderer: &mut RecordingRenderer, world: &World, hud: &Hud<'_>) {
    let previous: HashMap<UnitId, FxVec2> = HashMap::new();
    draw_world(renderer, &camera(), world.view(), &previous, 0.0, hud);
}

/// A 16x16 map with one unit of each side, a building, and a mine.
fn populated() -> World {
    let map = MapSpec::new(16, 16)
        .with_starting_gold(200)
        .with_building(BuildingKind::TownCenter, PlayerId::One, TilePos::new(1, 1))
        .with_mine(TilePos::new(12, 12), 5000)
        .with_unit(UnitKind::Villager, PlayerId::One, TilePos::new(6, 6))
        .with_unit(UnitKind::Soldier, PlayerId::Two, TilePos::new(9, 9));
    World::new(7, map)
}

fn quads_of_color(renderer: &RecordingRenderer, color: Color) -> Vec<(Vec2, Vec2)> {
    renderer
        .quads()
        .filter(|(_, _, c)| *c == color)
        .map(|(pos, size, _)| (pos, size))
        .collect()
}

#[test]
fn each_side_is_drawn_in_its_own_colour() {
    let mut renderer = RecordingRenderer::new();
    let world = populated();

    draw(&mut renderer, &world, &hud(&[], None));

    // Player one has a town centre and a villager, player two has one soldier.
    assert_eq!(quads_of_color(&renderer, PLAYER_ONE).len(), 2);
    assert_eq!(quads_of_color(&renderer, PLAYER_TWO).len(), 1);
    assert_ne!(PLAYER_ONE, PLAYER_TWO);
}

#[test]
fn a_building_is_drawn_at_its_footprint_size() {
    let mut renderer = RecordingRenderer::new();
    let world = populated();

    draw(&mut renderer, &world, &hud(&[], None));

    let size = BuildingKind::TownCenter.size() as f32;
    assert!(
        quads_of_color(&renderer, PLAYER_ONE)
            .contains(&(Vec2::new(1.0, 1.0), Vec2::new(size, size))),
        "the town centre should cover its whole footprint"
    );
}

#[test]
fn a_mine_is_drawn_where_it_stands() {
    let mut renderer = RecordingRenderer::new();
    let world = populated();

    draw(&mut renderer, &world, &hud(&[], None));

    assert!(quads_of_color(&renderer, MINE).contains(&(Vec2::new(12.0, 12.0), Vec2::new(2.0, 2.0))));
}

#[test]
fn a_selected_unit_gets_an_outline_the_others_do_not() {
    let mut renderer = RecordingRenderer::new();
    let world = populated();
    let (villager, _) = world
        .view()
        .units()
        .find(|(_, u)| u.owner == PlayerId::One)
        .expect("the villager is on the map");

    draw(&mut renderer, &world, &hud(&[], None));
    assert!(quads_of_color(&renderer, SELECTION).is_empty());

    renderer.clear();
    draw(&mut renderer, &world, &hud(&[villager], None));
    assert_eq!(quads_of_color(&renderer, SELECTION).len(), 1);
}

#[test]
fn the_drag_rectangle_is_drawn_as_four_lines() {
    let mut renderer = RecordingRenderer::new();
    let world = populated();
    let corners = (Vec2::new(2.0, 2.0), Vec2::new(6.0, 5.0));

    draw(&mut renderer, &world, &hud(&[], Some(corners)));

    let lines: Vec<_> = renderer
        .calls()
        .iter()
        .filter(|call| matches!(call, DrawCall::Line { .. }))
        .collect();
    assert_eq!(lines.len(), 4);
}

#[test]
fn a_unit_at_full_health_has_no_health_bar() {
    let mut renderer = RecordingRenderer::new();
    let world = populated();

    draw(&mut renderer, &world, &hud(&[], None));

    assert!(quads_of_color(&renderer, app::HEALTH).is_empty());
}

#[test]
fn a_wounded_unit_gets_a_health_bar() {
    let map = MapSpec::new(16, 16)
        .with_building(BuildingKind::TownCenter, PlayerId::One, TilePos::new(1, 1))
        .with_building(BuildingKind::TownCenter, PlayerId::Two, TilePos::new(12, 1))
        .with_unit(UnitKind::Soldier, PlayerId::One, TilePos::new(6, 6))
        .with_unit(UnitKind::Villager, PlayerId::Two, TilePos::new(6, 7));
    let mut world = World::new(7, map);
    let attacker = world
        .view()
        .units()
        .find(|(_, u)| u.owner == PlayerId::One)
        .map(|(id, _)| id)
        .expect("the soldier is on the map");
    let victim = world
        .view()
        .units()
        .find(|(_, u)| u.owner == PlayerId::Two)
        .map(|(id, _)| id)
        .expect("the villager is on the map");

    world.step(&[Command::Attack {
        player: PlayerId::One,
        units: smallvec![attacker],
        target: sim::EntityId::Unit(victim),
    }]);

    let mut renderer = RecordingRenderer::new();
    draw(&mut renderer, &world, &hud(&[], None));

    assert_eq!(quads_of_color(&renderer, app::HEALTH).len(), 1);
}

#[test]
fn a_unit_is_drawn_between_its_last_two_positions() {
    let map = MapSpec::new(16, 16).with_unit(UnitKind::Villager, PlayerId::One, TilePos::new(4, 4));
    let world = World::new(7, map);
    let (id, unit) = world.view().units().next().expect("the villager");
    let now = app::to_screen_space(unit.pos);
    // Pretend the unit was two tiles to the left last tick.
    let before = FxVec2::new(unit.pos.x - sim::Fx::from_int(2), unit.pos.y);
    let previous: HashMap<UnitId, FxVec2> = [(id, before)].into_iter().collect();

    let mut renderer = RecordingRenderer::new();
    draw_world(
        &mut renderer,
        &camera(),
        world.view(),
        &previous,
        0.5,
        &hud(&[], None),
    );

    let body = quads_of_color(&renderer, PLAYER_ONE)
        .first()
        .copied()
        .expect("the villager should be drawn");
    // Halfway through the tick, halfway between the two positions.
    assert!(
        (body.0.x - (now.x - 1.0 - body.1.x / 2.0)).abs() < 1e-3,
        "drawn at {:?}, expected halfway back",
        body.0
    );
}

#[test]
fn the_overlay_names_the_active_player_and_their_gold() {
    let mut renderer = RecordingRenderer::new();
    let world = populated();

    draw(&mut renderer, &world, &hud(&[], None));

    let overlay: Vec<String> = renderer.texts().map(|(text, _)| text.to_string()).collect();
    assert!(
        overlay
            .iter()
            .any(|line| line.contains("player 1") && line.contains("gold 200")),
        "overlay was {overlay:?}"
    );
}

#[test]
fn the_overlay_announces_the_winner_once_the_game_is_decided() {
    let map = MapSpec::new(16, 16).with_building(
        BuildingKind::TownCenter,
        PlayerId::One,
        TilePos::new(1, 1),
    );
    let mut world = World::new(7, map);
    world.step(&[]);

    let mut renderer = RecordingRenderer::new();
    draw(&mut renderer, &world, &hud(&[], None));

    let overlay: Vec<String> = renderer.texts().map(|(text, _)| text.to_string()).collect();
    assert!(
        overlay.iter().any(|line| line.contains("player 1 wins")),
        "overlay was {overlay:?}"
    );
}
