//! Turning held keys and wheel movement into camera motion.
//!
//! Camera state is presentation, so it lives here rather than in `sim` and is
//! free to use floating point. In hotseat play each player has their own
//! controller: where someone is looking is not part of the simulation.

use render::{Camera, InputState, Key, Vec2};

/// A camera plus the keyboard and wheel bindings that move it.
#[derive(Clone, PartialEq, Debug)]
pub struct CameraController {
    camera: Camera,
    map_width: i32,
    map_height: i32,
}

impl CameraController {
    /// Pan speed in tiles per second, at any zoom.
    pub const PAN_TILES_PER_SECOND: f32 = 24.0;
    /// Longest frame that may drive camera motion, in seconds.
    ///
    /// A stalled frame — a breakpoint, a disk hitch, a window drag — would
    /// otherwise pan the camera across the whole map in one step.
    pub const MAX_FRAME_SECONDS: f32 = 0.25;
    /// Zoom factor per unit of wheel scroll.
    pub const ZOOM_PER_SCROLL: f32 = 1.1;
    /// Zoom factor per second of held zoom key.
    pub const ZOOM_PER_SECOND: f32 = 4.0;
    /// Screen pixels per tile at the widest view.
    pub const MIN_ZOOM: f32 = 4.0;
    /// Screen pixels per tile at the closest view.
    pub const MAX_ZOOM: f32 = 64.0;

    pub const fn new(camera: Camera, map_width: i32, map_height: i32) -> CameraController {
        CameraController {
            camera,
            map_width,
            map_height,
        }
    }

    pub const fn camera(&self) -> Camera {
        self.camera
    }

    /// Matches the viewport to a resized window.
    pub fn resize(&mut self, viewport: Vec2) {
        self.camera.viewport = viewport;
    }

    /// Applies one frame of input over `dt` seconds.
    pub fn update(&mut self, input: &InputState, dt: f32) {
        let dt = dt.clamp(0.0, Self::MAX_FRAME_SECONDS);

        // Zoom before panning, so the clamp below is applied at the zoom the
        // frame ends on rather than the one it started at.
        let mut zoom_factor = Self::ZOOM_PER_SCROLL.powf(input.scroll);
        if input.key_down(Key::E) {
            zoom_factor *= Self::ZOOM_PER_SECOND.powf(dt);
        }
        if input.key_down(Key::Q) {
            zoom_factor /= Self::ZOOM_PER_SECOND.powf(dt);
        }
        self.camera.zoom = (self.camera.zoom * zoom_factor).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);

        let mut pan = Vec2::ZERO;
        if input.key_down(Key::W) {
            pan.y -= 1.0;
        }
        if input.key_down(Key::S) {
            pan.y += 1.0;
        }
        if input.key_down(Key::A) {
            pan.x -= 1.0;
        }
        if input.key_down(Key::D) {
            pan.x += 1.0;
        }

        let step = Self::PAN_TILES_PER_SECOND * dt;
        self.camera.center = self.camera.center + pan * step;
        self.clamp_to_map();
    }

    /// Keeps the camera centre on the map.
    ///
    /// Clamping the centre rather than the visible rectangle means the view can
    /// overhang the map edge, which is what lets a unit standing in the corner
    /// be centred and inspected.
    fn clamp_to_map(&mut self) {
        self.camera.center.x = self.camera.center.x.clamp(0.0, self.map_width as f32);
        self.camera.center.y = self.camera.center.y.clamp(0.0, self.map_height as f32);
    }
}
