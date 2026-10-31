//! Hotseat state and the translation of input into commands.
//!
//! Everything here is per-player interface state — camera, selection, the
//! rectangle currently being dragged — and none of it enters the simulation.
//! Two players sharing one keyboard is the cheapest way to exercise combat and
//! replay without writing an AI, and keeping the state out of `sim` is what
//! will let a networked client hold exactly one of these later.

use crate::pick;
use crate::ui::{self, Layout};
use crate::CameraController;
use render::{Camera, InputState, Key, MouseButton, Vec2};
use sim::{BuildingId, BuildingKind, Command, EntityId, PlayerId, UnitId, WorldView};
use smallvec::SmallVec;

/// One side's view of the game.
#[derive(Clone, PartialEq, Debug)]
pub struct PlayerUi {
    pub camera: CameraController,
    /// Units this player has selected, always alive and always theirs.
    pub selection: Vec<UnitId>,
    /// The building this player has selected, theirs or another player's,
    /// always standing. While one is selected no units are, and the other
    /// way round.
    pub building: Option<BuildingId>,
    /// Where the current left-drag started, in world units.
    pub drag_from: Option<Vec2>,
    /// Building kind being placed, if in placement mode. The origin tile
    /// under the cursor is the preview position.
    pub placing_building: Option<BuildingKind>,
}

impl PlayerUi {
    pub fn new(camera: CameraController) -> PlayerUi {
        PlayerUi {
            camera,
            selection: Vec::new(),
            building: None,
            drag_from: None,
            placing_building: None,
        }
    }
}

/// The application's whole interactive state.
#[derive(Clone, PartialEq, Debug)]
pub struct Session {
    /// One per player, player 1 first. Gaia has none: nobody plays it.
    players: Vec<PlayerUi>,
    active: PlayerId,
    /// Whether the mouse is held inside the window. Free by default, so the
    /// player can reach other programs; F10 locks it, which also stops the
    /// map scrolling on after the mouse leaves through an edge (the window
    /// cannot tell when it leaves).
    mouse_locked: bool,
    /// Where each unit kind's drawn body is, so a click selects what the
    /// player sees.
    bodies: pick::Bodies,
}

/// The keys that press the command panel's buttons, `1` the first.
const BUTTON_KEYS: [Key; 9] = [
    Key::Digit1,
    Key::Digit2,
    Key::Digit3,
    Key::Digit4,
    Key::Digit5,
    Key::Digit6,
    Key::Digit7,
    Key::Digit8,
    Key::Digit9,
];

impl Session {
    /// A drag shorter than this many screen pixels is a click, not a box
    /// selection. In pixels because a hand shakes in pixels: measured on the
    /// map instead, a wobble zoomed out would be a drag, and zoomed in a long
    /// drag would still be a click.
    pub const CLICK_SLOP_PIXELS: f32 = 6.0;
    /// The smallest a unit's click target gets, in screen pixels. Zoomed out
    /// a villager is drawn a couple of pixels wide, far too small to click.
    pub const MIN_PICK_PIXELS: f32 = 20.0;

    /// `players` is the map's player count, not counting Gaia.
    pub fn new(camera: Camera, map_width: i32, map_height: i32, players: u8) -> Session {
        let controller = CameraController::new(camera, map_width, map_height);
        Session {
            players: (0..players.max(1))
                .map(|_| PlayerUi::new(controller.clone()))
                .collect(),
            active: PlayerId::ONE,
            mouse_locked: false,
            bodies: pick::Bodies::default(),
        }
    }

    pub const fn active_player(&self) -> PlayerId {
        self.active
    }

    pub fn active(&self) -> &PlayerUi {
        &self.players[slot(self.active)]
    }

    fn active_mut(&mut self) -> &mut PlayerUi {
        let index = slot(self.active);
        &mut self.players[index]
    }

    pub fn camera(&self) -> Camera {
        self.active().camera.camera()
    }

    pub fn selection(&self) -> &[UnitId] {
        &self.active().selection
    }

    pub fn selected_building(&self) -> Option<BuildingId> {
        self.active().building
    }

    /// The rectangle being dragged right now, as opposite world-space corners.
    pub fn drag_box(&self, cursor_world: Vec2) -> Option<(Vec2, Vec2)> {
        self.active().drag_from.map(|from| (from, cursor_world))
    }

    /// Points one player's camera at a place on the map.
    ///
    /// Used at startup so that each side opens on their own base rather than
    /// on an empty corner, and per player because in hotseat they are looking
    /// at opposite ends of the map.
    pub fn look_at(&mut self, player: PlayerId, at: Vec2) {
        if let Some(ui) = self.players.get_mut(slot(player)) {
            ui.camera.look_at(at);
        }
    }

    /// Matches every camera to a resized window.
    pub fn resize(&mut self, viewport: Vec2) {
        for player in &mut self.players {
            player.camera.resize(viewport);
        }
    }

    /// Consumes one frame of input and returns the commands it means.
    ///
    /// `dt` drives camera motion only. Commands are not rate-limited here: the
    /// caller collects them until the next simulation tick, which is what
    /// keeps input responsive at frame rate while the simulation runs at 20 Hz.
    /// Matches picking to the bodies the sprites are drawn with.
    pub fn set_bodies(&mut self, bodies: pick::Bodies) {
        self.bodies = bodies;
    }

    /// The smallest click target at the current zoom, in tiles:
    /// [`Session::MIN_PICK_PIXELS`] on screen.
    fn min_pick(&self) -> f32 {
        Session::MIN_PICK_PIXELS / self.camera().zoom
    }

    /// Whether the mouse should be held inside the window. The application
    /// applies it to the real window; F10 toggles it.
    pub const fn mouse_locked(&self) -> bool {
        self.mouse_locked
    }

    pub fn update(&mut self, input: &InputState, dt: f32, view: WorldView<'_>) -> Vec<Command> {
        if input.key_pressed(Key::F10) {
            self.mouse_locked = !self.mouse_locked;
        }
        if input.key_pressed(Key::Tab) {
            // Drop the half-finished drag: it belongs to the player who is
            // handing over the keyboard.
            self.active_mut().drag_from = None;
            let count = self.players.len() as u8;
            self.active = PlayerId::new(self.active.number() % count + 1);
        }

        self.active_mut().camera.update(input, dt);
        self.prune_selection(view);

        let camera = self.camera();
        let cursor = camera.screen_to_world(input.cursor);

        // A press on the interface belongs to the interface: it neither
        // starts a drag nor orders anything on the map underneath.
        let layout = Layout::new(camera.viewport);
        let on_interface = layout.blocks(input.cursor);

        // Handle placement mode FIRST: left-click release places, right-click/ESC cancels.
        // Don't run update_selection in placement mode.
        let mut commands = Vec::new();
        let tc = self.town_center(view);
        let player = self.active;
        let ui = self.active_mut();
        let in_placement = ui.placing_building.is_some();

        if in_placement {
            ui.drag_from = None;
            if input.is_released(MouseButton::Left) && !on_interface {
                if let Some(kind) = ui.placing_building {
                    if let Some(tc) = tc {
                        let size = view.content().building(kind).size;
                        let origin = pick::placement_origin(cursor, size);
                        commands.push(Command::PlaceBuilding {
                            player,
                            building: tc,
                            kind,
                            origin,
                        });
                        ui.placing_building = None;
                    }
                }
            } else if input.is_pressed(MouseButton::Right) || input.key_pressed(Key::Escape) {
                ui.placing_building = None;
            }
        } else {
            // Normal mode: selection and right-click orders.
            self.update_selection(input, cursor, on_interface, view);
            if input.is_pressed(MouseButton::Right) && !on_interface {
                commands.extend(self.order(cursor, view));
            }
        }
        commands.extend(self.command_panel(input, &layout, view));
        commands
    }

    /// Drops selected units that have died or changed hands since last frame,
    /// and a selected building that has been destroyed.
    ///
    /// Without this, orders would be issued against stale identifiers every
    /// frame. The simulation would drop them, but the player would be clicking
    /// into a void with no feedback.
    fn prune_selection(&mut self, view: WorldView<'_>) {
        let owner = self.active;
        let ui = self.active_mut();
        ui.selection
            .retain(|id| view.unit(*id).is_some_and(|u| u.owner == owner));
        ui.building = ui.building.filter(|id| view.building(*id).is_some());
    }

    fn update_selection(
        &mut self,
        input: &InputState,
        cursor: Vec2,
        on_interface: bool,
        view: WorldView<'_>,
    ) {
        if input.is_pressed(MouseButton::Left) {
            if !on_interface {
                self.active_mut().drag_from = Some(cursor);
            }
            return;
        }
        if !input.is_released(MouseButton::Left) {
            return;
        }
        let owner = self.active;
        let Some(from) = self.active_mut().drag_from.take() else {
            return;
        };

        let travel = cursor - from;
        let zoom = self.camera().zoom;
        let slop = Session::CLICK_SLOP_PIXELS / zoom;
        let dragged = travel.x.abs() > slop || travel.y.abs() > slop;
        let min = self.min_pick();
        let (selection, building) = if dragged {
            // A box selects units only, as in the original, and only what
            // shows: a box dragged onto the panels stops at their edge rather
            // than reaching the units hidden under them.
            let camera = self.camera();
            let layout = Layout::new(camera.viewport);
            let mut end = input.cursor;
            end.y = end.y.clamp(layout.top_bar.max().y, layout.bottom.min.y);
            let end = camera.screen_to_world(end);
            let units = pick::units_in_box(view, owner, from, end, &self.bodies.at_least(min));
            (units, None)
        } else if let Some(unit) = pick::unit_at(view, cursor, &self.bodies, min) {
            // A unit in front of a building is what the player sees, so it
            // is picked first. Only your own units can be selected.
            let yours = view.unit(unit).is_some_and(|u| u.owner == owner);
            (yours.then_some(unit).into_iter().collect(), None)
        } else {
            // Any building can be selected, to look at; only your own will
            // offer its commands. A click on empty ground clears the
            // selection, which is how every RTS behaves and what makes a
            // misclick recoverable.
            (Vec::new(), pick::building_at(view, cursor, &self.bodies))
        };
        let ui = self.active_mut();
        ui.selection = selection;
        ui.building = building;
    }

    /// The command a right-click means, given what is under the cursor.
    fn order(&self, cursor: Vec2, view: WorldView<'_>) -> Option<Command> {
        let player = self.active;
        let units: SmallVec<[UnitId; 32]> = self.selection().iter().copied().collect();
        if units.is_empty() {
            return None;
        }

        match pick::entity_at(view, cursor, &self.bodies, self.min_pick()) {
            // A mine means gather, but only for units that gather; a soldier
            // sent to a mine should walk there rather than have its order
            // dropped.
            Some(EntityId::Site(mine)) => {
                let villagers: SmallVec<[UnitId; 32]> = units
                    .iter()
                    .copied()
                    .filter(|id| {
                        view.unit(*id)
                            .is_some_and(|u| view.content().unit(u.kind).gathers)
                    })
                    .collect();
                if villagers.is_empty() {
                    return Some(Command::Move {
                        player,
                        units,
                        target: pick::tile_at(cursor),
                    });
                }
                Some(Command::Gather {
                    player,
                    units: villagers,
                    site: mine,
                })
            }
            // Your own drop-off with loaded villagers selected: they drop
            // their loads off there, early, then stand, as in the original.
            // Units carrying nothing are left as they are.
            Some(EntityId::Building(building))
                if units
                    .iter()
                    .any(|id| self.takes_load_of(building, *id, view)) =>
            {
                Some(Command::Deposit {
                    player,
                    units: units
                        .iter()
                        .copied()
                        .filter(|id| self.takes_load_of(building, *id, view))
                        .collect(),
                    building,
                })
            }
            Some(target) if self.is_enemy(target, view) => Some(Command::Attack {
                player,
                units,
                target,
            }),
            _ => Some(Command::Move {
                player,
                units,
                target: pick::tile_at(cursor),
            }),
        }
    }

    /// Whether `building` is one of the active player's and takes what
    /// `unit` carries.
    fn takes_load_of(&self, building: BuildingId, unit: UnitId, view: WorldView<'_>) -> bool {
        let Some(unit) = view.unit(unit).filter(|u| u.carrying > 0) else {
            return false;
        };
        view.building(building).is_some_and(|b| {
            b.owner == self.active && view.content().building(b.kind).takes(unit.carried)
        })
    }

    fn is_enemy(&self, target: EntityId, view: WorldView<'_>) -> bool {
        match target {
            EntityId::Unit(id) => view.unit(id).is_some_and(|u| u.owner != self.active),
            EntityId::Building(id) => view.building(id).is_some_and(|b| b.owner != self.active),
            EntityId::Foundation(id) => view.foundation(id).is_some_and(|f| f.owner != self.active),
            EntityId::Site(_) => false,
        }
    }

    /// Finds the player's town center, if any.
    fn town_center(&self, view: WorldView<'_>) -> Option<BuildingId> {
        view.buildings()
            .find(|(_, b)| b.owner == self.active && b.kind == sim::BuildingKind::TOWN_CENTER)
            .map(|(id, _)| id)
    }

    /// The command panel: a click on one of its buttons, or the button's
    /// number key, trains that button's unit type at the selected building,
    /// or enters placement mode for that building kind when villagers are
    /// selected.
    fn command_panel(
        &mut self,
        input: &InputState,
        layout: &Layout,
        view: WorldView<'_>,
    ) -> Vec<Command> {
        let player = self.active;
        let ui = self.active_mut();

        // If in placement mode, Escape or right-click on interface cancels it.
        if ui.placing_building.is_some() {
            if input.key_pressed(Key::Escape)
                || (input.is_pressed(MouseButton::Right) && layout.blocks(input.cursor))
            {
                ui.placing_building = None;
            }
            return Vec::new();
        }

        // Building selected: show train buttons.
        if let Some(building) = ui.building {
            let trains = ui::trains_on_buttons(view, Some(building), player);
            let clicked = input
                .is_pressed(MouseButton::Left)
                .then(|| layout.buttons.iter().position(|b| b.contains(input.cursor)))
                .flatten();
            let keyed = BUTTON_KEYS
                .iter()
                .enumerate()
                .filter(|(_, key)| input.key_pressed(**key))
                .map(|(index, _)| index);
            return clicked
                .into_iter()
                .chain(keyed)
                .filter_map(|index| trains.get(index).copied())
                .map(|unit| Command::Train {
                    player,
                    building,
                    unit,
                })
                .collect();
        }

        // Your villagers selected: a button picks up its building to place.
        let builds = ui::builds_on_buttons(view, &ui.selection, player);
        let clicked = input
            .is_pressed(MouseButton::Left)
            .then(|| layout.buttons.iter().position(|b| b.contains(input.cursor)))
            .flatten();
        let keyed = BUTTON_KEYS
            .iter()
            .enumerate()
            .filter(|(_, key)| input.key_pressed(**key))
            .map(|(index, _)| index);
        if let Some(kind) = clicked
            .into_iter()
            .chain(keyed)
            .find_map(|index| builds.get(index).copied())
        {
            ui.placing_building = Some(kind);
        }

        Vec::new()
    }
}

/// A player's place in the session's table: player 1 is slot 0.
fn slot(player: PlayerId) -> usize {
    usize::from(player.number().saturating_sub(1))
}
