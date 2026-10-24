//! Hotseat state and the translation of input into commands.
//!
//! Everything here is per-player interface state — camera, selection, the
//! rectangle currently being dragged — and none of it enters the simulation.
//! Two players sharing one keyboard is the cheapest way to exercise combat and
//! replay without writing an AI, and keeping the state out of `sim` is what
//! will let a networked client hold exactly one of these later.

use crate::pick;
use crate::CameraController;
use render::{Camera, InputState, Key, MouseButton, Vec2};
use sim::{BuildingKind, Command, EntityId, PlayerId, UnitId, UnitKind, WorldView};
use smallvec::SmallVec;

/// One side's view of the game.
#[derive(Clone, PartialEq, Debug)]
pub struct PlayerUi {
    pub camera: CameraController,
    /// Units this player has selected, always alive and always theirs.
    pub selection: Vec<UnitId>,
    /// Where the current left-drag started, in world units.
    pub drag_from: Option<Vec2>,
}

impl PlayerUi {
    pub fn new(camera: CameraController) -> PlayerUi {
        PlayerUi {
            camera,
            selection: Vec::new(),
            drag_from: None,
        }
    }
}

/// The application's whole interactive state.
#[derive(Clone, PartialEq, Debug)]
pub struct Session {
    /// One per player, player 1 first. Gaia has none: nobody plays it.
    players: Vec<PlayerUi>,
    active: PlayerId,
    /// Whether the mouse is held inside the window. The window cannot tell
    /// when the mouse leaves it, so edge scrolling only runs while locked:
    /// otherwise a mouse that left through an edge would scroll the map for
    /// as long as the player used another program.
    mouse_locked: bool,
    /// Where each unit kind's drawn body is, so a click selects what the
    /// player sees.
    bodies: pick::Bodies,
}

impl Session {
    /// A drag shorter than this is a click, not a box selection.
    ///
    /// In world units, so the threshold is a fixed distance on the map rather
    /// than on screen; a shaky hand at high zoom would otherwise turn every
    /// click into an empty box that clears the selection.
    pub const CLICK_SLOP: f32 = 0.35;

    /// `players` is the map's player count, not counting Gaia.
    pub fn new(camera: Camera, map_width: i32, map_height: i32, players: u8) -> Session {
        let controller = CameraController::new(camera, map_width, map_height);
        Session {
            players: (0..players.max(1))
                .map(|_| PlayerUi::new(controller.clone()))
                .collect(),
            active: PlayerId::ONE,
            mouse_locked: true,
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

    /// Whether the mouse should be held inside the window. The application
    /// applies it to the real window; F10 toggles it.
    pub const fn mouse_locked(&self) -> bool {
        self.mouse_locked
    }

    pub fn update(&mut self, input: &InputState, dt: f32, view: WorldView<'_>) -> Vec<Command> {
        if input.key_pressed(Key::F10) {
            self.mouse_locked = !self.mouse_locked;
            for player in &mut self.players {
                player.camera.set_edge_scroll(self.mouse_locked);
            }
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

        self.update_selection(input, cursor, view);

        let mut commands = Vec::new();
        if input.is_pressed(MouseButton::Right) {
            commands.extend(self.order(cursor, view));
        }
        commands.extend(self.training(input, view));
        commands
    }

    /// Drops selected units that have died or changed hands since last frame.
    ///
    /// Without this, orders would be issued against stale identifiers every
    /// frame. The simulation would drop them, but the player would be clicking
    /// into a void with no feedback.
    fn prune_selection(&mut self, view: WorldView<'_>) {
        let owner = self.active;
        self.active_mut()
            .selection
            .retain(|id| view.unit(*id).is_some_and(|u| u.owner == owner));
    }

    fn update_selection(&mut self, input: &InputState, cursor: Vec2, view: WorldView<'_>) {
        if input.is_pressed(MouseButton::Left) {
            self.active_mut().drag_from = Some(cursor);
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
        let dragged = travel.x.abs() > Session::CLICK_SLOP || travel.y.abs() > Session::CLICK_SLOP;
        let selection = if dragged {
            pick::units_in_box(view, owner, from, cursor, &self.bodies)
        } else {
            // A click on empty ground clears the selection, which is how every
            // RTS behaves and what makes a misclick recoverable.
            pick::unit_at(view, cursor, &self.bodies)
                .filter(|id| view.unit(*id).is_some_and(|u| u.owner == owner))
                .into_iter()
                .collect()
        };
        self.active_mut().selection = selection;
    }

    /// The command a right-click means, given what is under the cursor.
    fn order(&self, cursor: Vec2, view: WorldView<'_>) -> Option<Command> {
        let player = self.active;
        let units: SmallVec<[UnitId; 32]> = self.selection().iter().copied().collect();
        if units.is_empty() {
            return None;
        }

        match pick::entity_at(view, cursor, &self.bodies) {
            // A mine means gather, but only for villagers; a soldier sent to a
            // mine should walk there rather than have its order dropped.
            Some(EntityId::Mine(mine)) => {
                let villagers: SmallVec<[UnitId; 32]> = units
                    .iter()
                    .copied()
                    .filter(|id| view.unit(*id).is_some_and(|u| u.kind == UnitKind::Villager))
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
                    mine,
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

    fn is_enemy(&self, target: EntityId, view: WorldView<'_>) -> bool {
        match target {
            EntityId::Unit(id) => view.unit(id).is_some_and(|u| u.owner != self.active),
            EntityId::Building(id) => view.building(id).is_some_and(|b| b.owner != self.active),
            EntityId::Mine(_) => false,
        }
    }

    /// Training hotkeys: one key per unit type, aimed at the first building
    /// that can make it.
    ///
    /// Picking the building for the player rather than making them select it
    /// keeps the slice to two keys. Choosing between several barracks is a
    /// user interface problem, and this slice is not testing that.
    fn training(&self, input: &InputState, view: WorldView<'_>) -> Vec<Command> {
        let player = self.active;
        [
            (Key::Digit1, UnitKind::Villager, BuildingKind::TownCenter),
            (Key::Digit2, UnitKind::Soldier, BuildingKind::Barracks),
        ]
        .into_iter()
        .filter(|(key, _, _)| input.key_pressed(*key))
        .filter_map(|(_, unit, from)| {
            let (building, _) = view
                .buildings()
                .find(|(_, b)| b.owner == player && b.kind == from)?;
            Some(Command::Train {
                player,
                building,
                unit,
            })
        })
        .collect()
    }
}

/// A player's place in the session's table: player 1 is slot 0.
fn slot(player: PlayerId) -> usize {
    usize::from(player.number().saturating_sub(1))
}
