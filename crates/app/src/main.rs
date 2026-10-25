//! The game binary: window, fixed-timestep loop, and wiring.

/// How the binary was asked to run.
///
/// Parsed by hand rather than with a command-line crate: three options do not
/// justify the dependency, and the binary is the only place they are used.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
struct Options {
    /// Write a replay of this session here.
    record: Option<std::path::PathBuf>,
    /// Play this recording back instead of reading live input.
    play: Option<std::path::PathBuf>,
    /// Replay a recording headlessly and check its checksums.
    verify: Option<std::path::PathBuf>,
    /// Read the game's numbers from this RON file instead of the built-in
    /// defaults.
    content: Option<std::path::PathBuf>,
    /// The extracted install to draw the original game's sprites from.
    /// Falls back to the `AOE_ASSETS` environment variable.
    assets: Option<std::path::PathBuf>,
    seed: u64,
}

const USAGE: &str = "\
usage: aoe [--record FILE] [--play FILE] [--verify FILE] [--content FILE] [--assets DIR] [--seed N]

  --record FILE  write this session's commands to FILE
  --play FILE    replay FILE instead of reading input
  --verify FILE  replay FILE without a window and check its checksums
  --content FILE read unit and building numbers from FILE (RON); a
                 replay always uses the numbers it was recorded with
  --assets DIR   the extracted Age of Empires II install to draw with (the
                 folder holding data/graphics.drs); defaults to $AOE_ASSETS
  --seed N       seed the simulation (default 24065)
";

/// The default seed, used whenever one is not given.
const DEFAULT_SEED: u64 = 24065;

fn parse(args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options {
        seed: DEFAULT_SEED,
        ..Options::default()
    };
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--record" => options.record = Some(value()?.into()),
            "--play" => options.play = Some(value()?.into()),
            "--verify" => options.verify = Some(value()?.into()),
            "--content" => options.content = Some(value()?.into()),
            "--assets" => options.assets = Some(value()?.into()),
            "--seed" => {
                let raw = value()?;
                options.seed = raw
                    .parse()
                    .map_err(|_| format!("--seed wants a number, got {raw}"))?;
            }
            "--help" | "-h" => return Err(USAGE.to_string()),
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }
    Ok(options)
}

/// Replays a recording with no window and reports the first divergence.
///
/// This is the determinism regression test in binary form: it is what a bug
/// report attaches, and what continuous integration can run without a GPU.
fn verify(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let replay = app::load(path)?;
    sim::verify(&replay)?;
    println!(
        "replay ok: {} ticks, {} command batches, {} checksums",
        replay.last_tick(),
        replay.commands.len(),
        replay.checksums.len()
    );
    Ok(())
}

#[cfg(feature = "window")]
mod game {
    use app::{draw_world, Hud, Session};
    use macroquad::prelude as mq;
    use render::macroquad_backend::{MacroquadInput, MacroquadRenderer};
    use render::{Camera, InputSource, Vec2};
    use sim::{Command, Content, FxVec2, MapSpec, Replay, UnitId, World};
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// Simulation rate. The renderer runs as fast as the window allows.
    const TICK_SECONDS: f32 = 0.05; // 20 Hz
    /// Ticks the loop may run in one frame before giving up on catching up.
    ///
    /// Under sustained load the simulation falls behind wall-clock time rather
    /// than spiralling: each frame would otherwise take longer than the ticks
    /// it is trying to make up.
    const MAX_CATCH_UP_TICKS: u32 = 5;
    /// Starting zoom, in screen pixels per tile: one screen pixel per sprite
    /// pixel for units, the size they are drawn at in the original.
    const START_ZOOM: f32 = app::UNIT_PIXELS_PER_TILE;

    pub struct Config {
        pub seed: u64,
        /// Numbers for a new game. `None` during playback, which uses the
        /// replay's own.
        pub content: Option<Content>,
        pub record: Option<PathBuf>,
        pub play: Option<PathBuf>,
        /// The extracted install to take sprites from.
        pub assets: PathBuf,
    }

    pub async fn run(config: Config) -> Result<(), Box<dyn std::error::Error>> {
        // A recording carries its own seed, map, and content, so playback
        // must use them rather than whatever this invocation was configured
        // with.
        let playback = config.play.as_deref().map(app::load).transpose()?;
        let (seed, map, content) = match (&playback, config.content) {
            (Some(replay), _) => (replay.seed, replay.map.clone(), replay.content.clone()),
            (None, Some(content)) => {
                let map = MapSpec::proof_of_concept(&content)?;
                map.validate_for(&content)?;
                (config.seed, map, content)
            }
            (None, None) => return Err("no content for a new game".into()),
        };

        let mut world = World::with_content(seed, map.clone(), content.clone());
        let mut recorder = config
            .record
            .as_ref()
            .map(|_| Replay::with_content(seed, map.clone(), content.clone()));

        let mut renderer = MacroquadRenderer::new();
        // Decoded once and uploaded now that the window, and with it the
        // graphics context, exists. A bad install is an error at startup,
        // not a crash mid-game.
        let sprites = app::load_images(&config.assets, &content)?.upload(&mut renderer);
        let mut input_source = MacroquadInput::new();
        let mut session = Session::new(
            Camera::new(Vec2::ZERO, START_ZOOM, MacroquadRenderer::viewport()),
            map.width(),
            map.height(),
            map.players(),
        );

        for player in (1..=map.players()).map(sim::PlayerId::new) {
            if let Some(home) = map.home(player, &content) {
                session.look_at(player, Vec2::new(home.x as f32, home.y as f32));
            }
        }

        // Clicks select what is drawn: the sprites' bodies, not just feet.
        session.set_bodies(sprites.bodies(&content));

        // The mouse starts free to leave the window; F10 holds it inside, as
        // AoE2's "lock mouse to window" does. See `Session::mouse_locked`.
        mq::set_cursor_grab(session.mouse_locked());

        let mut pending: Vec<Command> = Vec::new();
        let mut previous: HashMap<UnitId, FxVec2> = world.view().snapshot().into_iter().collect();
        let mut accumulator = 0.0;

        loop {
            let dt = mq::get_frame_time();
            let input = input_source.poll();
            if input.key_pressed(render::Key::Escape) {
                break;
            }

            session.resize(MacroquadRenderer::viewport());
            let was_locked = session.mouse_locked();
            let commands = session.update(&input, dt, world.view());
            if session.mouse_locked() != was_locked {
                mq::set_cursor_grab(session.mouse_locked());
            }
            // Live input is ignored during playback, apart from the camera:
            // watching a replay from a different angle is useful, changing its
            // outcome is not.
            if playback.is_none() {
                pending.extend(commands);
            }

            accumulator += dt;
            let mut ticks = 0;
            while accumulator >= TICK_SECONDS && ticks < MAX_CATCH_UP_TICKS {
                let tick = world.tick();
                let batch: Vec<Command> = match &playback {
                    Some(replay) => replay.commands_at(tick).to_vec(),
                    None => std::mem::take(&mut pending),
                };
                if let Some(recorder) = &mut recorder {
                    recorder.record(tick, &batch);
                    recorder.record_checksum(tick, world.checksum());
                }
                previous = world.view().snapshot().into_iter().collect();
                world.step(&batch);
                accumulator -= TICK_SECONDS;
                ticks += 1;
            }
            if accumulator >= TICK_SECONDS {
                accumulator = 0.0;
            }

            let cursor = session.camera().screen_to_world(input.cursor);
            let hud = Hud {
                active_player: session.active_player(),
                selection: session.selection(),
                building: session.selected_building(),
                drag_box: session.drag_box(cursor),
                replaying: playback.is_some(),
            };
            let alpha = (accumulator / TICK_SECONDS).clamp(0.0, 1.0);
            draw_world(
                &mut renderer,
                &session.camera(),
                world.view(),
                &previous,
                alpha,
                &hud,
                &sprites,
            );
            mq::next_frame().await;
        }

        if let (Some(path), Some(recorder)) = (&config.record, &recorder) {
            app::save(path, recorder)?;
            println!("wrote {} ({} ticks)", path.display(), world.tick());
        }
        Ok(())
    }
}

/// Checks the install and the content, then runs the game.
///
/// The install is checked first: without it there is nothing to draw, and
/// saying so beats loading content for a game that cannot start.
#[cfg(feature = "window")]
async fn start(options: &Options) -> Result<(), Box<dyn std::error::Error>> {
    let assets = app::assets_dir(options.assets.as_deref(), std::env::var_os("AOE_ASSETS"))?;
    let content = app::new_game_content(options.play.is_some(), options.content.as_deref())?;
    game::run(game::Config {
        seed: options.seed,
        content,
        record: options.record.clone(),
        play: options.play.clone(),
        assets,
    })
    .await
}

#[cfg(feature = "window")]
#[macroquad::main("RTS proof of concept")]
async fn main() {
    let options = match parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

    let result = match &options.verify {
        Some(path) => verify(path),
        None => start(&options).await,
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

/// Without the `window` feature there is no renderer, so the binary can still
/// check a replay but cannot play one.
#[cfg(not(feature = "window"))]
fn main() {
    let options = match parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let Some(path) = options.verify else {
        eprintln!("built without the `window` feature; only --verify is available");
        std::process::exit(2);
    };
    if let Err(error) = verify(&path) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
