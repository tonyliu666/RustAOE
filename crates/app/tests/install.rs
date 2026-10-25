//! Loading the game's sprites from an install, end to end, with a fake
//! install written by the test.

mod common;

use app::{load_images, AssetError};
use common::{colour, graphics, install, install_with, remove, slp, TWO_PIXELS};
use render::RecordingRenderer;
use sim::{BuildingKind, Content, PlayerId, UnitKind};

#[test]
fn each_player_slot_holds_that_players_colours() {
    let dir = install("colours", graphics(&[]), slp(2, 2, &TWO_PIXELS));
    let images = load_images(&dir, &Content::default()).expect("a complete install loads");
    let mut renderer = RecordingRenderer::new();

    let sprites = images.upload(&mut renderer);

    for player in 0..9u8 {
        let pixels = renderer
            .texture_pixels(
                sprites
                    .unit(UnitKind::VILLAGER, PlayerId::new(player))
                    .texture,
            )
            .expect("uploaded");
        // The first pixel is player colour value 3, shifted 16 per player;
        // the second is plain colour 200 for everyone.
        assert_eq!(&pixels[0..4], colour(3 + 16 * player), "player {player}");
        assert_eq!(&pixels[4..8], colour(200), "player {player}");
    }
    remove(&dir);
}

#[test]
fn neutral_objects_are_drawn_in_gaias_colours() {
    let dir = install("neutral", graphics(&[]), slp(1, 2, &TWO_PIXELS));
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &Content::default())
        .expect("loads")
        .upload(&mut renderer);

    let tree = renderer
        .texture_pixels(sprites.tree.texture)
        .expect("uploaded");
    assert_eq!(&tree[0..4], colour(3));
    remove(&dir);
}

#[test]
fn grass_with_no_frames_is_an_error_rather_than_a_crash_while_drawing() {
    let dir = install("no-grass", graphics(&[]), slp(0, 2, &TWO_PIXELS));

    let result = load_images(&dir, &Content::default());

    assert!(
        matches!(result, Err(AssetError::EmptySprite { id: 15001 })),
        "{:?}",
        result.err()
    );
    remove(&dir);
}

#[test]
fn an_empty_grass_frame_is_an_error_rather_than_a_crash() {
    let dir = install("empty-grass", graphics(&[]), slp(1, 0, &[0x0F]));

    let result = load_images(&dir, &Content::default());

    assert!(
        matches!(result, Err(AssetError::EmptySprite { id: 15001 })),
        "{:?}",
        result.err()
    );
    remove(&dir);
}

#[test]
fn a_unit_sprite_with_no_width_is_an_error_rather_than_an_infinite_scale() {
    let dir = install(
        "empty-unit",
        graphics(&[(1473, slp(1, 0, &[0x0F]))]),
        slp(1, 2, &TWO_PIXELS),
    );

    let result = load_images(&dir, &Content::default());

    assert!(
        matches!(result, Err(AssetError::EmptySprite { id: 1473 })),
        "{:?}",
        result.err()
    );
    remove(&dir);
}

/// The default content plus an archer drawn with sprite 7777.
fn with_archer() -> (Content, UnitKind) {
    let mut content = Content::default();
    let mut archer = content.unit(UnitKind::SOLDIER).clone();
    archer.name = "archer".to_string();
    archer.sprite = 7777;
    content.units.push(archer);
    let kind = content.unit_named("archer").expect("added");
    content
        .building_mut(BuildingKind::BARRACKS)
        .trains
        .push(kind);
    (content, kind)
}

#[test]
fn a_unit_type_added_in_content_is_drawn_with_the_sprite_its_entry_names() {
    let (content, archer) = with_archer();
    let mut files = graphics(&[]);
    // Three pixels wide, so it cannot be mistaken for the two-pixel default.
    files.push((common::SLP, 7777, slp(1, 3, &[3 << 2, 9, 9, 9, 0x0F])));
    let dir = install("archer", files, slp(1, 2, &TWO_PIXELS));
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &content)
        .expect("loads")
        .upload(&mut renderer);

    let sprite = sprites.unit(archer, PlayerId::ONE);
    assert_eq!(renderer.texture_size(sprite.texture), Some((3, 1)));
    remove(&dir);
}

#[test]
fn a_sprite_the_install_does_not_have_is_an_error_naming_the_entry() {
    let (content, _) = with_archer();
    let dir = install("no-archer", graphics(&[]), slp(1, 2, &TWO_PIXELS));

    let result = load_images(&dir, &content);

    assert!(
        matches!(&result, Err(AssetError::MissingGraphic { entry, id: 7777 }) if entry == "archer"),
        "{:?}",
        result.err()
    );
    remove(&dir);
}

// --- archive order ---------------------------------------------------------------

/// A three-pixel sprite, told apart from the two-pixel default by its width.
fn three_pixels() -> Vec<u8> {
    slp(1, 3, &[3 << 2, 9, 9, 9, 0x0F])
}

#[test]
fn a_sprite_in_an_expansion_archive_overrides_the_base_games_copy() {
    // The original opens the expansion's archives first and takes the first
    // match (FUN_0043ae90, FUN_00543630): gamedata_x1_p1.drs before
    // graphics.drs.
    let dir = install_with(
        "override",
        graphics(&[]),
        slp(1, 2, &TWO_PIXELS),
        &[(
            "gamedata_x1_p1.drs",
            vec![(common::SLP, 1473, three_pixels())],
        )],
    );
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &Content::default())
        .expect("loads")
        .upload(&mut renderer);

    let villager = sprites.unit(UnitKind::VILLAGER, PlayerId::ONE);
    assert_eq!(renderer.texture_size(villager.texture), Some((3, 1)));
    remove(&dir);
}

#[test]
fn a_sound_with_a_sprites_id_does_not_hide_the_sprite() {
    // Archives are searched by type and ID: sounds_x1.drs comes first and
    // holds a sound numbered like the villager's sprite.
    let dir = install_with(
        "sound-id",
        graphics(&[]),
        slp(1, 2, &TWO_PIXELS),
        &[(
            "sounds_x1.drs",
            vec![(common::WAV, 1473, b"RIFF....".to_vec())],
        )],
    );
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &Content::default())
        .expect("loads")
        .upload(&mut renderer);

    let villager = sprites.unit(UnitKind::VILLAGER, PlayerId::ONE);
    assert_eq!(renderer.texture_size(villager.texture), Some((2, 1)));
    remove(&dir);
}

#[test]
fn the_interface_image_is_loaded() {
    // The real one is in interfac.drs; this fake install keeps it in
    // graphics.drs, and the search finds it in whichever archive holds it.
    let dir = install(
        "interface",
        graphics(&[(51101, three_pixels())]),
        slp(1, 2, &TWO_PIXELS),
    );
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &Content::default())
        .expect("loads")
        .upload(&mut renderer);

    assert_eq!(
        renderer.texture_size(sprites.interface.texture),
        Some((3, 1))
    );
    remove(&dir);
}

#[test]
fn an_install_without_the_interface_image_is_an_error() {
    let files = graphics(&[])
        .into_iter()
        .filter(|(_, id, _)| *id != 51101)
        .collect();
    let dir = install("no-interface", files, slp(1, 2, &TWO_PIXELS));

    let result = load_images(&dir, &Content::default());

    assert!(
        matches!(result, Err(AssetError::Missing { id: 51101, .. })),
        "{:?}",
        result.err()
    );
    remove(&dir);
}

#[test]
fn each_unit_types_icon_is_its_frame_of_the_unit_icons() {
    // Frame n of the fake icon sprite is n + 1 pixels wide, so the size
    // tells which frame was taken: the villager's is 15, the soldier's 8.
    let dir = install(
        "icons",
        graphics(&[(50730, common::icons(16))]),
        slp(1, 2, &TWO_PIXELS),
    );
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &Content::default())
        .expect("loads")
        .upload(&mut renderer);

    let width = |kind| {
        renderer
            .texture_size(sprites.unit_icon(kind).texture)
            .map(|(w, _)| w)
    };
    assert_eq!(width(UnitKind::VILLAGER), Some(16));
    assert_eq!(width(UnitKind::SOLDIER), Some(9));
    // Taken from the unit icons, not the building icons (colour 7).
    let pixels = renderer
        .texture_pixels(sprites.unit_icon(UnitKind::VILLAGER).texture)
        .expect("uploaded");
    assert_eq!(&pixels[0..4], colour(9));
    remove(&dir);
}

#[test]
fn an_icon_frame_the_install_does_not_have_is_an_error_naming_the_entry() {
    let mut content = Content::default();
    content.unit_mut(UnitKind::SOLDIER).icon = 99;
    let dir = install("icon-missing", graphics(&[]), slp(1, 2, &TWO_PIXELS));

    let result = load_images(&dir, &content);

    assert!(
        matches!(&result, Err(AssetError::MissingIcon { entry, frame: 99 }) if entry == "soldier"),
        "{:?}",
        result.err()
    );
    remove(&dir);
}

#[test]
fn each_building_types_icon_is_its_frame_of_the_building_icons() {
    // The town centre's is frame 28, the barracks' frame 2.
    let dir = install(
        "building-icons",
        graphics(&[(50705, common::icons_in(30, 7))]),
        slp(1, 2, &TWO_PIXELS),
    );
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &Content::default())
        .expect("loads")
        .upload(&mut renderer);

    let width = |kind| {
        renderer
            .texture_size(sprites.building_icon(kind).texture)
            .map(|(w, _)| w)
    };
    assert_eq!(width(BuildingKind::TOWN_CENTER), Some(29));
    assert_eq!(width(BuildingKind::BARRACKS), Some(3));
    remove(&dir);
}

#[test]
fn a_building_icon_the_install_does_not_have_is_an_error_naming_the_entry() {
    let mut content = Content::default();
    content.building_mut(BuildingKind::BARRACKS).icon = 99;
    let dir = install(
        "building-icon-missing",
        graphics(&[]),
        slp(1, 2, &TWO_PIXELS),
    );

    let result = load_images(&dir, &content);

    assert!(
        matches!(&result, Err(AssetError::MissingIcon { entry, frame: 99 }) if entry == "barracks"),
        "{:?}",
        result.err()
    );
    remove(&dir);
}

#[test]
fn every_frame_of_the_progress_bar_is_loaded_in_order() {
    let dir = install("progress", graphics(&[]), slp(1, 2, &TWO_PIXELS));
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &Content::default())
        .expect("loads")
        .upload(&mut renderer);

    assert_eq!(sprites.progress.len(), 100);
    // Frame n of the fake bar is n + 1 pixels wide.
    for (n, frame) in sprites.progress.iter().enumerate() {
        assert_eq!(
            renderer.texture_size(frame.texture),
            Some((n as u32 + 1, 1))
        );
    }
    remove(&dir);
}
