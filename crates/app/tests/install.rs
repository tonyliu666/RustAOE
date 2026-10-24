//! Loading the game's sprites from an install, end to end, with a fake
//! install written by the test.

mod common;

use app::{load_images, AssetError};
use common::{colour, graphics, install, remove, slp, TWO_PIXELS};
use render::RecordingRenderer;

#[test]
fn each_player_slot_holds_that_players_colours() {
    let dir = install("colours", graphics(&[]), slp(2, 2, &TWO_PIXELS));
    let images = load_images(&dir).expect("a complete install loads");
    let mut renderer = RecordingRenderer::new();

    let sprites = images.upload(&mut renderer);

    for player in 0..9u8 {
        let pixels = renderer
            .texture_pixels(sprites.villager[player as usize].texture)
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

    let sprites = load_images(&dir).expect("loads").upload(&mut renderer);

    let tree = renderer
        .texture_pixels(sprites.tree.texture)
        .expect("uploaded");
    assert_eq!(&tree[0..4], colour(3));
    remove(&dir);
}

#[test]
fn grass_with_no_frames_is_an_error_rather_than_a_crash_while_drawing() {
    let dir = install("no-grass", graphics(&[]), slp(0, 2, &TWO_PIXELS));

    let result = load_images(&dir);

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

    let result = load_images(&dir);

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

    let result = load_images(&dir);

    assert!(
        matches!(result, Err(AssetError::EmptySprite { id: 1473 })),
        "{:?}",
        result.err()
    );
    remove(&dir);
}
