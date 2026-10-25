//! Turning the mouse at the screen edge and the wheel into camera motion.
//!
//! Camera state is presentation, so it lives here rather than in `sim` and is
//! free to use floating point. In hotseat play each player has their own
//! controller: where someone is looking is not part of the simulation.

use crate::ui::Layout;
use render::{Camera, InputState, MouseButton, Vec2};

/// A camera plus the mouse bindings that move it. No key moves the view.
#[derive(Clone, PartialEq, Debug)]
pub struct CameraController {
    camera: Camera,
    map_width: i32,
    map_height: i32,
}

impl CameraController {
    /// Edge-scroll speed in tiles per second, at any zoom.
    pub const PAN_TILES_PER_SECOND: f32 = 48.0;
    /// Longest frame that may drive camera motion, in seconds.
    ///
    /// A stalled frame — a breakpoint, a disk hitch, a window drag — would
    /// otherwise pan the camera across the whole map in one step.
    pub const MAX_FRAME_SECONDS: f32 = 0.25;
    /// How close to a window edge, in screen pixels, the cursor pans the map.
    pub const EDGE_SCROLL_PIXELS: f32 = 8.0;
    /// Zoom factor per unit of wheel scroll.
    pub const ZOOM_PER_SCROLL: f32 = 1.1;
    /// Screen pixels per tile at the widest view, at most: the view also
    /// stops zooming out once the map fills the window.
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

    /// Centres the open middle of the window, between the top bar and the
    /// bottom panels, on a world position.
    pub fn look_at(&mut self, at: Vec2) {
        let open = OpenMiddle::of(&self.camera);
        self.camera.center = at - Vec2::new(0.0, open.offset);
        self.clamp_to_map();
    }

    /// Matches the viewport to a resized window.
    pub fn resize(&mut self, viewport: Vec2) {
        self.camera.viewport = viewport;
        self.clamp_to_map();
    }

    /// Applies one frame of input over `dt` seconds.
    pub fn update(&mut self, input: &InputState, dt: f32) {
        let dt = dt.clamp(0.0, Self::MAX_FRAME_SECONDS);

        // Zoom before panning, so the clamp below is applied at the zoom the
        // frame ends on rather than the one it started at.
        // Only the wheel zooms: the mouse alone moves the view, and the
        // keyboard is kept for game shortcuts.
        let zoom_factor = Self::ZOOM_PER_SCROLL.powf(input.scroll);
        self.camera.zoom *= zoom_factor;
        self.clamp_to_map();

        // Only the mouse at a screen edge pans, as in the original; there are
        // no pan keys. Not while dragging a selection box, which would slide
        // the map out from under the box, and not before the cursor has really
        // been seen, which would drift the view towards the corner the backend
        // reports by default.
        let pan = if input.cursor_seen && !input.is_down(MouseButton::Left) {
            self.edge_direction(input.cursor)
        } else {
            Vec2::ZERO
        };

        let step = Self::PAN_TILES_PER_SECOND * dt;
        self.camera.center = self.camera.center + pan * step;
        self.clamp_to_map();
    }

    /// Which way the cursor's position pushes the view, as a unit vector, or
    /// zero when the cursor is not at an edge.
    ///
    /// The direction runs from the screen centre to the cursor, so the upper
    /// part of the left edge pans left and up, and a corner pans diagonally.
    ///
    /// Only inside the window. In a window, a cursor outside it means the
    /// player is using another program; in full screen the cursor cannot
    /// leave, so pushing against the border stays inside the band.
    fn edge_direction(&self, cursor: Vec2) -> Vec2 {
        let viewport = self.camera.viewport;
        let inside = (0.0..viewport.x).contains(&cursor.x) && (0.0..viewport.y).contains(&cursor.y);
        let band = Self::EDGE_SCROLL_PIXELS;
        let at_edge = cursor.x < band
            || cursor.y < band
            || cursor.x >= viewport.x - band
            || cursor.y >= viewport.y - band;
        let from_centre = cursor - viewport * 0.5;
        let length = (from_centre.x * from_centre.x + from_centre.y * from_centre.y).sqrt();
        // A window too small to have a middle puts its centre in the band.
        if !inside || !at_edge || length == 0.0 {
            return Vec2::ZERO;
        }
        from_centre * (1.0 / length)
    }

    /// Keeps the open middle of the window full of map: nothing beyond the
    /// map's edge is shown except under the interface.
    ///
    /// The zoom stops where the map fills the open middle, and the view stops
    /// where the open middle's edge meets the map's. The interface spans the
    /// window's width, so sideways the open middle is the whole window.
    fn clamp_to_map(&mut self) {
        let (width, height) = (self.map_width as f32, self.map_height as f32);
        let viewport = self.camera.viewport;
        let open_height = OpenMiddle::height_fraction() * viewport.y;
        let fill = (viewport.x / width).max(open_height / height);
        let widest = Self::MIN_ZOOM.max(fill);
        self.camera.zoom = self.camera.zoom.clamp(widest, Self::MAX_ZOOM.max(widest));

        // In world units: half the open middle's size, and how far its centre
        // sits from the window's.
        let open = OpenMiddle::of(&self.camera);
        let half = Vec2::new(viewport.x, open_height) * (0.5 / self.camera.zoom);
        let clamp_axis = |centre: f32, half: f32, size: f32| {
            if half * 2.0 >= size {
                size / 2.0
            } else {
                centre.clamp(half, size - half)
            }
        };
        self.camera.center.x = clamp_axis(self.camera.center.x, half.x, width);
        let open_centre = self.camera.center.y + open.offset;
        self.camera.center.y = clamp_axis(open_centre, half.y, height) - open.offset;
    }
}

/// The part of the window the interface leaves open, between the top bar and
/// the bottom panels.
struct OpenMiddle {
    /// How far the open middle's centre sits below the window's, in world
    /// units at the camera's zoom. Negative: the bottom panels are taller
    /// than the top bar, so the open middle's centre is above the window's.
    offset: f32,
}

impl OpenMiddle {
    fn of(camera: &Camera) -> OpenMiddle {
        let layout = Layout::new(camera.viewport);
        let (top, bottom) = (layout.top_bar.max().y, layout.bottom.min.y);
        let centre = (top + bottom) / 2.0;
        OpenMiddle {
            offset: (centre - camera.viewport.y / 2.0) / camera.zoom,
        }
    }

    /// The open middle's height as a fraction of the window's. The layout
    /// stretches with the window, so this is the same at every size.
    fn height_fraction() -> f32 {
        let layout = Layout::new(Vec2::new(1.0, 1.0));
        layout.bottom.min.y - layout.top_bar.max().y
    }
}
