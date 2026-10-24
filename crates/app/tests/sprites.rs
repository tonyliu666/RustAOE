//! Drawing with the original game's sprites, and loading them.
//!
//! Draw tests use a sprite set made of fake textures loaded into the
//! recording renderer, so no game data is needed. Loading tests use a scratch
//! directory with missing or damaged archives.

mod common;

use app::{draw_world, load_images, unskew_tile, AssetError, Hud, Sprites, UNIT_PIXELS_PER_TILE};
use assets::Image;
use common::sprites;
use render::{Camera, RecordingRenderer, TextureId, Vec2};
use sim::{BuildingKind, FxVec2, MapSpec, PlayerId, TilePos, UnitId, UnitKind, World};
use std::collections::HashMap;

fn camera() -> Camera {
    Camera::new(Vec2::new(8.0, 8.0), 20.0, Vec2::new(800.0, 600.0))
}

fn hud() -> Hud<'static> {
    Hud {
        active_player: PlayerId::ONE,
        selection: &[],
        drag_box: None,
        replaying: false,
    }
}

fn draw(renderer: &mut RecordingRenderer, world: &World, sprites: &Sprites) {
    let previous: HashMap<UnitId, FxVec2> = HashMap::new();
    draw_world(
        renderer,
        &camera(),
        world.view(),
        &previous,
        0.0,
        &hud(),
        sprites,
    );
}

fn map() -> MapSpec {
    MapSpec::new(16, 16)
        .with_building(BuildingKind::TownCenter, PlayerId::ONE, TilePos::new(1, 1))
        .with_building(
            BuildingKind::TownCenter,
            PlayerId::TWO,
            TilePos::new(10, 10),
        )
}

fn close(a: Vec2, b: Vec2) -> bool {
    (a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4
}

/// The sprite calls drawn with `texture`.
fn drawn_with(renderer: &RecordingRenderer, texture: TextureId) -> Vec<(Vec2, Vec2)> {
    renderer
        .sprites()
        .filter(|(t, _, _)| *t == texture)
        .map(|(_, pos, size)| (pos, size))
        .collect()
}

// --- units ------------------------------------------------------------------

#[test]
fn a_unit_is_one_sprite_whose_hotspot_stands_on_its_position() {
    let mut renderer = RecordingRenderer::new();
    let set = sprites(&mut renderer);
    let world = World::new(
        1,
        map().with_unit(UnitKind::Villager, PlayerId::ONE, TilePos::new(6, 6)),
    );

    draw(&mut renderer, &world, &set);

    // The villager stands at the centre of tile (6, 6): (6.5, 6.5).
    let scale = 1.0 / UNIT_PIXELS_PER_TILE;
    let drawn = drawn_with(&renderer, set.villager[1].texture);
    assert_eq!(drawn.len(), 1, "drawn {drawn:?}");
    let (pos, size) = drawn[0];
    assert!(
        close(pos, Vec2::new(6.5 - 10.0 * scale, 6.5 - 40.0 * scale)),
        "at {pos:?}"
    );
    assert!(
        close(size, Vec2::new(21.0 * scale, 42.0 * scale)),
        "size {size:?}"
    );
}

#[test]
fn another_players_unit_is_drawn_in_that_players_colours() {
    let mut renderer = RecordingRenderer::new();
    let set = sprites(&mut renderer);
    let world = World::new(
        1,
        map().with_unit(UnitKind::Villager, PlayerId::TWO, TilePos::new(6, 6)),
    );

    draw(&mut renderer, &world, &set);

    assert_eq!(drawn_with(&renderer, set.villager[2].texture).len(), 1);
    assert!(drawn_with(&renderer, set.villager[1].texture).is_empty());
}

#[test]
fn a_soldier_is_drawn_with_the_soldier_sprite() {
    let mut renderer = RecordingRenderer::new();
    let set = sprites(&mut renderer);
    let world = World::new(
        1,
        map().with_unit(UnitKind::Soldier, PlayerId::ONE, TilePos::new(6, 6)),
    );

    draw(&mut renderer, &world, &set);

    assert_eq!(drawn_with(&renderer, set.soldier[1].texture).len(), 1);
}

// --- buildings --------------------------------------------------------------

#[test]
fn a_building_is_one_sprite_as_wide_as_its_footprint_centred_on_its_hotspot() {
    let mut renderer = RecordingRenderer::new();
    let set = sprites(&mut renderer);
    let world = World::new(1, map());

    draw(&mut renderer, &world, &set);

    // Player one's 4x4 town centre at (1, 1) has its centre at (3, 3). The
    // 200-pixel-wide sprite is scaled to 4 tiles: 0.02 tiles per pixel.
    let drawn = drawn_with(&renderer, set.town_center[1].texture);
    assert_eq!(drawn.len(), 1, "drawn {drawn:?}");
    let (pos, size) = drawn[0];
    assert!(
        close(pos, Vec2::new(3.0 - 100.0 * 0.02, 3.0 - 150.0 * 0.02)),
        "at {pos:?}"
    );
    assert!(close(size, Vec2::new(4.0, 240.0 * 0.02)), "size {size:?}");
}

// --- terrain ----------------------------------------------------------------

#[test]
fn open_ground_is_drawn_with_grass_one_tile_per_sprite() {
    let mut renderer = RecordingRenderer::new();
    let set = sprites(&mut renderer);
    let world = World::new(1, map());

    draw(&mut renderer, &world, &set);

    let grass_at_tile = renderer
        .sprites()
        .filter(|(t, _, _)| set.grass.contains(t))
        .any(|(_, pos, size)| pos == Vec2::new(7.0, 5.0) && size == Vec2::new(1.0, 1.0));
    assert!(grass_at_tile, "no grass sprite on tile (7, 5)");
}

#[test]
fn grass_varies_from_tile_to_tile() {
    let mut renderer = RecordingRenderer::new();
    let set = sprites(&mut renderer);
    let world = World::new(1, map());

    draw(&mut renderer, &world, &set);

    for texture in &set.grass {
        assert!(
            !drawn_with(&renderer, *texture).is_empty(),
            "grass {texture:?} never used"
        );
    }
}

// --- unskewing terrain ------------------------------------------------------

/// A 97x49 image whose every pixel records its own position as (x, y).
fn coordinate_image() -> Image {
    let (width, height) = (97u32, 49u32);
    let mut rgba = Vec::new();
    for y in 0..height {
        for x in 0..width {
            rgba.extend([x as u8, y as u8, 0, 255]);
        }
    }
    Image {
        width,
        height,
        rgba,
    }
}

/// The diamond pixel a square pixel was sampled from.
fn source_of(square: &Image, x: u32, y: u32) -> (i32, i32) {
    let [sx, sy, _, _] = square.pixel(x, y);
    (i32::from(sx), i32::from(sy))
}

fn near(a: (i32, i32), b: (i32, i32)) -> bool {
    (a.0 - b.0).abs() <= 3 && (a.1 - b.1).abs() <= 3
}

#[test]
fn unskewing_turns_the_diamond_tile_into_a_square() {
    let square = unskew_tile(&coordinate_image(), 32);

    assert_eq!((square.width, square.height), (32, 32));
    // Square corners come from the diamond's vertices: top-left from the top,
    // top-right from the right, bottom-left from the left, bottom-right from
    // the bottom. The middle comes from the middle.
    assert!(
        near(source_of(&square, 0, 0), (48, 0)),
        "{:?}",
        source_of(&square, 0, 0)
    );
    assert!(
        near(source_of(&square, 31, 0), (96, 24)),
        "{:?}",
        source_of(&square, 31, 0)
    );
    assert!(
        near(source_of(&square, 0, 31), (0, 24)),
        "{:?}",
        source_of(&square, 0, 31)
    );
    assert!(
        near(source_of(&square, 31, 31), (48, 48)),
        "{:?}",
        source_of(&square, 31, 31)
    );
    assert!(
        near(source_of(&square, 16, 16), (48, 24)),
        "{:?}",
        source_of(&square, 16, 16)
    );
}

// --- loading ----------------------------------------------------------------

fn scratch(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("aoe-sprites-test-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(path.join("data")).expect("scratch directory");
    path
}

#[test]
fn a_folder_without_the_game_files_is_an_error_naming_the_file() {
    let dir = scratch("missing");

    match load_images(&dir) {
        Err(AssetError::Io { path, .. }) => assert!(path.ends_with("interfac.drs"), "{path:?}"),
        other => panic!("expected a missing-file error, got {:?}", other.err()),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_damaged_archive_is_an_error_rather_than_a_crash() {
    let dir = scratch("damaged");
    std::fs::write(dir.join("data/interfac.drs"), b"not an archive at all").expect("write");

    match load_images(&dir) {
        Err(AssetError::Drs { path, .. }) => assert!(path.ends_with("interfac.drs"), "{path:?}"),
        other => panic!("expected a damaged-archive error, got {:?}", other.err()),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ground_under_a_building_is_grass_not_rock() {
    // A building blocks its tiles for pathing, but the ground under it is
    // still grass; only rock from the map gets trees.
    let mut renderer = RecordingRenderer::new();
    let set = sprites(&mut renderer);
    let world = World::new(1, map().with_mine(TilePos::new(12, 2), 100));

    draw(&mut renderer, &world, &set);

    // The map has no rock: the blocked tiles are all under the building and
    // the mine, so no tree may be drawn.
    assert!(drawn_with(&renderer, set.tree.texture).is_empty());
    for tile in [Vec2::new(2.0, 2.0), Vec2::new(13.0, 3.0)] {
        assert!(
            renderer
                .sprites()
                .any(|(t, pos, _)| set.grass.contains(&t) && pos == tile),
            "no grass under {tile:?}"
        );
    }
}

// --- no shapes --------------------------------------------------------------

#[test]
fn the_only_filled_rectangles_are_health_bars() {
    // Everything on the map is a sprite. Health bars are the one filled
    // rectangle left, as in the original.
    let map = map()
        .with_mine(TilePos::new(12, 2), 100)
        .with_blocked([TilePos::new(7, 7)])
        .with_unit(UnitKind::Villager, PlayerId::ONE, TilePos::new(6, 6))
        .with_unit(UnitKind::Soldier, PlayerId::TWO, TilePos::new(6, 7));
    let mut world = World::new(1, map);
    let (soldier, _) = world
        .view()
        .units()
        .find(|(_, u)| u.owner == PlayerId::TWO)
        .expect("soldier");
    let (villager, _) = world
        .view()
        .units()
        .find(|(_, u)| u.owner == PlayerId::ONE)
        .expect("villager");
    // Wound the villager so that a health bar is drawn.
    world.step(&[sim::Command::Attack {
        player: PlayerId::TWO,
        units: smallvec::smallvec![soldier],
        target: sim::EntityId::Unit(villager),
    }]);
    let mut renderer = RecordingRenderer::new();
    let set = sprites(&mut renderer);

    let previous: HashMap<UnitId, FxVec2> = HashMap::new();
    let hud = Hud {
        selection: std::slice::from_ref(&villager),
        ..hud()
    };
    draw_world(
        &mut renderer,
        &camera(),
        world.view(),
        &previous,
        0.0,
        &hud,
        &set,
    );

    let colours: Vec<_> = renderer.quads().map(|(_, _, c)| c).collect();
    assert!(
        !colours.is_empty(),
        "the wounded villager should have a health bar"
    );
    assert!(
        colours
            .iter()
            .all(|c| *c == app::HEALTH || *c == app::HEALTH_LOST),
        "a filled rectangle other than a health bar was drawn: {colours:?}"
    );
}

// --- trees and mines --------------------------------------------------------

#[test]
fn blocked_ground_is_grass_with_a_tree_standing_on_it() {
    let mut renderer = RecordingRenderer::new();
    let set = sprites(&mut renderer);
    let world = World::new(1, map().with_blocked([TilePos::new(7, 5)]));

    draw(&mut renderer, &world, &set);

    // The tree's ground point stands on the tile's centre, at the units' scale.
    let scale = 1.0 / UNIT_PIXELS_PER_TILE;
    let trees = drawn_with(&renderer, set.tree.texture);
    assert_eq!(trees.len(), 1, "drawn {trees:?}");
    assert!(
        close(
            trees[0].0,
            Vec2::new(7.5 - 55.0 * scale, 5.5 - 120.0 * scale)
        ),
        "at {:?}",
        trees[0].0
    );
    assert!(renderer
        .sprites()
        .any(|(t, pos, _)| set.grass.contains(&t) && pos == Vec2::new(7.0, 5.0)));
}

#[test]
fn a_gold_mine_is_its_sprite_standing_on_the_mines_centre() {
    let mut renderer = RecordingRenderer::new();
    let set = sprites(&mut renderer);
    // A 2x2 mine at (12, 2) has its centre at (13, 3).
    let world = World::new(1, map().with_mine(TilePos::new(12, 2), 100));

    draw(&mut renderer, &world, &set);

    let scale = 1.0 / UNIT_PIXELS_PER_TILE;
    let mines = drawn_with(&renderer, set.gold_mine.texture);
    assert_eq!(mines.len(), 1, "drawn {mines:?}");
    assert!(
        close(
            mines[0].0,
            Vec2::new(13.0 - 41.0 * scale, 3.0 - 22.0 * scale)
        ),
        "at {:?}",
        mines[0].0
    );
}

#[test]
fn a_third_player_and_gaia_get_their_own_colours() {
    let mut renderer = RecordingRenderer::new();
    let set = sprites(&mut renderer);
    let map = map()
        .with_players(3)
        .with_unit(UnitKind::Villager, PlayerId::new(3), TilePos::new(6, 6))
        .with_unit(UnitKind::Villager, PlayerId::GAIA, TilePos::new(9, 9));
    let world = World::new(1, map);

    draw(&mut renderer, &world, &set);

    assert_eq!(drawn_with(&renderer, set.villager[3].texture).len(), 1);
    assert_eq!(drawn_with(&renderer, set.villager[0].texture).len(), 1);
}

#[test]
fn unskewing_an_empty_tile_gives_a_transparent_square_rather_than_a_panic() {
    let empty = Image {
        width: 0,
        height: 0,
        rgba: Vec::new(),
    };

    let square = unskew_tile(&empty, 4);

    assert_eq!((square.width, square.height), (4, 4));
    assert!(square.rgba.iter().all(|&b| b == 0));
}
