//! Selection, hotseat switching, and the input-to-command mapping.
//!
//! Input is synthesised as a plain `InputState`, so all of this is testable
//! without a window. That is the reason `InputSource` exists as a trait.

use app::Session;
use render::{Camera, InputState, Key, MouseButton, Vec2};
use sim::{BuildingKind, Command, EntityId, MapSpec, PlayerId, TilePos, UnitId, UnitKind, World};

/// A camera showing the whole 16x16 map.
///
/// The zoom is [`CameraController::MIN_ZOOM`] because the controller clamps to
/// that on the first frame; picking anything below it would make the camera the
/// tests convert with differ from the one the session actually uses.
fn camera() -> Camera {
    Camera::new(
        Vec2::new(8.0, 8.0),
        app::CameraController::MIN_ZOOM,
        Vec2::new(64.0, 64.0),
    )
}

fn session() -> Session {
    Session::new(camera(), 16, 16, 2)
}

/// A map with two villagers for player one and one for player two, plus a
/// building each so that neither side has already lost.
fn map() -> MapSpec {
    MapSpec::new(16, 16)
        .with_starting_gold(200)
        .with_building(BuildingKind::TownCenter, PlayerId::ONE, TilePos::new(0, 0))
        .with_building(BuildingKind::Barracks, PlayerId::ONE, TilePos::new(0, 12))
        .with_building(BuildingKind::TownCenter, PlayerId::TWO, TilePos::new(12, 0))
        .with_mine(TilePos::new(6, 12), 5000)
        .with_unit(UnitKind::Villager, PlayerId::ONE, TilePos::new(5, 5))
        .with_unit(UnitKind::Soldier, PlayerId::ONE, TilePos::new(6, 5))
        .with_unit(UnitKind::Villager, PlayerId::TWO, TilePos::new(9, 9))
        .with_unit(UnitKind::Soldier, PlayerId::TWO, TilePos::new(5, 7))
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
        .find(|(_, u)| u.owner == PlayerId::ONE && u.kind == UnitKind::Villager)
        .map(|(id, _)| id)
        .expect("player one has a villager");
    let enemy = world
        .view()
        .units()
        .find(|(_, u)| u.owner == PlayerId::TWO && u.kind == UnitKind::Soldier)
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
        .find(|(_, u)| u.kind == UnitKind::Soldier)
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

#[test]
fn the_first_hotkey_trains_a_villager_at_the_town_centre() {
    let world = world();
    let mut session = session();
    let mut input = InputState::default();
    input.press_key(Key::Digit1);

    let commands = session.update(&input, 0.0, world.view());

    match commands.as_slice() {
        [Command::Train {
            player,
            unit,
            building,
        }] => {
            assert_eq!(*player, PlayerId::ONE);
            assert_eq!(*unit, UnitKind::Villager);
            assert_eq!(
                world.view().building(*building).map(|b| b.kind),
                Some(BuildingKind::TownCenter)
            );
        }
        other => panic!("expected one train order, got {other:?}"),
    }
}

#[test]
fn the_second_hotkey_trains_a_soldier_at_the_barracks() {
    let world = world();
    let mut session = session();
    let mut input = InputState::default();
    input.press_key(Key::Digit2);

    let commands = session.update(&input, 0.0, world.view());

    match commands.as_slice() {
        [Command::Train { unit, building, .. }] => {
            assert_eq!(*unit, UnitKind::Soldier);
            assert_eq!(
                world.view().building(*building).map(|b| b.kind),
                Some(BuildingKind::Barracks)
            );
        }
        other => panic!("expected one train order, got {other:?}"),
    }
}

#[test]
fn a_hotkey_with_no_building_that_can_train_it_does_nothing() {
    // Player two has a town centre but no barracks.
    let world = world();
    let mut session = session();
    let mut tab = InputState::default();
    tab.press_key(Key::Tab);
    session.update(&tab, 0.0, world.view());

    let mut input = InputState::default();
    input.press_key(Key::Digit2);
    let commands = session.update(&input, 0.0, world.view());

    assert!(commands.is_empty(), "got {commands:?}");
}

// --- hotseat --------------------------------------------------------------

#[test]
fn tab_cycles_through_every_player_and_back_to_the_first() {
    let map = map().with_players(3).with_building(
        BuildingKind::TownCenter,
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
        at(Vec2::new(11.5, 11.5)),
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
    app::Bodies {
        villager: body,
        soldier: body,
    }
}

/// Presses the left button over world point `from` and releases it over
/// `to`, at `zoom` pixels per tile, and returns how many units ended up
/// selected.
fn click_and_release(zoom: f32, from: Vec2, to: Vec2) -> usize {
    let world = world();
    let camera = Camera::new(Vec2::new(8.0, 8.0), zoom, Vec2::new(800.0, 600.0));
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
    session.set_bodies(app::Bodies {
        villager: app::Body {
            offset: Vec2::new(-0.25, -1.0),
            size: Vec2::new(0.5, 1.0),
        },
        soldier: app::Body {
            offset: Vec2::new(-0.25, -1.0),
            size: Vec2::new(0.5, 1.0),
        },
    });
    // Player one's villager stands at (5.5, 5.5); click 0.8 tiles above.
    let mut click = at_cursor(at(Vec2::new(5.5, 4.7)));
    click.press(MouseButton::Left);
    session.update(&click, 0.0, world.view());
    let mut release = at_cursor(at(Vec2::new(5.5, 4.7)));
    release.release(MouseButton::Left);
    session.update(&release, 0.0, world.view());

    assert_eq!(session.selection().len(), 1);
}
