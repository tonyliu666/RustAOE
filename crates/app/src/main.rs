//! The game binary: window, fixed-timestep loop, and wiring.

#[cfg(not(feature = "window"))]
fn main() {
    eprintln!("built without the `window` feature; nothing to run");
}

#[cfg(feature = "window")]
mod game {
    use app::{draw_world, CameraController};
    use macroquad::prelude as mq;
    use render::macroquad_backend::{MacroquadInput, MacroquadRenderer};
    use render::{Camera, InputSource, Vec2};
    use sim::{MapSpec, TilePos, World};

    /// Simulation rate. The renderer runs as fast as the window allows.
    const TICK_SECONDS: f32 = 0.05; // 20 Hz
    /// Ticks the loop may run in one frame before giving up on catching up.
    ///
    /// Under sustained load the simulation falls behind wall-clock time rather
    /// than spiralling: each frame would otherwise take longer than the ticks
    /// it is trying to make up.
    const MAX_CATCH_UP_TICKS: u32 = 5;
    const MAP_SIZE: i32 = 128;

    /// A placeholder map until maps are loaded from RON.
    ///
    /// A ring of blocked tiles around the middle, purely so that terrain
    /// colouring and camera movement are visibly doing something.
    fn map() -> MapSpec {
        let blocked = (0..MAP_SIZE)
            .filter(|i| i % 8 == 0)
            .map(|i| TilePos::new(i, MAP_SIZE / 2));
        MapSpec::new(MAP_SIZE, MAP_SIZE).with_blocked(blocked)
    }

    pub async fn run() {
        let mut world = World::new(0x5EED, map());
        let mut renderer = MacroquadRenderer::new();
        let mut input_source = MacroquadInput::new();
        let mut camera = CameraController::new(
            Camera::new(
                Vec2::new(MAP_SIZE as f32 / 2.0, MAP_SIZE as f32 / 2.0),
                16.0,
                MacroquadRenderer::viewport(),
            ),
            MAP_SIZE,
            MAP_SIZE,
        );
        let mut accumulator = 0.0;

        loop {
            let dt = mq::get_frame_time();
            let input = input_source.poll();
            if input.key_pressed(render::Key::Escape) {
                return;
            }

            camera.resize(MacroquadRenderer::viewport());
            camera.update(&input, dt);

            accumulator += dt;
            let mut ticks = 0;
            while accumulator >= TICK_SECONDS && ticks < MAX_CATCH_UP_TICKS {
                world.step(&[]);
                accumulator -= TICK_SECONDS;
                ticks += 1;
            }
            if accumulator >= TICK_SECONDS {
                accumulator = 0.0;
            }

            draw_world(&mut renderer, &camera.camera(), world.view());
            mq::next_frame().await;
        }
    }
}

#[cfg(feature = "window")]
#[macroquad::main("RTS proof of concept")]
async fn main() {
    game::run().await;
}
