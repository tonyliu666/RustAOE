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
