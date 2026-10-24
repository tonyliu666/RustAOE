//! Drawing units, buildings, mines, and the interface on top of them.
//!
//! Asserted against `RecordingRenderer` rather than by eye: this is the payoff
//! of `render` not knowing what a unit is.

mod common;

use app::{draw_world, Hud, Sprites, UNIT_PIXELS_PER_TILE};
use render::{Camera, Color, DrawCall, RecordingRenderer, Vec2};
use sim::{
    BuildingKind, Command, FxVec2, MapSpec, PlayerId, Resources, TilePos, UnitId, UnitKind, World,
};
use smallvec::smallvec;
use std::collections::HashMap;

fn camera() -> Camera {
    Camera::new(Vec2::new(8.0, 8.0), 20.0, Vec2::new(640.0, 640.0))
}

fn hud<'a>(selection: &'a [UnitId], drag_box: Option<(Vec2, Vec2)>) -> Hud<'a> {
    Hud {
        active_player: PlayerId::ONE,
        selection,
        drag_box,
        replaying: false,
    }
}

/// Draws a frame with a fresh fake sprite set, returning the set so a test
/// can tell which texture is which.
fn draw(renderer: &mut RecordingRenderer, world: &World, hud: &Hud<'_>) -> Sprites {
    let previous: HashMap<UnitId, FxVec2> = HashMap::new();
    let sprites = common::sprites(renderer);
    draw_world(
        renderer,
        &camera(),
        world.view(),
        &previous,
        0.0,
        hud,
        &sprites,
    );
    sprites
}

fn lines(renderer: &RecordingRenderer) -> usize {
    renderer
        .calls()
        .iter()
        .filter(|call| matches!(call, DrawCall::Line { .. }))
        .count()
}

/// A 16x16 map with one unit of each side, a building, and a mine.
fn populated() -> World {
    let map = MapSpec::new(16, 16)
        .with_starting_gold(200)
        .with_building(BuildingKind::TownCenter, PlayerId::ONE, TilePos::new(1, 1))
        .with_mine(TilePos::new(12, 12), 5000)
        .with_unit(UnitKind::Villager, PlayerId::ONE, TilePos::new(6, 6))
        .with_unit(UnitKind::Soldier, PlayerId::TWO, TilePos::new(9, 9));
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
fn a_selected_unit_gets_an_outline_the_others_do_not() {
    let mut renderer = RecordingRenderer::new();
    let world = populated();
    let (villager, _) = world
        .view()
        .units()
        .find(|(_, u)| u.owner == PlayerId::ONE)
        .expect("the villager is on the map");

    draw(&mut renderer, &world, &hud(&[], None));
    assert_eq!(lines(&renderer), 0);

    // A thin outline, as in the original: four lines, no filled rectangle.
    renderer.clear();
    draw(&mut renderer, &world, &hud(&[villager], None));
    assert_eq!(lines(&renderer), 4);
}

#[test]
fn the_drag_rectangle_is_drawn_as_four_lines() {
    let mut renderer = RecordingRenderer::new();
    let world = populated();
    let corners = (Vec2::new(2.0, 2.0), Vec2::new(6.0, 5.0));

    draw(&mut renderer, &world, &hud(&[], Some(corners)));

    assert_eq!(lines(&renderer), 4);
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
        .with_building(BuildingKind::TownCenter, PlayerId::ONE, TilePos::new(1, 1))
        .with_building(BuildingKind::TownCenter, PlayerId::TWO, TilePos::new(12, 1))
        .with_unit(UnitKind::Soldier, PlayerId::ONE, TilePos::new(6, 6))
        .with_unit(UnitKind::Villager, PlayerId::TWO, TilePos::new(6, 7));
    let mut world = World::new(7, map);
    let attacker = world
        .view()
        .units()
        .find(|(_, u)| u.owner == PlayerId::ONE)
        .map(|(id, _)| id)
        .expect("the soldier is on the map");
    let victim = world
        .view()
        .units()
        .find(|(_, u)| u.owner == PlayerId::TWO)
        .map(|(id, _)| id)
        .expect("the villager is on the map");

    world.step(&[Command::Attack {
        player: PlayerId::ONE,
        units: smallvec![attacker],
        target: sim::EntityId::Unit(victim),
    }]);

    let mut renderer = RecordingRenderer::new();
    draw(&mut renderer, &world, &hud(&[], None));

    assert_eq!(quads_of_color(&renderer, app::HEALTH).len(), 1);
}

#[test]
fn a_unit_is_drawn_between_its_last_two_positions() {
    let map = MapSpec::new(16, 16).with_unit(UnitKind::Villager, PlayerId::ONE, TilePos::new(4, 4));
    let world = World::new(7, map);
    let (id, unit) = world.view().units().next().expect("the villager");
    let now = app::to_screen_space(unit.pos);
    // Pretend the unit was two tiles to the left last tick.
    let before = FxVec2::new(unit.pos.x - sim::Fx::from_int(2), unit.pos.y);
    let previous: HashMap<UnitId, FxVec2> = [(id, before)].into_iter().collect();

    let mut renderer = RecordingRenderer::new();
    let sprites = common::sprites(&mut renderer);
    draw_world(
        &mut renderer,
        &camera(),
        world.view(),
        &previous,
        0.5,
        &hud(&[], None),
        &sprites,
    );

    let villager = sprites.villager[1];
    let (_, pos, _) = renderer
        .sprites()
        .find(|(t, _, _)| *t == villager.texture)
        .expect("the villager should be drawn");
    // Halfway through the tick, halfway between the two positions.
    let hotspot_x = villager.hotspot.x / UNIT_PIXELS_PER_TILE;
    assert!(
        (pos.x - (now.x - 1.0 - hotspot_x)).abs() < 1e-3,
        "drawn at {pos:?}, expected halfway back"
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
fn the_overlay_shows_every_resource_of_the_active_player() {
    let mut renderer = RecordingRenderer::new();
    let map = MapSpec::new(16, 16)
        .with_starting_resources(Resources {
            food: 11,
            wood: 22,
            gold: 33,
            stone: 44,
        })
        .with_building(BuildingKind::TownCenter, PlayerId::ONE, TilePos::new(1, 1));
    let world = World::new(7, map);

    draw(&mut renderer, &world, &hud(&[], None));

    let overlay: Vec<String> = renderer.texts().map(|(text, _)| text.to_string()).collect();
    assert!(
        overlay.iter().any(|line| line.contains("food 11")
            && line.contains("wood 22")
            && line.contains("gold 33")
            && line.contains("stone 44")),
        "overlay was {overlay:?}"
    );
}

#[test]
fn the_overlay_names_a_player_beyond_the_second() {
    let mut renderer = RecordingRenderer::new();
    let map = MapSpec::new(16, 16).with_players(3).with_building(
        BuildingKind::TownCenter,
        PlayerId::new(3),
        TilePos::new(1, 1),
    );
    let world = World::new(7, map);
    let mut hud = hud(&[], None);
    hud.active_player = PlayerId::new(3);

    draw(&mut renderer, &world, &hud);

    let overlay: Vec<String> = renderer.texts().map(|(text, _)| text.to_string()).collect();
    assert!(
        overlay.iter().any(|line| line.starts_with("player 3 ")),
        "overlay was {overlay:?}"
    );
}

#[test]
fn the_overlay_announces_the_winner_once_the_game_is_decided() {
    let map = MapSpec::new(16, 16).with_building(
        BuildingKind::TownCenter,
        PlayerId::ONE,
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
