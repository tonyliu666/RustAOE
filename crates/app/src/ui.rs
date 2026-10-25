//! Where the original's interface puts its panels.
//!
//! The interface image (`interfac.drs` 51101, 800x600) is drawn stretched over
//! the whole window, so every rectangle here is measured on that image and
//! stretched the same way. Everything is in screen pixels and a pure function
//! of the window size, so it is tested without a window.

use render::Vec2;
use sim::{BuildingId, PlayerId, Resource, UnitKind, WorldView};

/// The size of the image the rectangles below were measured on.
const IMAGE: Vec2 = Vec2::new(800.0, 600.0);
/// Height of the top bar, which holds the resource slots.
const TOP_BAR_HEIGHT: f32 = 25.0;
/// Where each resource's amount is written in the top bar: the left end of
/// the baseline, just after the slot's icon. The slots run wood, food, gold,
/// stone, then population, which waits for a population cap.
const WOOD_AT: Vec2 = Vec2::new(32.0, 18.0);
const FOOD_AT: Vec2 = Vec2::new(108.0, 18.0);
const GOLD_AT: Vec2 = Vec2::new(186.0, 18.0);
const STONE_AT: Vec2 = Vec2::new(260.0, 18.0);
/// The top bar's text height at 800x600: the slots are 17 pixels tall.
const BAR_TEXT_PX: f32 = 14.0;
/// The debug readout: just below the top bar, at the left.
const DEBUG_AT: Vec2 = Vec2::new(8.0, 41.0);
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
// The information panel, top to bottom: the selected building's icon with
// its name and hit points beside it, then the unit in training with its
// progress bar, then the units queued behind it.
const BUILDING_ICON: (f32, f32, f32, f32) = (227.0, 473.0, 36.0, 36.0);
/// Where the name's and the hit points' text sits: the left end of its
/// baseline.
const NAME_AT: Vec2 = Vec2::new(271.0, 486.0);
const HIT_POINTS_AT: Vec2 = Vec2::new(271.0, 504.0);
/// The panel's text height at 800x600.
const TEXT_PX: f32 = 16.0;
const TRAINING_ICON: (f32, f32, f32, f32) = (227.0, 517.0, 36.0, 36.0);
/// The progress bar's slot: beside the icon of the unit in training, at the
/// size of the original's bar sprite (`interfac.drs` 50764, 100x12).
const PROGRESS_BAR: (f32, f32, f32, f32) = (271.0, 529.0, 100.0, 12.0);
/// The queue's small icons: the first one's corner, their size, and the
/// distance from one to the next.
const QUEUE_ORIGIN: Vec2 = Vec2::new(227.0, 560.0);
const QUEUE_ICON: f32 = 24.0;
const QUEUE_STEP: f32 = 28.0;
/// Units waiting behind the one in training.
const QUEUE_SLOTS: usize = sim::Building::QUEUE_LIMIT - 1;

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

fn point(at: Vec2, scale: Vec2) -> Vec2 {
    Vec2::new(at.x * scale.x, at.y * scale.y)
}

fn rect((x, y, w, h): (f32, f32, f32, f32)) -> Rect {
    Rect::new(Vec2::new(x, y), Vec2::new(w, h))
}

/// The interface's rectangles for one window size.
#[derive(Clone, PartialEq, Debug)]
pub struct Layout {
    pub top_bar: Rect,
    /// Where each resource's amount is written, in [`Resource::ALL`] order;
    /// see [`Layout::resource_at`].
    resources: [Vec2; 4],
    /// The top bar's text height, in screen pixels.
    pub bar_text_px: f32,
    /// Where the debug readout is written.
    pub debug_at: Vec2,
    /// The whole bottom row: the three panels and the frame around them.
    pub bottom: Rect,
    pub command_panel: Rect,
    /// The command panel's buttons, row by row, left to right.
    pub buttons: [Rect; BUTTON_COLUMNS * BUTTON_ROWS],
    pub info_panel: Rect,
    pub building_icon: Rect,
    pub name_at: Vec2,
    pub hit_points_at: Vec2,
    /// The information panel's text height, in screen pixels.
    pub text_px: f32,
    pub training_icon: Rect,
    pub progress_bar: Rect,
    /// One slot per unit waiting behind the one in training, oldest first.
    pub queue_icons: [Rect; QUEUE_SLOTS],
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
            resources: Resource::ALL.map(|r| {
                let at = match r {
                    Resource::Food => FOOD_AT,
                    Resource::Wood => WOOD_AT,
                    Resource::Gold => GOLD_AT,
                    Resource::Stone => STONE_AT,
                };
                point(at, scale)
            }),
            bar_text_px: BAR_TEXT_PX * scale.y,
            debug_at: point(DEBUG_AT, scale),
            bottom: rect((0.0, BOTTOM_TOP, IMAGE.x, IMAGE.y - BOTTOM_TOP)).scaled(scale),
            command_panel: rect(COMMAND_PANEL).scaled(scale),
            buttons,
            info_panel: rect(INFO_PANEL).scaled(scale),
            building_icon: rect(BUILDING_ICON).scaled(scale),
            name_at: point(NAME_AT, scale),
            hit_points_at: point(HIT_POINTS_AT, scale),
            text_px: TEXT_PX * scale.y,
            training_icon: rect(TRAINING_ICON).scaled(scale),
            progress_bar: rect(PROGRESS_BAR).scaled(scale),
            queue_icons: std::array::from_fn(|i| {
                let at = QUEUE_ORIGIN + Vec2::new(QUEUE_STEP * i as f32, 0.0);
                Rect::new(at, Vec2::new(QUEUE_ICON, QUEUE_ICON)).scaled(scale)
            }),
            minimap: rect(MINIMAP).scaled(scale),
        }
    }

    /// Where `resource`'s amount is written in the top bar.
    pub fn resource_at(&self, resource: Resource) -> Vec2 {
        let index = Resource::ALL
            .iter()
            .position(|r| *r == resource)
            .expect("every resource is in ALL");
        self.resources[index]
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
