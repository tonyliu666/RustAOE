//! How long decoding the sprites takes, on the real install: run in release
//! with `AOE_ASSETS` set (and `AOE_CONTENT` naming a content file to time
//! too), `cargo test --release -p app --test load_time -- --ignored
//! --nocapture`. Results go in `docs/benchmarks.md`.

use std::path::Path;
use std::time::Instant;

/// The median of five loads, in milliseconds.
fn median_load_ms(dir: &Path, content: &sim::Content, looks: &app::Looks) -> f64 {
    let mut times: Vec<f64> = (0..5)
        .map(|_| {
            let start = Instant::now();
            app::load_images(dir, content, looks).expect("the install draws it");
            start.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    times.sort_by(f64::total_cmp);
    times[2]
}

#[test]
#[ignore = "needs the game's data: set AOE_ASSETS and run with --ignored"]
fn sprite_load_time_on_the_real_install() {
    let dir = std::env::var("AOE_ASSETS").expect("AOE_ASSETS names an install");
    let dir = Path::new(&dir);

    let default = sim::Content::default();
    println!(
        "default content ({} unit, {} building types): {:.1} ms",
        default.units.len(),
        default.buildings.len(),
        median_load_ms(dir, &default, &app::default_looks())
    );
    if let Ok(path) = std::env::var("AOE_CONTENT") {
        let content = app::load_content(Path::new(&path)).expect("the content file loads");
        let looks = app::game_looks(Some(Path::new(&path))).expect("its looks load");
        println!(
            "{path} ({} unit, {} building types): {:.1} ms",
            content.units.len(),
            content.buildings.len(),
            median_load_ms(dir, &content, &looks)
        );
    }
}
