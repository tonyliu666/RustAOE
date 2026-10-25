//! Selection, hotseat switching, and the input-to-command mapping.
//!
//! Input is synthesised as a plain `InputState`, so all of this is testable
//! without a window. That is the reason `InputSource` exists as a trait.

use app::Session;
use render::{Camera, InputState, Key, MouseButton, Vec2};
use sim::{BuildingKind, Command, EntityId, MapSpec, PlayerId, TilePos, UnitId, UnitKind, World};

/// A camera showing the middle 8x8 tiles of the 16x16 map, with room to
/// scroll either way.
///
/// The view lies inside the map, so the controller leaves it as it is on the
/// first frame; a camera it had to move would make the one the tests convert
/// with differ from the one the session actually uses.
fn camera() -> Camera {
    Camera::new(Vec2::new(8.0, 8.0), 8.0, Vec2::new(64.0, 64.0))
}

fn session() -> Session {
    Session::new(camera(), 16, 16, 2)
}

/// A map with two villagers for player one and one for player two, plus a
/// building each so that neither side has already lost.
fn map() -> MapSpec {
    MapSpec::new(16, 16)
        .with_starting_gold(200)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(0, 0))
        .with_building(BuildingKind::BARRACKS, PlayerId::ONE, TilePos::new(0, 12))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(12, 0),
        )
        .with_mine(TilePos::new(6, 12), 5000)
        .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(5, 5))
        .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(6, 5))
        .with_unit(UnitKind::VILLAGER, PlayerId::TWO, TilePos::new(9, 9))
        .with_unit(UnitKind::SOLDIER, PlayerId::TWO, TilePos::new(5, 7))
}

fn world() -> World {
    World::new(3, map())
}

/// The screen position of a world point, for this camera.
fn at(world_pos: Vec2) -> Vec2 {
    camera().world_to_screen(world_pos)
}

/// An otherwise empty frame of input with the cursor somewhere.
fn at_cursor(cursor: Vec2) -> InputState {
    let mut input = InputState::default();
    input.cursor = cursor;
    input
}

/// A click: press on one frame, release on the next.
///
/// Two frames rather than one, because that is what a real mouse produces and
/// because the press and the release mean different things — the press opens a
/// drag, the release resolves it.
fn click(session: &mut Session, world: &World, button: MouseButton, cursor: Vec2) -> Vec<Command> {
    let mut press = at_cursor(cursor);
    press.press(button);
    let mut commands = session.update(&press, 0.0, world.view());

    let mut release = at_cursor(cursor);
    release.hold(button);
    release.release(button);
    commands.extend(session.update(&release, 0.0, world.view()));
    commands
}

fn units_of(world: &World, owner: PlayerId) -> Vec<UnitId> {
    world
        .view()
        .units()
        .filter(|(_, u)| u.owner == owner)
        .map(|(id, _)| id)
        .collect()
}

/// Selects every unit the active player owns by dragging a box over the map.
fn select_all(session: &mut Session, world: &World) {
    let mut press = at_cursor(at(Vec2::new(0.0, 0.0)));
    press.press(MouseButton::Left);
    session.update(&press, 0.0, world.view());

    let mut release = at_cursor(at(Vec2::new(16.0, 16.0)));
    release.hold(MouseButton::Left);
    release.release(MouseButton::Left);
    session.update(&release, 0.0, world.view());
}

// --- selection ------------------------------------------------------------

#[test]
fn clicking_a_unit_selects_it() {
    let world = world();
    let mut session = session();
    let expected = units_of(&world, PlayerId::ONE)[0];

    click(
        &mut session,
        &world,
        MouseButton::Left,
        at(Vec2::new(5.5, 5.5)),
    );

    assert_eq!(session.selection(), [expected]);
}

#[test]
fn clicking_empty_ground_clears_the_selection() {
    let world = world();
    let mut session = session();
    select_all(&mut session, &world);
    assert!(!session.selection().is_empty());

    click(
        &mut session,
        &world,
        MouseButton::Left,
        at(Vec2::new(2.5, 9.5)),
    );

    assert!(session.selection().is_empty());
}

#[test]
fn a_box_selects_only_the_active_players_units() {
    let world = world();
    let mut session = session();

    select_all(&mut session, &world);

    let mine = units_of(&world, PlayerId::ONE);
    let mut selected = session.selection().to_vec();
    selected.sort();
    let mut expected = mine;
    expected.sort();
    assert_eq!(selected, expected);
}

#[test]
fn a_drag_in_progress_is_reported_so_it_can_be_drawn() {
    let world = world();
    let mut session = session();

    let mut press = at_cursor(at(Vec2::new(1.0, 1.0)));
    press.press(MouseButton::Left);
    session.update(&press, 0.0, world.view());

    let corners = session.drag_box(Vec2::new(4.0, 3.0));
    assert_eq!(corners, Some((Vec2::new(1.0, 1.0), Vec2::new(4.0, 3.0))));
}

#[test]
fn a_dead_unit_falls_out_of_the_selection() {
    let mut world = world();
    let mut session = session();
    select_all(&mut session, &world);
    let before = session.selection().len();

    let villager = world
        .view()
        .units()
        .find(|(_, u)| u.owner == PlayerId::ONE && u.kind == UnitKind::VILLAGER)
        .map(|(id, _)| id)
        .expect("player one has a villager");
    let enemy = world
        .view()
        .units()
        .find(|(_, u)| u.owner == PlayerId::TWO && u.kind == UnitKind::SOLDIER)
        .map(|(id, _)| id)
        .expect("player two has a soldier");
    world.step(&[Command::Attack {
        player: PlayerId::TWO,
        units: [enemy].into_iter().collect(),
        target: EntityId::Unit(villager),
    }]);
    // Forty hit points at eight damage a second, plus the walk into range.
    for _ in 0..300 {
        world.step(&[]);
    }
    assert!(
        world.view().unit(villager).is_none(),
        "the villager survived"
    );

    session.update(&InputState::default(), 0.0, world.view());

    assert_eq!(session.selection().len(), before - 1);
    assert!(!session.selection().contains(&villager));
}

// --- orders ---------------------------------------------------------------

#[test]
fn right_clicking_open_ground_orders_a_move_there() {
    let world = world();
    let mut session = session();
    select_all(&mut session, &world);

    let commands = click(
        &mut session,
        &world,
        MouseButton::Right,
        at(Vec2::new(10.5, 13.5)),
    );

    match commands.as_slice() {
        [Command::Move { player, target, .. }] => {
            assert_eq!(*player, PlayerId::ONE);
            assert_eq!(*target, TilePos::new(10, 13));
        }
        other => panic!("expected one move order, got {other:?}"),
    }
}

#[test]
fn right_clicking_a_mine_orders_the_villagers_to_gather() {
    let world = world();
    let mut session = session();
    select_all(&mut session, &world);

    let commands = click(
        &mut session,
        &world,
        MouseButton::Right,
        at(Vec2::new(6.5, 12.5)),
    );

    match commands.as_slice() {
        [Command::Gather { player, units, .. }] => {
            assert_eq!(*player, PlayerId::ONE);
            // The soldier in the selection is not a gatherer and is dropped.
            assert_eq!(units.len(), 1);
        }
        other => panic!("expected one gather order, got {other:?}"),
    }
}

#[test]
fn right_clicking_a_mine_with_no_villagers_selected_is_a_move_order() {
    let world = world();
    let mut session = session();
    let soldier = world
        .view()
        .units()
        .find(|(_, u)| u.kind == UnitKind::SOLDIER)
        .map(|(id, _)| id)
        .expect("the soldier is on the map");
    click(
        &mut session,
        &world,
        MouseButton::Left,
        at(Vec2::new(6.5, 5.5)),
    );
    assert_eq!(session.selection(), [soldier]);

    let commands = click(
        &mut session,
        &world,
        MouseButton::Right,
        at(Vec2::new(6.5, 12.5)),
    );

    assert!(
        matches!(commands.as_slice(), [Command::Move { .. }]),
        "got {commands:?}"
    );
}

#[test]
fn right_clicking_an_enemy_unit_orders_an_attack() {
    let world = world();
    let mut session = session();
    select_all(&mut session, &world);

    let commands = click(
        &mut session,
        &world,
        MouseButton::Right,
        at(Vec2::new(9.5, 9.5)),
    );

    assert!(
        matches!(
            commands.as_slice(),
            [Command::Attack {
                target: EntityId::Unit(_),
                ..
            }]
        ),
        "got {commands:?}"
    );
}

#[test]
fn right_clicking_an_enemy_building_orders_an_attack() {
    let world = world();
    let mut session = session();
    select_all(&mut session, &world);

    let commands = click(
        &mut session,
        &world,
        MouseButton::Right,
        at(Vec2::new(13.5, 1.5)),
    );

    assert!(
        matches!(
            commands.as_slice(),
            [Command::Attack {
                target: EntityId::Building(_),
                ..
            }]
        ),
        "got {commands:?}"
    );
}

#[test]
fn right_clicking_your_own_building_is_a_move_order_not_an_attack() {
    let world = world();
    let mut session = session();
    select_all(&mut session, &world);

    let commands = click(
        &mut session,
        &world,
        MouseButton::Right,
        at(Vec2::new(1.5, 1.5)),
    );

    assert!(
        matches!(commands.as_slice(), [Command::Move { .. }]),
        "got {commands:?}"
    );
}

#[test]
fn right_clicking_with_nothing_selected_orders_nothing() {
    let world = world();
    let mut session = session();

    let commands = click(
        &mut session,
        &world,
        MouseButton::Right,
        at(Vec2::new(10.5, 13.5)),
    );

    assert!(commands.is_empty(), "got {commands:?}");
}

// --- training -------------------------------------------------------------

// --- hotseat --------------------------------------------------------------

#[test]
fn tab_cycles_through_every_player_and_back_to_the_first() {
    let map = map().with_players(3).with_building(
        BuildingKind::TOWN_CENTER,
        PlayerId::new(3),
        TilePos::new(12, 12),
    );
    let world = World::new(3, map);
    let mut session = Session::new(camera(), 16, 16, 3);
    let mut tab = InputState::default();
    tab.press_key(Key::Tab);

    let mut seen = vec![session.active_player()];
    for _ in 0..3 {
        session.update(&tab, 0.0, world.view());
        seen.push(session.active_player());
    }

    assert_eq!(
        seen,
        [
            PlayerId::ONE,
            PlayerId::TWO,
            PlayerId::new(3),
            PlayerId::ONE
        ]
    );
}

#[test]
fn tab_hands_the_keyboard_to_the_other_player() {
    let world = world();
    let mut session = session();
    assert_eq!(session.active_player(), PlayerId::ONE);

    let mut input = InputState::default();
    input.press_key(Key::Tab);
    session.update(&input, 0.0, world.view());

    assert_eq!(session.active_player(), PlayerId::TWO);
}

#[test]
fn each_player_keeps_their_own_selection_across_a_swap() {
    let world = world();
    let mut session = session();
    select_all(&mut session, &world);
    let first = session.selection().to_vec();

    let mut tab = InputState::default();
    tab.press_key(Key::Tab);
    session.update(&tab, 0.0, world.view());
    assert!(
        session.selection().is_empty(),
        "player two selected nothing yet"
    );
    click(
        &mut session,
        &world,
        MouseButton::Left,
        at(Vec2::new(9.5, 9.5)),
    );
    let second = session.selection().to_vec();

    session.update(&tab, 0.0, world.view());
    assert_eq!(session.selection(), first.as_slice());
    assert_ne!(first, second);
}

#[test]
fn each_player_keeps_their_own_camera() {
    let world = world();
    let mut session = session();

    // The mouse at the right edge of the window pans the view.
    let mut pan = InputState::default();
    pan.cursor = Vec2::new(
        session.camera().viewport.x - 1.0,
        session.camera().viewport.y / 2.0,
    );
    pan.cursor_seen = true;
    session.update(&pan, 0.5, world.view());
    let moved = session.camera().center;

    let mut tab = InputState::default();
    tab.press_key(Key::Tab);
    session.update(&tab, 0.0, world.view());

    assert_ne!(session.camera().center, moved);
}

#[test]
fn orders_are_stamped_with_whoever_holds_the_keyboard() {
    let world = world();
    let mut session = session();
    let mut tab = InputState::default();
    tab.press_key(Key::Tab);
    session.update(&tab, 0.0, world.view());

    click(
        &mut session,
        &world,
        MouseButton::Left,
        at(Vec2::new(9.5, 9.5)),
    );
    let commands = click(
        &mut session,
        &world,
        MouseButton::Right,
        at(Vec2::new(10.5, 8.5)),
    );

    match commands.as_slice() {
        [Command::Move { player, .. }] => assert_eq!(*player, PlayerId::TWO),
        other => panic!("expected a move order for player two, got {other:?}"),
    }
}

#[test]
fn a_half_finished_drag_does_not_survive_a_swap() {
    let world = world();
    let mut session = session();
    let mut press = at_cursor(at(Vec2::new(1.0, 1.0)));
    press.press(MouseButton::Left);
    session.update(&press, 0.0, world.view());

    let mut tab = InputState::default();
    tab.press_key(Key::Tab);
    session.update(&tab, 0.0, world.view());
    session.update(&tab, 0.0, world.view());

    assert_eq!(session.drag_box(Vec2::new(4.0, 4.0)), None);
}

// --- mouse lock ---------------------------------------------------------------

/// A frame with the cursor resting on the right edge of the window.
fn at_right_edge(session: &Session) -> InputState {
    let mut input = InputState::default();
    let viewport = session.camera().viewport;
    input.cursor = Vec2::new(viewport.x - 1.0, viewport.y / 2.0);
    input.cursor_seen = true;
    input
}

fn press(key: Key) -> InputState {
    let mut input = InputState::default();
    input.press_key(key);
    input
}

#[test]
fn the_mouse_starts_free_to_leave_the_window() {
    assert!(!session().mouse_locked());
}

#[test]
fn f10_locks_the_mouse_and_f10_again_frees_it() {
    let world = world();
    let mut session = session();

    session.update(&press(Key::F10), 0.0, world.view());
    assert!(session.mouse_locked());

    session.update(&press(Key::F10), 0.0, world.view());
    assert!(!session.mouse_locked());
}

#[test]
fn the_edges_scroll_whether_or_not_the_mouse_is_locked() {
    for presses in [0, 1] {
        let world = world();
        let mut session = session();
        for _ in 0..presses {
            session.update(&press(Key::F10), 0.0, world.view());
        }
        let before = session.camera().center;

        session.update(&at_right_edge(&session), 0.5, world.view());

        assert_ne!(session.camera().center, before, "locked: {}", presses == 1);
    }
}

// --- selecting at any zoom ------------------------------------------------------

/// The real villager's body: 21x42 sprite pixels standing on (10, 40), at 48
/// sprite pixels per tile.
fn villager_bodies() -> app::Bodies {
    let body = app::Body {
        offset: Vec2::new(-10.0 / 48.0, -40.0 / 48.0),
        size: Vec2::new(21.0 / 48.0, 42.0 / 48.0),
    };
    app::Bodies::all(body)
}

/// Presses the left button over world point `from` and releases it over
/// `to`, at `zoom` pixels per tile, and returns how many units ended up
/// selected.
fn click_and_release(zoom: f32, from: Vec2, to: Vec2) -> usize {
    let world = world();
    // A window exactly the 16x16 map's size at this zoom, so that the
    // controller keeps the zoom asked for: a larger window would make it zoom
    // in until the map filled it.
    let camera = Camera::new(
        Vec2::new(8.0, 8.0),
        zoom,
        Vec2::new(16.0 * zoom, 16.0 * zoom),
    );
    let mut session = Session::new(camera, 16, 16, 2);
    session.set_bodies(villager_bodies());
    let screen = |w: Vec2| camera.world_to_screen(w);

    let mut down = InputState::default();
    down.cursor = screen(from);
    down.press(MouseButton::Left);
    session.update(&down, 0.0, world.view());
    let mut up = InputState::default();
    up.cursor = screen(to);
    up.release(MouseButton::Left);
    session.update(&up, 0.0, world.view());
    session.selection().len()
}

#[test]
fn a_click_whose_hand_wobbles_two_pixels_selects_at_every_zoom() {
    // Player one's villager stands at (5.5, 5.5). Zoomed out it is drawn a
    // few pixels wide; a 2-pixel wobble must not miss it.
    let feet = Vec2::new(5.5, 5.5);
    for zoom in [4.0, 8.0, 16.0, 48.0] {
        let wobble = Vec2::new(2.0 / zoom, 0.0);

        assert_eq!(
            click_and_release(zoom, feet, feet + wobble),
            1,
            "zoom {zoom}"
        );
    }
}

#[test]
fn zoomed_out_a_unit_can_be_clicked_a_few_pixels_beside_its_tiny_sprite() {
    // At 4 pixels per tile the villager is under 2 pixels wide; a click 8
    // pixels to its side still selects it.
    let beside = Vec2::new(5.5 + 8.0 / 4.0, 5.2);

    assert_eq!(click_and_release(4.0, beside, beside), 1);
}

#[test]
fn zoomed_in_a_click_beside_the_sprite_still_misses() {
    // At 48 pixels per tile the sprite itself is the target; 30 pixels to
    // its side is empty ground.
    let beside = Vec2::new(5.5 + 30.0 / 48.0, 5.2);

    assert_eq!(click_and_release(48.0, beside, beside), 0);
}

#[test]
fn a_click_on_a_units_drawn_body_selects_it() {
    let world = world();
    let mut session = session();
    session.set_bodies(app::Bodies::all(app::Body {
        offset: Vec2::new(-0.25, -1.0),
        size: Vec2::new(0.5, 1.0),
    }));
    // Player one's villager stands at (5.5, 5.5); click 0.8 tiles above.
    let mut click = at_cursor(at(Vec2::new(5.5, 4.7)));
    click.press(MouseButton::Left);
    session.update(&click, 0.0, world.view());
    let mut release = at_cursor(at(Vec2::new(5.5, 4.7)));
    release.release(MouseButton::Left);
    session.update(&release, 0.0, world.view());

    assert_eq!(session.selection().len(), 1);
}

// --- the interface ----------------------------------------------------------

/// A point on the command panel, for this camera's window.
fn on_command_panel() -> Vec2 {
    let panel = app::Layout::new(camera().viewport).command_panel;
    panel.min + panel.size * 0.5
}

#[test]
fn a_left_click_on_the_command_panel_keeps_the_selection() {
    let world = world();
    let mut session = session();
    select_all(&mut session, &world);
    let selected = session.selection().to_vec();
    assert!(!selected.is_empty());

    click(&mut session, &world, MouseButton::Left, on_command_panel());

    assert_eq!(session.selection(), selected);
}

#[test]
fn a_right_click_on_the_command_panel_orders_nothing() {
    let world = world();
    let mut session = session();
    select_all(&mut session, &world);

    let commands = click(&mut session, &world, MouseButton::Right, on_command_panel());

    assert_eq!(commands, []);
}

#[test]
fn a_drag_started_on_the_interface_selects_nothing() {
    let world = world();
    let mut session = session();
    select_all(&mut session, &world);
    let selected = session.selection().to_vec();

    let mut press = at_cursor(on_command_panel());
    press.press(MouseButton::Left);
    session.update(&press, 0.0, world.view());
    assert_eq!(session.drag_box(Vec2::ZERO), None);
    let mut release = at_cursor(at(Vec2::new(5.0, 5.0)));
    release.hold(MouseButton::Left);
    release.release(MouseButton::Left);
    session.update(&release, 0.0, world.view());

    assert_eq!(session.selection(), selected);
}

// --- selecting buildings -------------------------------------------------------

/// The building of `kind` that `owner` has on the test map.
fn building_of(world: &World, owner: PlayerId, kind: BuildingKind) -> sim::BuildingId {
    world
        .view()
        .buildings()
        .find(|(_, b)| b.owner == owner && b.kind == kind)
        .map(|(id, _)| id)
        .expect("the building is on the map")
}

/// Player one's barracks covers tiles (0, 12) to (2, 14); this is its middle.
const BARRACKS_MIDDLE: Vec2 = Vec2::new(1.5, 13.5);

#[test]
fn clicking_your_building_selects_it_and_drops_the_selected_units() {
    let world = world();
    let mut session = session();
    select_all(&mut session, &world);
    assert!(!session.selection().is_empty());

    click(&mut session, &world, MouseButton::Left, at(BARRACKS_MIDDLE));

    let barracks = building_of(&world, PlayerId::ONE, BuildingKind::BARRACKS);
    assert_eq!(session.selected_building(), Some(barracks));
    assert_eq!(session.selection(), []);
}

#[test]
fn clicking_empty_ground_drops_the_selected_building() {
    let world = world();
    let mut session = session();
    click(&mut session, &world, MouseButton::Left, at(BARRACKS_MIDDLE));

    click(
        &mut session,
        &world,
        MouseButton::Left,
        at(Vec2::new(8.5, 6.5)),
    );

    assert_eq!(session.selected_building(), None);
}

#[test]
fn clicking_a_unit_drops_the_selected_building() {
    let world = world();
    let mut session = session();
    click(&mut session, &world, MouseButton::Left, at(BARRACKS_MIDDLE));

    click(
        &mut session,
        &world,
        MouseButton::Left,
        at(Vec2::new(5.5, 5.5)),
    );

    assert_eq!(session.selected_building(), None);
    assert_eq!(session.selection().len(), 1);
}

#[test]
fn a_unit_standing_in_front_of_a_building_is_picked_before_it() {
    let world = World::new(
        3,
        map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(1, 13)),
    );
    let mut session = session();

    click(&mut session, &world, MouseButton::Left, at(BARRACKS_MIDDLE));

    assert_eq!(session.selected_building(), None);
    assert_eq!(session.selection().len(), 1);
}

#[test]
fn clicking_a_buildings_roof_above_its_footprint_selects_it() {
    // The barracks drawn 1.5 tiles taller than its footprint: its body runs
    // from y 10.5 down to the footprint's bottom edge at 15.
    let world = world();
    let mut session = session();
    let barracks_body = app::Body {
        offset: Vec2::new(-1.5, -3.0),
        size: Vec2::new(3.0, 4.5),
    };
    session.set_bodies(app::Bodies {
        buildings: vec![barracks_body, barracks_body],
        ..app::Bodies::default()
    });

    click(
        &mut session,
        &world,
        MouseButton::Left,
        at(Vec2::new(1.5, 11.0)),
    );

    let barracks = building_of(&world, PlayerId::ONE, BuildingKind::BARRACKS);
    assert_eq!(session.selected_building(), Some(barracks));
}

#[test]
fn an_enemy_building_can_be_selected_to_look_at() {
    let world = world();
    let mut session = session();

    click(
        &mut session,
        &world,
        MouseButton::Left,
        at(Vec2::new(14.0, 2.0)),
    );

    let theirs = building_of(&world, PlayerId::TWO, BuildingKind::TOWN_CENTER);
    assert_eq!(session.selected_building(), Some(theirs));
}

#[test]
fn a_drag_box_drops_the_selected_building() {
    let world = world();
    let mut session = session();
    click(&mut session, &world, MouseButton::Left, at(BARRACKS_MIDDLE));

    select_all(&mut session, &world);

    assert_eq!(session.selected_building(), None);
    assert!(!session.selection().is_empty());
}

#[test]
fn a_destroyed_building_falls_out_of_the_selection() {
    // Player two's soldiers knock down player one's barracks.
    let map = MapSpec::new(16, 16)
        .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(0, 0))
        .with_building(BuildingKind::BARRACKS, PlayerId::ONE, TilePos::new(0, 12))
        .with_building(
            BuildingKind::TOWN_CENTER,
            PlayerId::TWO,
            TilePos::new(12, 0),
        )
        .with_unit(UnitKind::SOLDIER, PlayerId::TWO, TilePos::new(3, 13));
    let mut content = sim::Content::default();
    content.building_mut(BuildingKind::BARRACKS).max_hp = 1;
    let mut world = World::with_content(3, map, content);
    let mut session = session();
    click(&mut session, &world, MouseButton::Left, at(BARRACKS_MIDDLE));
    let barracks = building_of(&world, PlayerId::ONE, BuildingKind::BARRACKS);
    assert_eq!(session.selected_building(), Some(barracks));

    let (soldier, _) = world
        .view()
        .units()
        .find(|(_, u)| u.owner == PlayerId::TWO)
        .expect("the soldier");
    world.step(&[Command::Attack {
        player: PlayerId::TWO,
        units: [soldier].into_iter().collect(),
        target: EntityId::Building(barracks),
    }]);
    for _ in 0..200 {
        world.step(&[]);
    }
    assert!(
        world.view().building(barracks).is_none(),
        "the barracks stands"
    );
    session.update(&InputState::default(), 0.0, world.view());

    assert_eq!(session.selected_building(), None);
}

// --- the command panel ------------------------------------------------------------

/// The middle of command button `index`, for this camera's window.
fn button(index: usize) -> Vec2 {
    let rect = app::Layout::new(camera().viewport).buttons[index];
    rect.min + rect.size * 0.5
}

/// The one train order in `commands`, as (player, building, unit).
fn train_order(commands: &[Command]) -> (PlayerId, sim::BuildingId, UnitKind) {
    match commands {
        [Command::Train {
            player,
            building,
            unit,
        }] => (*player, *building, *unit),
        other => panic!("expected one train order, got {other:?}"),
    }
}

/// Player one's town centre covers tiles (0, 0) to (3, 3).
const TOWN_CENTRE_MIDDLE: Vec2 = Vec2::new(2.0, 2.0);

#[test]
fn the_first_button_of_your_barracks_trains_a_soldier_there() {
    let world = world();
    let mut session = session();
    click(&mut session, &world, MouseButton::Left, at(BARRACKS_MIDDLE));

    let commands = click(&mut session, &world, MouseButton::Left, button(0));

    let barracks = building_of(&world, PlayerId::ONE, BuildingKind::BARRACKS);
    assert_eq!(
        train_order(&commands),
        (PlayerId::ONE, barracks, UnitKind::SOLDIER)
    );
}

#[test]
fn the_first_button_of_your_town_centre_trains_a_villager_there() {
    let world = world();
    let mut session = session();
    click(
        &mut session,
        &world,
        MouseButton::Left,
        at(TOWN_CENTRE_MIDDLE),
    );

    let commands = click(&mut session, &world, MouseButton::Left, button(0));

    let town_centre = building_of(&world, PlayerId::ONE, BuildingKind::TOWN_CENTER);
    assert_eq!(
        train_order(&commands),
        (PlayerId::ONE, town_centre, UnitKind::VILLAGER)
    );
}

#[test]
fn pressing_a_button_keeps_the_building_selected() {
    let world = world();
    let mut session = session();
    click(&mut session, &world, MouseButton::Left, at(BARRACKS_MIDDLE));

    click(&mut session, &world, MouseButton::Left, button(0));

    let barracks = building_of(&world, PlayerId::ONE, BuildingKind::BARRACKS);
    assert_eq!(session.selected_building(), Some(barracks));
}

#[test]
fn a_button_with_nothing_on_it_does_nothing() {
    // The barracks trains one type, so only the first button has a unit.
    let world = world();
    let mut session = session();
    click(&mut session, &world, MouseButton::Left, at(BARRACKS_MIDDLE));

    let commands = click(&mut session, &world, MouseButton::Left, button(1));

    assert_eq!(commands, []);
}

#[test]
fn another_players_building_has_no_buttons_to_press() {
    let world = world();
    let mut session = session();
    click(
        &mut session,
        &world,
        MouseButton::Left,
        at(Vec2::new(14.0, 2.0)),
    );

    let mut commands = click(&mut session, &world, MouseButton::Left, button(0));
    commands.extend(session.update(&press(Key::Digit1), 0.0, world.view()));

    assert_eq!(commands, []);
}

#[test]
fn key_one_presses_the_first_button_of_the_selected_building() {
    let world = world();
    let mut session = session();
    click(&mut session, &world, MouseButton::Left, at(BARRACKS_MIDDLE));

    let commands = session.update(&press(Key::Digit1), 0.0, world.view());

    let barracks = building_of(&world, PlayerId::ONE, BuildingKind::BARRACKS);
    assert_eq!(
        train_order(&commands),
        (PlayerId::ONE, barracks, UnitKind::SOLDIER)
    );
}

#[test]
fn with_nothing_selected_the_number_keys_do_nothing() {
    let world = world();
    let mut session = session();

    let mut commands = Vec::new();
    for key in [Key::Digit1, Key::Digit2, Key::Digit3] {
        commands.extend(session.update(&press(key), 0.0, world.view()));
    }

    assert_eq!(commands, []);
}

#[test]
fn with_units_selected_the_number_keys_do_nothing() {
    let world = world();
    let mut session = session();
    select_all(&mut session, &world);

    let commands = session.update(&press(Key::Digit1), 0.0, world.view());

    assert_eq!(commands, []);
}

#[test]
fn a_second_barracks_trains_when_it_is_the_one_selected() {
    let map = map().with_building(BuildingKind::BARRACKS, PlayerId::ONE, TilePos::new(10, 12));
    let world = World::new(3, map);
    let mut session = session();
    let second = world
        .view()
        .buildings()
        .find(|(_, b)| b.kind == BuildingKind::BARRACKS && b.origin == TilePos::new(10, 12))
        .map(|(id, _)| id)
        .expect("the second barracks");
    click(
        &mut session,
        &world,
        MouseButton::Left,
        at(Vec2::new(11.5, 13.5)),
    );

    let commands = session.update(&press(Key::Digit1), 0.0, world.view());

    assert_eq!(train_order(&commands).1, second);
}

#[test]
fn a_type_added_in_content_gets_the_next_button_and_key() {
    // The Phase 1.5 gate scenario: an archer added only in content, trained
    // at the barracks after the soldier, so it is the second button and key
    // 2, with no code naming it.
    let mut content = sim::Content::default();
    let mut archer = content.unit(UnitKind::SOLDIER).clone();
    archer.name = "archer".to_string();
    content.units.push(archer);
    let archer = content.unit_named("archer").expect("added");
    content
        .building_mut(BuildingKind::BARRACKS)
        .trains
        .push(archer);
    let world = World::with_content(3, map(), content);
    let mut session = session();
    click(&mut session, &world, MouseButton::Left, at(BARRACKS_MIDDLE));

    let by_click = click(&mut session, &world, MouseButton::Left, button(1));
    let by_key = session.update(&press(Key::Digit2), 0.0, world.view());

    assert_eq!(train_order(&by_click).2, archer);
    assert_eq!(train_order(&by_key).2, archer);
}

#[test]
fn a_drag_into_the_panels_selects_only_what_shows_above_them() {
    // A villager at (6.5, 11.5) stands under the bottom panels in this
    // window; the two units at y 5.5 are in the open middle.
    let world = World::new(
        3,
        map().with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(6, 11)),
    );
    let mut session = session();
    let hidden = world
        .view()
        .units()
        .find(|(_, u)| u.owner == PlayerId::ONE && u.pos.y.to_bits() > 11 << 16)
        .map(|(id, _)| id)
        .expect("the hidden villager");
    let end = at(Vec2::new(7.5, 11.5));
    assert!(app::Layout::new(camera().viewport).bottom.contains(end));

    let mut press = at_cursor(at(Vec2::new(4.0, 4.6)));
    press.press(MouseButton::Left);
    session.update(&press, 0.0, world.view());
    let mut release = at_cursor(end);
    release.hold(MouseButton::Left);
    release.release(MouseButton::Left);
    session.update(&release, 0.0, world.view());

    assert_eq!(session.selection().len(), 2, "{:?}", session.selection());
    assert!(!session.selection().contains(&hidden));
}

// --- dropping off by hand ------------------------------------------------------------

#[test]
fn right_clicking_your_drop_off_with_a_loaded_villager_drops_the_load_off() {
    // A second mine at (8, 5), in the open middle of this window, so the
    // villager mining it can be clicked; its town centre covers (0, 0) to
    // (3, 3).
    let mut world = World::new(3, map().with_mine(TilePos::new(8, 5), 500));
    let villager = units_of(&world, PlayerId::ONE)
        .into_iter()
        .find(|id| {
            world
                .view()
                .unit(*id)
                .is_some_and(|u| u.kind == UnitKind::VILLAGER)
        })
        .expect("the villager");
    let (mine, _) = world
        .view()
        .sites()
        .find(|(_, m)| m.origin == TilePos::new(8, 5))
        .expect("the near mine");
    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: [villager].into_iter().collect(),
        site: mine,
    }]);
    for _ in 0..2000 {
        if world.view().unit(villager).is_some_and(|u| u.carrying > 0) {
            break;
        }
        world.step(&[]);
    }
    assert!(world.view().unit(villager).is_some_and(|u| u.carrying > 0));
    let mut session = session();
    let feet = app::to_screen_space(world.view().unit(villager).expect("the villager").pos);
    click(&mut session, &world, MouseButton::Left, at(feet));
    assert_eq!(session.selection(), [villager]);
    let town_centre = building_of(&world, PlayerId::ONE, BuildingKind::TOWN_CENTER);

    let commands = click(
        &mut session,
        &world,
        MouseButton::Right,
        at(TOWN_CENTRE_MIDDLE),
    );

    match commands.as_slice() {
        [Command::Deposit {
            player,
            units,
            building,
        }] => {
            assert_eq!((*player, *building), (PlayerId::ONE, town_centre));
            assert_eq!(units.as_slice(), [villager], "only the loaded villager");
        }
        other => panic!("expected a drop-off, got {other:?}"),
    }
}

#[test]
fn right_clicking_a_building_that_does_not_take_the_load_is_no_drop_off() {
    // The barracks takes nothing: a loaded villager is told to walk there.
    let mut world = World::new(3, map().with_mine(TilePos::new(8, 5), 500));
    let villager = units_of(&world, PlayerId::ONE)
        .into_iter()
        .find(|id| {
            world
                .view()
                .unit(*id)
                .is_some_and(|u| u.kind == UnitKind::VILLAGER)
        })
        .expect("the villager");
    let (site, _) = world
        .view()
        .sites()
        .find(|(_, m)| m.origin == TilePos::new(8, 5))
        .expect("the near mine");
    world.step(&[Command::Gather {
        player: PlayerId::ONE,
        units: [villager].into_iter().collect(),
        site,
    }]);
    while world.view().unit(villager).is_some_and(|u| u.carrying == 0) {
        world.step(&[]);
    }
    let mut session = session();
    let feet = app::to_screen_space(world.view().unit(villager).expect("the villager").pos);
    click(&mut session, &world, MouseButton::Left, at(feet));

    let commands = click(
        &mut session,
        &world,
        MouseButton::Right,
        at(BARRACKS_MIDDLE),
    );

    assert!(
        matches!(commands.as_slice(), [Command::Move { .. }]),
        "{commands:?}"
    );
}
