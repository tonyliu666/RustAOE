//! Choosing the content a new game starts with.

use app::{new_game_content, ContentFileError};
use sim::Content;

fn broken_file(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("aoe-startup-test-{name}.ron"));
    std::fs::write(&path, "not a content file").expect("the scratch file should write");
    path
}

#[test]
fn with_no_content_file_a_new_game_uses_the_default_content() {
    let content = new_game_content(false, None).expect("the built-in content loads");

    assert_eq!(content, Some(Content::default()));
}

#[test]
fn a_broken_content_file_stops_a_new_game() {
    let path = broken_file("new-game");

    let result = new_game_content(false, Some(&path));

    assert!(
        matches!(result, Err(ContentFileError::Malformed(_))),
        "{result:?}"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn playback_ignores_the_content_file_so_a_broken_one_does_not_stop_it() {
    // A replay plays with the content it was recorded with, so `--content`
    // has nothing to say during playback, broken or not.
    let path = broken_file("playback");

    let result = new_game_content(true, Some(&path));

    assert!(matches!(result, Ok(None)), "{result:?}");
    let _ = std::fs::remove_file(&path);
}

// --- where the game's sprites come from ------------------------------------

use app::assets_dir;
use std::path::{Path, PathBuf};

#[test]
fn the_assets_option_names_the_install() {
    let dir = assets_dir(Some(Path::new("/games/aoe2")), Some("/elsewhere".into()));

    assert_eq!(dir, Ok(PathBuf::from("/games/aoe2")));
}

#[test]
fn without_the_option_the_environment_names_the_install() {
    let dir = assets_dir(None, Some("/from/env".into()));

    assert_eq!(dir, Ok(PathBuf::from("/from/env")));
}

#[test]
fn with_neither_there_is_no_game_to_draw_and_the_error_says_how_to_fix_it() {
    let error = assets_dir(None, None).expect_err("nothing to draw with");

    assert!(error.contains("--assets"), "{error}");
    assert!(error.contains("AOE_ASSETS"), "{error}");
}
