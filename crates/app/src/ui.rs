//! Where the original's interface puts its panels.
//!
//! The interface image (`interfac.drs` 51101, 800x600) is drawn stretched over
//! the whole window, so every rectangle here is measured on that image and
//! stretched the same way. Everything is in screen pixels and a pure function
//! of the window size, so it is tested without a window.

use render::Vec2;
use sim::{BuildingId, PlayerId, UnitKind, WorldView};

/// The size of the image the rectangles below were measured on.
const IMAGE: Vec2 = Vec2::new(800.0, 600.0);
/// Height of the top bar, which holds the resource slots.
const TOP_BAR_HEIGHT: f32 = 25.0;
/// Where the bottom row of panels starts, frame included.
const BOTTOM_TOP: f32 = 455.0;
/// The dark area of the command panel, inside its frame.
const COMMAND_PANEL: (f32, f32, f32, f32) = (4.0, 461.0, 206.0, 127.0);
/// The parchment of the information panel.
const INFO_PANEL: (f32, f32, f32, f32) = (219.0, 467.0, 280.0, 128.0);
/// The minimap's frame.
const MINIMAP: (f32, f32, f32, f32) = (507.0, 461.0, 290.0, 136.0);
/// The command panel's grid: five columns of three rows, as in the original.
const BUTTON_COLUMNS: usize = 5;
const BUTTON_ROWS: usize = 3;
/// The first button's top-left corner, and the distance from one button to
/// the next. Buttons are the icons' size, 36 pixels.
const BUTTON_ORIGIN: Vec2 = Vec2::new(7.0, 465.0);
const BUTTON_STEP: f32 = 41.0;
const BUTTON_SIZE: f32 = 36.0;
/// The progress bar's slot: beside the icon of the unit in training, at the
/// size of the original's bar sprite (`interfac.drs` 50764, 100x12).
const PROGRESS_BAR: (f32, f32, f32, f32) = (271.0, 499.0, 100.0, 12.0);

/// An axis-aligned rectangle in screen pixels.
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Rect {
    /// The top-left corner.
    pub min: Vec2,
    pub size: Vec2,
}

impl Rect {
    pub const fn new(min: Vec2, size: Vec2) -> Rect {
        Rect { min, size }
    }

    pub fn max(self) -> Vec2 {
        self.min + self.size
    }

    /// Whether `point` is inside, counting the top and left edges but not the
    /// bottom and right ones, so that rectangles side by side never share a
    /// point.
    pub fn contains(self, point: Vec2) -> bool {
        let max = self.max();
        point.x >= self.min.x && point.y >= self.min.y && point.x < max.x && point.y < max.y
    }

    /// Whether `other` lies wholly inside.
    pub fn contains_rect(self, other: Rect) -> bool {
        let (max, other_max) = (self.max(), other.max());
        other.min.x >= self.min.x
            && other.min.y >= self.min.y
            && other_max.x <= max.x
            && other_max.y <= max.y
    }

    /// Stretched by `scale` along each axis, from the window's corner.
    fn scaled(self, scale: Vec2) -> Rect {
        Rect::new(
            Vec2::new(self.min.x * scale.x, self.min.y * scale.y),
            Vec2::new(self.size.x * scale.x, self.size.y * scale.y),
        )
    }
}

fn rect((x, y, w, h): (f32, f32, f32, f32)) -> Rect {
    Rect::new(Vec2::new(x, y), Vec2::new(w, h))
}

/// The interface's rectangles for one window size.
#[derive(Clone, PartialEq, Debug)]
pub struct Layout {
    pub top_bar: Rect,
    /// The whole bottom row: the three panels and the frame around them.
    pub bottom: Rect,
    pub command_panel: Rect,
    /// The command panel's buttons, row by row, left to right.
    pub buttons: [Rect; BUTTON_COLUMNS * BUTTON_ROWS],
    pub info_panel: Rect,
    pub progress_bar: Rect,
    pub minimap: Rect,
}

impl Layout {
    pub fn new(window: Vec2) -> Layout {
        let scale = Vec2::new(window.x / IMAGE.x, window.y / IMAGE.y);
        let buttons = std::array::from_fn(|i| {
            let (column, row) = (i % BUTTON_COLUMNS, i / BUTTON_COLUMNS);
            let at = BUTTON_ORIGIN + Vec2::new(column as f32, row as f32) * BUTTON_STEP;
            Rect::new(at, Vec2::new(BUTTON_SIZE, BUTTON_SIZE)).scaled(scale)
        });
        Layout {
            top_bar: rect((0.0, 0.0, IMAGE.x, TOP_BAR_HEIGHT)).scaled(scale),
            bottom: rect((0.0, BOTTOM_TOP, IMAGE.x, IMAGE.y - BOTTOM_TOP)).scaled(scale),
            command_panel: rect(COMMAND_PANEL).scaled(scale),
            buttons,
            info_panel: rect(INFO_PANEL).scaled(scale),
            progress_bar: rect(PROGRESS_BAR).scaled(scale),
            minimap: rect(MINIMAP).scaled(scale),
        }
    }

    /// Whether a click at `point` lands on the interface rather than the map.
    pub fn blocks(&self, point: Vec2) -> bool {
        self.top_bar.contains(point) || self.bottom.contains(point)
    }
}

/// What the command panel's buttons train, in button order, for `player`
/// with `building` selected: the building's `trains` list, as many as there
/// are buttons. Empty for no building or another player's: only your own
/// buildings take your commands.
pub fn trains_on_buttons(
    view: WorldView<'_>,
    building: Option<BuildingId>,
    player: PlayerId,
) -> Vec<UnitKind> {
    building
        .and_then(|id| view.building(id))
        .filter(|b| b.owner == player)
        .map(|b| {
            let trains = &view.content().building(b.kind).trains;
            trains
                .iter()
                .copied()
                .take(BUTTON_COLUMNS * BUTTON_ROWS)
                .collect()
        })
        .unwrap_or_default()
}
