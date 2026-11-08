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
    let images =
        load_images(&dir, &Content::default(), &looks()).expect("a complete install loads");
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

    let sprites = load_images(&dir, &Content::default(), &looks())
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

    let result = load_images(&dir, &Content::default(), &looks());

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

    let result = load_images(&dir, &Content::default(), &looks());

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
        graphics(&[(1479, slp(1, 0, &[0x0F]))]),
        slp(1, 2, &TWO_PIXELS),
    );

    let result = load_images(&dir, &Content::default(), &looks());

    assert!(
        matches!(result, Err(AssetError::EmptySprite { id: 1479 })),
        "{:?}",
        result.err()
    );
    remove(&dir);
}

/// How the default content looks.
fn looks() -> app::Looks {
    app::default_looks()
}

/// The default content plus an archer, and its look: drawn with sprite
/// 7777.
fn with_archer() -> (Content, app::Looks, UnitKind) {
    let mut content = Content::default();
    let mut archer = content.unit(UnitKind::SOLDIER).clone();
    archer.name = "archer".to_string();
    content.units.push(archer);
    let kind = content.unit_named("archer").expect("added");
    content
        .building_mut(BuildingKind::BARRACKS)
        .trains
        .push(kind);
    let mut looks = looks();
    let mut look = looks.unit("soldier").expect("the soldier's look").clone();
    look.sprite = 7777;
    looks.units.insert("archer".to_string(), look);
    (content, looks, kind)
}

#[test]
fn a_unit_type_added_in_content_is_drawn_with_the_sprite_its_entry_names() {
    let (content, looks, archer) = with_archer();
    let mut files = graphics(&[]);
    // Three pixels wide, so it cannot be mistaken for the two-pixel default.
    files.push((common::SLP, 7777, slp(1, 3, &[3 << 2, 9, 9, 9, 0x0F])));
    let dir = install("archer", files, slp(1, 2, &TWO_PIXELS));
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &content, &looks)
        .expect("loads")
        .upload(&mut renderer);

    let sprite = sprites.unit(archer, PlayerId::ONE);
    assert_eq!(renderer.texture_size(sprite.texture), Some((3, 1)));
    remove(&dir);
}

#[test]
fn a_sprite_the_install_does_not_have_is_an_error_naming_the_entry() {
    let (content, looks, _) = with_archer();
    let dir = install("no-archer", graphics(&[]), slp(1, 2, &TWO_PIXELS));

    let result = load_images(&dir, &content, &looks);

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
            vec![(common::SLP, 1479, three_pixels())],
        )],
    );
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &Content::default(), &looks())
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

    let sprites = load_images(&dir, &Content::default(), &looks())
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

    let sprites = load_images(&dir, &Content::default(), &looks())
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

    let result = load_images(&dir, &Content::default(), &looks());

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

    let sprites = load_images(&dir, &Content::default(), &looks())
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
    let mut looks = looks();
    looks
        .units
        .get_mut("soldier")
        .expect("the soldier's look")
        .icon = 99;
    let content = Content::default();
    let dir = install("icon-missing", graphics(&[]), slp(1, 2, &TWO_PIXELS));

    let result = load_images(&dir, &content, &looks);

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
        graphics(&[(50705, common::icons_in(52, 7))]),
        slp(1, 2, &TWO_PIXELS),
    );
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &Content::default(), &looks())
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
    let mut looks = looks();
    looks
        .buildings
        .get_mut("barracks")
        .expect("the barracks' look")
        .icon = 99;
    let content = Content::default();
    let dir = install(
        "building-icon-missing",
        graphics(&[]),
        slp(1, 2, &TWO_PIXELS),
    );

    let result = load_images(&dir, &content, &looks);

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

    let sprites = load_images(&dir, &Content::default(), &looks())
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

#[test]
fn an_icon_frame_with_no_pixels_is_an_error_naming_the_entry() {
    // Every frame of this unit icon sprite is zero pixels wide.
    let dir = install(
        "icon-empty",
        graphics(&[(50730, slp(16, 0, &[0x0F]))]),
        slp(1, 2, &TWO_PIXELS),
    );

    let result = load_images(&dir, &Content::default(), &looks());

    assert!(
        matches!(&result, Err(AssetError::EmptyIcon { entry, frame: 15 }) if entry == "villager"),
        "{:?}",
        result.err()
    );
    remove(&dir);
}

// --- buildings drawn in parts -----------------------------------------------------

fn town_centre_in_parts(parts: Vec<u32>) -> app::Looks {
    let at_one_point = parts
        .into_iter()
        .map(|slp| app::Part::new(slp, 0, 0))
        .collect();
    town_centre_in_placed_parts(at_one_point)
}

/// How the default content looks with the town centre drawn in `parts`.
fn town_centre_in_placed_parts(parts: Vec<app::Part>) -> app::Looks {
    let mut looks = looks();
    looks
        .buildings
        .get_mut("town_center")
        .expect("the town centre's look")
        .parts = parts;
    looks
}

#[test]
fn a_type_drawn_in_parts_is_composed_into_one_sprite_around_their_shared_point() {
    // Part 7001 is 4 pixels wide with its point 1 pixel in; part 7002 is 2
    // wide and sits up and to the right, its point 3 pixels to its left and
    // 1 below. Together: 6 by 2 pixels, the shared point at (1, 1).
    let dir = install(
        "parts",
        graphics(&[
            (7001, common::piece(4, (1, 0), 9)),
            (7002, common::piece(2, (-3, 1), 7)),
        ]),
        slp(1, 2, &TWO_PIXELS),
    );
    let looks = town_centre_in_parts(vec![7001, 7002]);
    let content = Content::default();
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &content, &looks)
        .expect("loads")
        .upload(&mut renderer);

    let town_centre = sprites.building(BuildingKind::TOWN_CENTER, PlayerId::ONE);
    assert_eq!(renderer.texture_size(town_centre.texture), Some((6, 2)));
    assert_eq!(town_centre.hotspot, render::Vec2::new(1.0, 1.0));
    let pixels = renderer
        .texture_pixels(town_centre.texture)
        .expect("uploaded");
    let at = |x: usize, y: usize| &pixels[(y * 6 + x) * 4..(y * 6 + x) * 4 + 4];
    assert_eq!(at(0, 1), colour(9), "the first part's row");
    assert_eq!(at(3, 1), colour(9));
    assert_eq!(at(4, 0), colour(7), "the second part, above and right");
    assert_eq!(at(5, 0), colour(7));
    assert_eq!(at(0, 0)[3], 0, "between the parts is transparent");
    remove(&dir);
}

#[test]
fn a_later_part_is_drawn_over_an_earlier_one() {
    let dir = install(
        "parts-order",
        graphics(&[
            (7001, common::piece(3, (0, 0), 9)),
            (7002, common::piece(1, (-1, 0), 7)),
        ]),
        slp(1, 2, &TWO_PIXELS),
    );
    let looks = town_centre_in_parts(vec![7001, 7002]);
    let content = Content::default();
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &content, &looks)
        .expect("loads")
        .upload(&mut renderer);

    let texture = sprites
        .building(BuildingKind::TOWN_CENTER, PlayerId::ONE)
        .texture;
    let pixels = renderer.texture_pixels(texture).expect("uploaded");
    assert_eq!(
        &pixels[4..8],
        colour(7),
        "the second part covers the first's middle"
    );
    remove(&dir);
}

#[test]
fn a_part_the_install_does_not_have_is_an_error_naming_the_entry() {
    let dir = install("parts-missing", graphics(&[]), slp(1, 2, &TWO_PIXELS));
    let looks = town_centre_in_parts(vec![3596, 7777]);
    let content = Content::default();

    let result = load_images(&dir, &content, &looks);

    assert!(
        matches!(&result, Err(AssetError::MissingGraphic { entry, id: 7777 }) if entry == "town_center"),
        "{:?}",
        result.err()
    );
    remove(&dir);
}

#[test]
fn a_later_parts_transparent_pixels_leave_the_earlier_part_showing() {
    // Part 7002 covers the same three pixels as 7001, but its first is
    // transparent: 7001 shows through there.
    let dir = install(
        "parts-gap",
        graphics(&[
            (7001, common::piece(3, (0, 0), 9)),
            (7002, common::piece_with_gap(3, 1, (0, 0), 7)),
        ]),
        slp(1, 2, &TWO_PIXELS),
    );
    let looks = town_centre_in_parts(vec![7001, 7002]);
    let content = Content::default();
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &content, &looks)
        .expect("loads")
        .upload(&mut renderer);

    let texture = sprites
        .building(BuildingKind::TOWN_CENTER, PlayerId::ONE)
        .texture;
    let pixels = renderer.texture_pixels(texture).expect("uploaded");
    assert_eq!(&pixels[0..4], colour(9), "through the gap");
    assert_eq!(&pixels[4..8], colour(7));
    remove(&dir);
}

#[test]
fn a_later_parts_shadow_darkens_the_earlier_part_rather_than_replacing_it() {
    // A shadow is black at half opacity: over an opaque pixel it darkens it,
    // and the pixel stays opaque.
    let dir = install(
        "parts-shadow",
        graphics(&[
            (7001, common::piece(2, (0, 0), 9)),
            (7002, common::shadow_piece(2, (0, 0))),
        ]),
        slp(1, 2, &TWO_PIXELS),
    );
    let looks = town_centre_in_parts(vec![7001, 7002]);
    let content = Content::default();
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &content, &looks)
        .expect("loads")
        .upload(&mut renderer);

    let texture = sprites
        .building(BuildingKind::TOWN_CENTER, PlayerId::ONE)
        .texture;
    let pixels = renderer.texture_pixels(texture).expect("uploaded");
    let (under, shaded) = (colour(9), &pixels[0..4]);
    assert_eq!(shaded[3], 255, "still opaque");
    for channel in 0..3 {
        assert!(
            shaded[channel] < under[channel].max(1) || under[channel] == 0,
            "channel {channel}: {shaded:?} over {under:?}"
        );
        assert!(
            shaded[channel] >= under[channel] / 3,
            "not blacked out: {shaded:?}"
        );
    }
}

#[test]
fn parts_placed_absurdly_far_apart_are_an_error_rather_than_a_huge_image() {
    // A damaged hotspot puts one part 30,000 pixels from the other.
    let dir = install(
        "parts-far",
        graphics(&[
            (7001, common::piece(2, (0, 0), 9)),
            (7002, common::piece(2, (-30_000, 0), 7)),
        ]),
        slp(1, 2, &TWO_PIXELS),
    );
    let looks = town_centre_in_parts(vec![7001, 7002]);
    let content = Content::default();

    let result = load_images(&dir, &content, &looks);

    assert!(
        matches!(&result, Err(AssetError::PartsTooFarApart { entry }) if entry == "town_center"),
        "{:?}",
        result.err()
    );
    remove(&dir);
}

#[test]
fn a_parts_offset_moves_it_that_many_pixels_from_the_shared_point() {
    // As a graphic's piece in the original: drawn 3 pixels right of where its
    // hotspot alone would put it.
    let dir = install(
        "parts-offset",
        graphics(&[
            (7001, common::piece(2, (0, 0), 9)),
            (7002, common::piece(1, (0, 0), 7)),
        ]),
        slp(1, 2, &TWO_PIXELS),
    );
    let looks =
        town_centre_in_placed_parts(vec![app::Part::new(7001, 0, 0), app::Part::new(7002, 3, 0)]);
    let content = Content::default();
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &content, &looks)
        .expect("loads")
        .upload(&mut renderer);

    let town_centre = sprites.building(BuildingKind::TOWN_CENTER, PlayerId::ONE);
    assert_eq!(renderer.texture_size(town_centre.texture), Some((4, 1)));
    let pixels = renderer
        .texture_pixels(town_centre.texture)
        .expect("uploaded");
    assert_eq!(pixels[2 * 4 + 3], 0, "between the parts");
    assert_eq!(&pixels[12..16], colour(7), "the moved part");
    remove(&dir);
}

#[test]
fn a_type_with_no_look_is_an_error_naming_it() {
    // A replay's types are drawn with the looks of the content the game was
    // started with, matched by name; one it lacks cannot be drawn.
    let dir = install("no-look", graphics(&[]), slp(1, 2, &TWO_PIXELS));
    let mut looks = app::default_looks();
    looks.units.remove("soldier");

    let result = load_images(&dir, &Content::default(), &looks);

    assert!(
        matches!(&result, Err(AssetError::NoLook { entry }) if entry == "soldier"),
        "{:?}",
        result.err()
    );
    remove(&dir);
}

#[test]
fn each_kind_of_site_is_composed_from_the_originals_pieces() {
    // Each site's first piece, then its own picture over it: the gold mine
    // is 4479 then 2561, the stone mine 4482 then 1034, a tree 2296 then
    // 4652, and a forage bush 2560 alone.
    let dir = install(
        "site-pieces",
        graphics(&[
            (4479, common::piece(3, (0, 0), 11)),
            (2561, common::piece(1, (0, 0), 12)),
            (4482, common::piece(3, (0, 0), 21)),
            (1034, common::piece(1, (0, 0), 22)),
            (2296, common::piece(3, (0, 0), 31)),
            (4652, common::piece(1, (0, 0), 32)),
            (2560, common::piece(2, (0, 0), 41)),
        ]),
        slp(1, 2, &TWO_PIXELS),
    );
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &Content::default(), &looks())
        .expect("loads")
        .upload(&mut renderer);

    for (sprite, under, over, what) in [
        (sprites.gold_mine, 11, 12, "gold mine"),
        (sprites.stone_mine, 21, 22, "stone mine"),
        (sprites.tree, 31, 32, "tree"),
    ] {
        assert_eq!(
            renderer.texture_size(sprite.texture),
            Some((3, 1)),
            "{what}"
        );
        let pixels = renderer.texture_pixels(sprite.texture).expect("uploaded");
        assert_eq!(
            &pixels[0..4],
            colour(over),
            "{what}: its own picture on top"
        );
        assert_eq!(&pixels[4..8], colour(under), "{what}: the piece under it");
    }
    let bush = renderer
        .texture_pixels(sprites.bush.texture)
        .expect("uploaded");
    assert_eq!(&bush[0..8], [colour(41), colour(41)].concat());
    remove(&dir);
}

#[test]
fn farms_are_drawn_with_the_farm_ground_in_its_variations() {
    // Terrain 15004: the original's farm, crops and all.
    let terrain = vec![
        (common::SLP, 15001, slp(1, 2, &TWO_PIXELS)),
        (common::SLP, 15004, slp(3, 2, &TWO_PIXELS)),
    ];
    let dir = install_with(
        "farm-ground",
        graphics(&[]),
        slp(1, 2, &TWO_PIXELS),
        &[("terrain.drs", terrain)],
    );
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &Content::default(), &looks())
        .expect("loads")
        .upload(&mut renderer);

    assert_eq!(sprites.farm.len(), 3);
    assert_eq!(sprites.grass.len(), 1);
    remove(&dir);
}

#[test]
fn an_install_without_the_farm_ground_is_an_error() {
    let terrain = vec![(common::SLP, 15001, slp(1, 2, &TWO_PIXELS))];
    let dir = install_with(
        "no-farm-ground",
        graphics(&[]),
        slp(1, 2, &TWO_PIXELS),
        &[("terrain.drs", terrain)],
    );

    let result = load_images(&dir, &Content::default(), &looks());

    assert!(
        matches!(result, Err(AssetError::Missing { id: 15004, .. })),
        "{:?}",
        result.err()
    );
    remove(&dir);
}

#[test]
fn the_cursors_are_frames_of_the_originals_mouse_pointer_sprite_with_their_hotspots() {
    // The original loads its cursors from one sprite, `mcursors` (51000), of
    // 19 frames. Frame n of the fake is n + 1 pixels wide; frame 8, the axe,
    // has its hotspot moved so the test sees it carried through.
    let mut cursors = common::icons(19);
    let axe_header = 32 + 32 * 8;
    cursors[axe_header + 24..axe_header + 32].copy_from_slice(&[5, 0, 0, 0, 7, 0, 0, 0]);
    let dir = install(
        "cursors",
        graphics(&[(51000, cursors)]),
        slp(1, 2, &TWO_PIXELS),
    );
    let mut renderer = RecordingRenderer::new();

    let sprites = load_images(&dir, &Content::default(), &looks())
        .expect("loads")
        .upload(&mut renderer);

    let width = |resource| {
        renderer
            .texture_size(sprites.cursor(resource).texture)
            .map(|(w, _)| w)
    };
    assert_eq!(width(None), Some(1), "the arrow is frame 0");
    assert_eq!(
        width(Some(sim::Resource::Wood)),
        Some(9),
        "the axe is frame 8"
    );
    assert_eq!(
        sprites.cursor(Some(sim::Resource::Wood)).hotspot,
        render::Vec2::new(5.0, 7.0)
    );
    remove(&dir);
}

#[test]
fn an_install_without_the_cursors_is_an_error() {
    let files = graphics(&[])
        .into_iter()
        .filter(|(_, id, _)| *id != 51000)
        .collect();
    let dir = install("no-cursors", files, slp(1, 2, &TWO_PIXELS));

    let result = load_images(&dir, &Content::default(), &looks());

    assert!(
        matches!(result, Err(AssetError::Missing { id: 51000, .. })),
        "{:?}",
        result.err()
    );
    remove(&dir);
}
