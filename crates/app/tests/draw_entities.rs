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
        building: None,
        drag_box,
        replaying: false,
        cursor_world: Vec2::ZERO,
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
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(1, 1))
        .with_mine(TilePos::new(12, 12), 5000)
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(6, 6))
        .with_unit(UnitKind::SOLDIER, PlayerId::TWO, TilePos::new(9, 9));
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
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(1, 1))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(12, 1),
        )
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(6, 6))
        .with_unit(UnitKind::VILLAGER, PlayerId::TWO, TilePos::new(6, 7));
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
    // A melee soldier walks up to touch the villager first.
    let full = world.view().content().unit(UnitKind::VILLAGER).max_hp;
    for _ in 0..100 {
        if world.view().unit(victim).is_some_and(|u| u.hp < full) {
            break;
        }
        world.step(&[]);
    }

    let mut renderer = RecordingRenderer::new();
    draw(&mut renderer, &world, &hud(&[], None));

    assert_eq!(quads_of_color(&renderer, app::HEALTH).len(), 1);
}

#[test]
fn a_unit_is_drawn_between_its_last_two_positions() {
    let map = MapSpec::new(16, 16).with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(4, 4));
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

    let villager = sprites.unit(UnitKind::VILLAGER, PlayerId::new(1));
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
fn the_overlay_names_a_player_beyond_the_second() {
    let mut renderer = RecordingRenderer::new();
    let map = MapSpec::new(16, 16).with_players(3).with_building(
        BuildingKind::TOWN_CENTER,
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
        BuildingKind::TOWN_CENTER,
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

// --- a selected building -------------------------------------------------------

/// Where the town centre's sprite was drawn: (top-left, size).
fn town_centre_drawn(renderer: &RecordingRenderer, sprites: &Sprites) -> (Vec2, Vec2) {
    let texture = sprites
        .building(BuildingKind::TOWN_CENTER, PlayerId::ONE)
        .texture;
    renderer
        .sprites()
        .find(|(t, _, _)| *t == texture)
        .map(|(_, pos, size)| (pos, size))
        .expect("the town centre is drawn")
}

#[test]
fn a_selected_building_gets_an_outline_around_its_sprite() {
    let mut renderer = RecordingRenderer::new();
    let world = populated();
    let (town_centre, _) = world.view().buildings().next().expect("a building");
    let hud = Hud {
        building: Some(town_centre),
        ..hud(&[], None)
    };

    let sprites = draw(&mut renderer, &world, &hud);

    let (pos, size) = town_centre_drawn(&renderer, &sprites);
    let ends: Vec<Vec2> = renderer
        .calls()
        .iter()
        .filter_map(|call| match call {
            DrawCall::Line { a, .. } => Some(*a),
            _ => None,
        })
        .collect();
    assert_eq!(
        ends,
        [
            pos,
            Vec2::new(pos.x + size.x, pos.y),
            pos + size,
            Vec2::new(pos.x, pos.y + size.y),
        ]
    );
}

#[test]
fn a_buildings_click_target_is_where_its_sprite_is_drawn() {
    let mut renderer = RecordingRenderer::new();
    let world = populated();
    let (_, building) = world.view().buildings().next().expect("a building");

    let sprites = draw(&mut renderer, &world, &hud(&[], None));

    let (pos, size) = town_centre_drawn(&renderer, &sprites);
    let body = sprites
        .bodies(world.view().content())
        .building(BuildingKind::TOWN_CENTER)
        .expect("a body for each building type");
    let half = building.size as f32 / 2.0;
    let centre = Vec2::new(
        building.origin.x as f32 + half,
        building.origin.y as f32 + half,
    );
    assert_eq!((centre + body.offset, body.size), (pos, size));
}

// --- the command panel ------------------------------------------------------------

/// The sprites drawn with `texture`, as (top-left, size).
fn drawn_with(renderer: &RecordingRenderer, texture: render::TextureId) -> Vec<(Vec2, Vec2)> {
    renderer
        .sprites()
        .filter(|(t, _, _)| *t == texture)
        .map(|(_, pos, size)| (pos, size))
        .collect()
}

/// A screen rectangle in world units, as the panel's sprites are drawn.
fn in_world(rect: app::Rect) -> (Vec2, Vec2) {
    let camera = camera();
    (
        camera.screen_to_world(rect.min),
        rect.size * (1.0 / camera.zoom),
    )
}

/// A map with a barracks for each player.
fn two_barracks() -> World {
    let map = MapSpec::new(16, 16)
        .with_building(BuildingKind::BARRACKS, PlayerId::ONE, TilePos::new(1, 1))
        .with_building(BuildingKind::BARRACKS, PlayerId::TWO, TilePos::new(10, 1));
    World::new(7, map)
}

fn barracks_of(world: &World, owner: PlayerId) -> sim::BuildingId {
    world
        .view()
        .buildings()
        .find(|(_, b)| b.owner == owner)
        .map(|(id, _)| id)
        .expect("a barracks")
}

#[test]
fn your_selected_barracks_shows_the_soldiers_icon_on_the_first_button() {
    let mut renderer = RecordingRenderer::new();
    let world = two_barracks();
    let hud = Hud {
        building: Some(barracks_of(&world, PlayerId::ONE)),
        ..hud(&[], None)
    };

    let sprites = draw(&mut renderer, &world, &hud);

    let buttons = app::Layout::new(camera().viewport).buttons;
    let soldier = sprites.unit_icon(UnitKind::SOLDIER).texture;
    assert_eq!(drawn_with(&renderer, soldier), [in_world(buttons[0])]);
    let villager = sprites.unit_icon(UnitKind::VILLAGER).texture;
    assert_eq!(drawn_with(&renderer, villager), []);
}

#[test]
fn the_buttons_are_drawn_over_the_interface() {
    let mut renderer = RecordingRenderer::new();
    let world = two_barracks();
    let hud = Hud {
        building: Some(barracks_of(&world, PlayerId::ONE)),
        ..hud(&[], None)
    };

    let sprites = draw(&mut renderer, &world, &hud);

    let order: Vec<render::TextureId> = renderer.sprites().map(|(t, _, _)| t).collect();
    let interface = order.iter().position(|t| *t == sprites.interface.texture);
    let button = order
        .iter()
        .position(|t| *t == sprites.unit_icon(UnitKind::SOLDIER).texture);
    assert!(interface < button, "{interface:?} {button:?}");
}

#[test]
fn another_players_building_shows_no_buttons() {
    let mut renderer = RecordingRenderer::new();
    let world = two_barracks();
    let hud = Hud {
        building: Some(barracks_of(&world, PlayerId::TWO)),
        ..hud(&[], None)
    };

    let sprites = draw(&mut renderer, &world, &hud);

    let soldier = sprites.unit_icon(UnitKind::SOLDIER).texture;
    assert_eq!(drawn_with(&renderer, soldier), []);
}

#[test]
fn with_no_building_selected_there_are_no_buttons() {
    let mut renderer = RecordingRenderer::new();
    let world = two_barracks();

    let sprites = draw(&mut renderer, &world, &hud(&[], None));

    let soldier = sprites.unit_icon(UnitKind::SOLDIER).texture;
    assert_eq!(drawn_with(&renderer, soldier), []);
}

// --- the information panel --------------------------------------------------------

fn texts(renderer: &RecordingRenderer) -> Vec<(String, Vec2)> {
    renderer
        .texts()
        .map(|(text, pos)| (text.to_string(), pos))
        .collect()
}

/// Selects player one's barracks in `world` and draws a frame.
fn draw_with_barracks_selected(renderer: &mut RecordingRenderer, world: &World) -> Sprites {
    let hud = Hud {
        building: Some(barracks_of(world, PlayerId::ONE)),
        ..hud(&[], None)
    };
    draw(renderer, world, &hud)
}

/// [`two_barracks`] with player one training `count` soldiers, stepped
/// until the first has `remaining` ticks left.
fn training(count: usize, remaining: u32) -> World {
    let map = MapSpec::new(16, 16)
        .with_starting_resources(Resources {
            food: 1000,
            gold: 1000,
            ..Resources::default()
        })
        .with_building(BuildingKind::BARRACKS, PlayerId::ONE, TilePos::new(1, 1))
        .with_building(BuildingKind::BARRACKS, PlayerId::TWO, TilePos::new(10, 1));
    let mut world = World::new(7, map);
    let barracks = barracks_of(&world, PlayerId::ONE);
    let orders: Vec<Command> = (0..count)
        .map(|_| Command::Train {
            player: PlayerId::ONE,
            building: barracks,
            unit: UnitKind::SOLDIER,
        })
        .collect();
    world.step(&orders);
    while world
        .view()
        .building(barracks)
        .expect("standing")
        .train_ticks
        > remaining
    {
        world.step(&[]);
    }
    world
}

#[test]
fn the_information_panel_shows_the_selected_buildings_icon_name_and_hit_points() {
    let mut renderer = RecordingRenderer::new();
    let world = two_barracks();

    let sprites = draw_with_barracks_selected(&mut renderer, &world);

    let layout = app::Layout::new(camera().viewport);
    let icon = sprites.building_icon(BuildingKind::BARRACKS).texture;
    assert_eq!(
        drawn_with(&renderer, icon),
        [in_world(layout.building_icon)]
    );
    let camera = camera();
    let texts = texts(&renderer);
    let max_hp = world
        .view()
        .content()
        .building(BuildingKind::BARRACKS)
        .max_hp;
    assert!(
        texts.contains(&(
            "barracks".to_string(),
            camera.screen_to_world(layout.name_at)
        )),
        "{texts:?}"
    );
    assert!(
        texts.contains(&(
            format!("{max_hp}/{max_hp}"),
            camera.screen_to_world(layout.hit_points_at)
        )),
        "{texts:?}"
    );
}

#[test]
fn with_nothing_training_there_is_no_progress_bar() {
    let mut renderer = RecordingRenderer::new();
    let world = two_barracks();

    let sprites = draw_with_barracks_selected(&mut renderer, &world);

    for frame in &sprites.progress {
        assert_eq!(drawn_with(&renderer, frame.texture), []);
    }
}

#[test]
fn the_progress_bar_shows_how_much_of_the_unit_is_trained() {
    // With 70 percent of the soldier's training time left it is 30 percent
    // done, and frame n of the original's bar is n percent full.
    let total = sim::Content::default().unit(UnitKind::SOLDIER).train_ticks;
    assert_eq!(total % 10, 0, "a time that splits into tenths");
    let mut renderer = RecordingRenderer::new();
    let world = training(1, total / 10 * 7);

    let sprites = draw_with_barracks_selected(&mut renderer, &world);

    let layout = app::Layout::new(camera().viewport);
    let drawn: Vec<usize> = sprites
        .progress
        .iter()
        .enumerate()
        .filter(|(_, s)| !drawn_with(&renderer, s.texture).is_empty())
        .map(|(frame, _)| frame)
        .collect();
    assert_eq!(drawn, [30]);
    assert_eq!(
        drawn_with(&renderer, sprites.progress[30].texture),
        [in_world(layout.progress_bar)]
    );
}

#[test]
fn the_unit_in_training_and_the_ones_queued_behind_it_show_their_icons() {
    let mut renderer = RecordingRenderer::new();
    let world = training(3, 50);

    let sprites = draw_with_barracks_selected(&mut renderer, &world);

    let layout = app::Layout::new(camera().viewport);
    let soldier = sprites.unit_icon(UnitKind::SOLDIER).texture;
    let mut expected = vec![
        in_world(layout.buttons[0]),
        in_world(layout.training_icon),
        in_world(layout.queue_icons[0]),
        in_world(layout.queue_icons[1]),
    ];
    let mut drawn = drawn_with(&renderer, soldier);
    let key = |r: &(Vec2, Vec2)| (r.0.x.to_bits(), r.0.y.to_bits());
    expected.sort_by_key(key);
    drawn.sort_by_key(key);
    assert_eq!(drawn, expected);
}

#[test]
fn another_players_building_shows_its_name_but_not_what_it_trains() {
    let mut renderer = RecordingRenderer::new();
    let world = training(2, 50);
    // Player one's barracks is training; player two looks at it.
    let hud = Hud {
        active_player: PlayerId::TWO,
        building: Some(barracks_of(&world, PlayerId::ONE)),
        ..hud(&[], None)
    };

    let sprites = draw(&mut renderer, &world, &hud);

    assert!(texts(&renderer).iter().any(|(t, _)| t == "barracks"));
    let soldier = sprites.unit_icon(UnitKind::SOLDIER).texture;
    assert_eq!(drawn_with(&renderer, soldier), []);
    for frame in &sprites.progress {
        assert_eq!(drawn_with(&renderer, frame.texture), []);
    }
}

#[test]
fn with_nothing_selected_the_information_panel_is_empty() {
    let mut renderer = RecordingRenderer::new();
    let world = two_barracks();

    let sprites = draw(&mut renderer, &world, &hud(&[], None));

    let icon = sprites.building_icon(BuildingKind::BARRACKS).texture;
    assert_eq!(drawn_with(&renderer, icon), []);
    assert!(!texts(&renderer).iter().any(|(t, _)| t == "barracks"));
}

// --- the top bar and the debug line -----------------------------------------------

/// The text drawn at screen point `at`, if any.
fn text_at(renderer: &RecordingRenderer, at: Vec2) -> Option<String> {
    let world_at = camera().screen_to_world(at);
    renderer
        .texts()
        .find(|(_, pos)| (pos.x - world_at.x).abs() < 1e-4 && (pos.y - world_at.y).abs() < 1e-4)
        .map(|(text, _)| text.to_string())
}

fn stocked() -> World {
    let map = MapSpec::new(16, 16)
        .with_players(2)
        .with_starting_resources(Resources {
            food: 11,
            wood: 22,
            gold: 33,
            stone: 44,
        })
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(1, 1))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(10, 1),
        );
    World::new(7, map)
}

#[test]
fn the_top_bar_shows_each_resource_in_its_own_slot() {
    let mut renderer = RecordingRenderer::new();
    let world = stocked();

    draw(&mut renderer, &world, &hud(&[], None));

    let layout = app::Layout::new(camera().viewport);
    for (resource, amount) in [
        (sim::Resource::Food, "11"),
        (sim::Resource::Wood, "22"),
        (sim::Resource::Gold, "33"),
        (sim::Resource::Stone, "44"),
    ] {
        assert_eq!(
            text_at(&renderer, layout.resource_at(resource)).as_deref(),
            Some(amount),
            "{resource:?}"
        );
    }
}

#[test]
fn the_top_bar_shows_the_resources_of_whoever_holds_the_keyboard() {
    let map = MapSpec::new(16, 16)
        .with_starting_resources(Resources {
            food: 200,
            ..Resources::default()
        })
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(1, 1))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(10, 1),
        );
    let mut world = World::new(7, map);
    let (town_centre, _) = world
        .view()
        .buildings()
        .find(|(_, b)| b.owner == PlayerId::ONE)
        .expect("a town centre");
    // Player one spends 50 food on a villager; player two keeps 200.
    world.step(&[Command::Train {
        player: PlayerId::ONE,
        building: town_centre,
        unit: UnitKind::VILLAGER,
    }]);
    let layout = app::Layout::new(camera().viewport);
    let food = layout.resource_at(sim::Resource::Food);
    let mut renderer = RecordingRenderer::new();

    draw(&mut renderer, &world, &hud(&[], None));
    let ones = text_at(&renderer, food);
    renderer.clear();
    let twos_turn = Hud {
        active_player: PlayerId::TWO,
        ..hud(&[], None)
    };
    draw(&mut renderer, &world, &twos_turn);
    let twos = text_at(&renderer, food);

    assert_eq!(ones.as_deref(), Some("150"));
    assert_eq!(twos.as_deref(), Some("200"));
}

#[test]
fn the_debug_readout_is_one_line_just_below_the_top_bar() {
    let mut renderer = RecordingRenderer::new();
    let world = stocked();

    draw(&mut renderer, &world, &hud(&[], None));

    let camera = camera();
    let layout = app::Layout::new(camera.viewport);
    let lines: Vec<(String, Vec2)> = renderer
        .texts()
        .filter(|(text, _)| text.contains("tick"))
        .map(|(text, pos)| (text.to_string(), camera.world_to_screen(pos)))
        .collect();
    assert_eq!(lines.len(), 1, "{lines:?}");
    let (line, at) = &lines[0];
    assert!(line.starts_with("player 1  tick 0"), "{line}");
    assert!(at.y > layout.top_bar.max().y, "{at:?} is on the top bar");
    assert!(
        at.y < layout.top_bar.max().y + 30.0,
        "{at:?} is far below it"
    );
    // The old lines are gone: resources live in the top bar, controls in
    // QUICKSTART.
    for (text, _) in renderer.texts() {
        assert!(
            !text.contains("selected") && !text.contains("tab switch"),
            "{text}"
        );
    }
}

// --- drawn back to front ------------------------------------------------------

#[test]
fn buildings_are_drawn_back_to_front_so_the_nearer_roof_covers_the_farther() {
    // The barracks is placed first, so it has the lower ID, but it stands
    // south of the town centre, nearer the viewer: it must be drawn after,
    // as picking assumes.
    let map = MapSpec::new(16, 16)
        .with_building(BuildingKind::BARRACKS, PlayerId::ONE, TilePos::new(2, 8))
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(2, 2));
    let world = World::new(7, map);
    let mut renderer = RecordingRenderer::new();

    let sprites = draw(&mut renderer, &world, &hud(&[], None));

    let order: Vec<render::TextureId> = renderer.sprites().map(|(t, _, _)| t).collect();
    let at = |texture| order.iter().position(|t| *t == texture).expect("drawn");
    let town_centre = sprites
        .building(BuildingKind::TOWN_CENTER, PlayerId::ONE)
        .texture;
    let barracks = sprites
        .building(BuildingKind::BARRACKS, PlayerId::ONE)
        .texture;
    assert!(at(town_centre) < at(barracks));
}

#[test]
fn units_are_drawn_back_to_front() {
    // The soldier is placed first but stands further south.
    let map = MapSpec::new(16, 16)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(1, 1))
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(8, 9))
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(8, 7));
    let world = World::new(7, map);
    let mut renderer = RecordingRenderer::new();

    let sprites = draw(&mut renderer, &world, &hud(&[], None));

    let order: Vec<render::TextureId> = renderer.sprites().map(|(t, _, _)| t).collect();
    let at = |texture| order.iter().position(|t| *t == texture).expect("drawn");
    let villager = sprites.unit(UnitKind::VILLAGER, PlayerId::ONE).texture;
    let soldier = sprites.unit(UnitKind::SOLDIER, PlayerId::ONE).texture;
    assert!(at(villager) < at(soldier));
}

#[test]
fn a_sprite_set_without_a_progress_bar_draws_the_rest_rather_than_panicking() {
    let mut renderer = RecordingRenderer::new();
    let world = training(1, 50);
    let mut sprites = common::sprites(&mut renderer);
    sprites.progress.clear();
    let hud = Hud {
        building: Some(barracks_of(&world, PlayerId::ONE)),
        ..hud(&[], None)
    };

    draw_world(
        &mut renderer,
        &camera(),
        world.view(),
        &HashMap::new(),
        0.0,
        &hud,
        &sprites,
    );

    let soldier = sprites.unit_icon(UnitKind::SOLDIER).texture;
    assert!(!drawn_with(&renderer, soldier).is_empty());
}

// --- the information panel for a unit ---------------------------------------------

/// A map with player one's villager beside a mine, for gathering.
fn villager_by_a_mine() -> (World, UnitId) {
    let map = MapSpec::new(16, 16)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(1, 1))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(12, 12),
        )
        .with_mine(TilePos::new(7, 2), 500)
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(6, 2));
    let world = World::new(7, map);
    let (villager, _) = world
        .view()
        .units()
        .find(|(_, u)| u.owner == PlayerId::ONE)
        .expect("the villager");
    (world, villager)
}

#[test]
fn a_selected_unit_shows_its_icon_name_and_hit_points() {
    let mut renderer = RecordingRenderer::new();
    let (world, villager) = villager_by_a_mine();
    let selection = [villager];

    let sprites = draw(&mut renderer, &world, &hud(&selection, None));

    let layout = app::Layout::new(camera().viewport);
    let icon = sprites.unit_icon(UnitKind::VILLAGER).texture;
    assert!(
        drawn_with(&renderer, icon).contains(&in_world(layout.building_icon)),
        "no icon on the information panel"
    );
    let camera = camera();
    let max_hp = world.view().content().unit(UnitKind::VILLAGER).max_hp;
    let texts = texts(&renderer);
    assert!(
        texts.contains(&(
            "villager".to_string(),
            camera.screen_to_world(layout.name_at)
        )),
        "{texts:?}"
    );
    assert!(
        texts.contains(&(
            format!("{max_hp}/{max_hp}"),
            camera.screen_to_world(layout.hit_points_at)
        )),
        "{texts:?}"
    );
}

#[test]
fn a_villager_carrying_gold_shows_its_load() {
    let (mut world, villager) = villager_by_a_mine();
    let (mine, _) = world.view().sites().next().expect("the mine");
    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: smallvec![villager],
        site: mine,
    }]);
    for _ in 0..400 {
        if world.view().unit(villager).is_some_and(|u| u.carrying > 0) {
            break;
        }
        world.step(&[]);
    }
    let carrying = world.view().unit(villager).expect("the villager").carrying;
    assert!(carrying > 0, "never mined anything");
    let mut renderer = RecordingRenderer::new();
    let selection = [villager];

    draw(&mut renderer, &world, &hud(&selection, None));

    let layout = app::Layout::new(camera().viewport);
    assert!(
        texts(&renderer).contains(&(
            format!("carrying {carrying} gold"),
            camera().screen_to_world(layout.carrying_at)
        )),
        "{:?}",
        texts(&renderer)
    );
}

#[test]
fn a_villager_carrying_nothing_shows_no_load() {
    let mut renderer = RecordingRenderer::new();
    let (world, villager) = villager_by_a_mine();
    let selection = [villager];

    draw(&mut renderer, &world, &hud(&selection, None));

    assert!(
        !texts(&renderer)
            .iter()
            .any(|(t, _)| t.starts_with("carrying")),
        "{:?}",
        texts(&renderer)
    );
}

#[test]
fn with_several_units_selected_the_first_is_shown() {
    let map = MapSpec::new(16, 16)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(1, 1))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(12, 12),
        )
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(6, 6))
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(7, 6));
    let world = World::new(7, map);
    let ids: Vec<UnitId> = world.view().units().map(|(id, _)| id).collect();
    let first = world.view().unit(ids[0]).expect("first").kind;
    let mut renderer = RecordingRenderer::new();

    draw(&mut renderer, &world, &hud(&ids, None));

    let name = world.view().content().unit(first).name.clone();
    let layout = app::Layout::new(camera().viewport);
    let at = camera().screen_to_world(layout.name_at);
    let names: Vec<String> = texts(&renderer)
        .into_iter()
        .filter(|(_, pos)| *pos == at)
        .map(|(t, _)| t)
        .collect();
    assert_eq!(names, [name]);
}

#[test]
fn a_villager_carrying_stone_says_stone() {
    let map = MapSpec::new(16, 16)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(1, 1))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(12, 12),
        )
        .with_site(sim::Resource::Stone, TilePos::new(7, 2), 300)
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(6, 2));
    let mut world = World::new(7, map);
    let (villager, _) = world.view().units().next().expect("the villager");
    let (site, _) = world.view().sites().next().expect("the stone");
    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: smallvec![villager],
        site,
    }]);
    for _ in 0..400 {
        if world.view().unit(villager).is_some_and(|u| u.carrying > 0) {
            break;
        }
        world.step(&[]);
    }
    let carrying = world.view().unit(villager).expect("the villager").carrying;
    let mut renderer = RecordingRenderer::new();
    let selection = [villager];

    draw(&mut renderer, &world, &hud(&selection, None));

    assert!(
        texts(&renderer)
            .iter()
            .any(|(t, _)| *t == format!("carrying {carrying} stone")),
        "{:?}",
        texts(&renderer)
    );
}
