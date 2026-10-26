//! The mouse cursor: drawn by the game, over everything, as the original
//! draws its own, and turned into the job's cursor when villagers are
//! selected and the mouse is over something they would gather.

mod common;

use app::{draw_world, entity_at, to_screen_space, Hud, Layout, Session, Sprites};
use render::{Camera, DrawCall, RecordingRenderer, Vec2};
use sim::{
    BuildingKind, EntityId, FxVec2, MapSpec, PlayerId, Resource, TilePos, UnitId, UnitKind, World,
};
use std::collections::HashMap;

fn camera() -> Camera {
    Camera::new(Vec2::new(8.0, 8.0), 20.0, Vec2::new(800.0, 600.0))
}

/// A villager and a soldier of player one, and one site of each resource
/// other than food on a farm.
fn world() -> World {
    World::new(
        1,
        MapSpec::new(16, 16)
            .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(1, 1))
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(6, 6))
            .with_unit(UnitKind::SOLDIER, PlayerId::ONE, TilePos::new(7, 6))
            .with_site(Resource::Wood, TilePos::new(12, 2), 100)
            .with_site(Resource::Gold, TilePos::new(12, 6), 100)
            .with_site(Resource::Stone, TilePos::new(12, 10), 100)
            .with_site(Resource::Food, TilePos::new(12, 14), 100),
    )
}

fn unit(world: &World, kind: UnitKind) -> UnitId {
    world
        .view()
        .units()
        .find(|(_, u)| u.kind == kind)
        .map(|(id, _)| id)
        .expect("unit on the map")
}

/// Where a site of `resource` is drawn, so the mouse is over it.
fn over(world: &World, resource: Resource) -> Vec2 {
    let site = world
        .view()
        .sites()
        .find(|(_, s)| s.resource == resource)
        .map(|(_, s)| s.center())
        .expect("site on the map");
    to_screen_space(site)
}

/// Draws a frame with `selection` selected and the mouse at `mouse`, in
/// world units, and returns the last sprite drawn: its texture, corner, and
/// size.
fn last_sprite(
    renderer: &mut RecordingRenderer,
    sprites: &Sprites,
    world: &World,
    selection: &[UnitId],
    mouse: Vec2,
) -> (render::TextureId, Vec2, Vec2) {
    last_sprite_seen_by(renderer, &camera(), sprites, world, selection, mouse)
}

/// [`last_sprite`], seen through `camera`.
fn last_sprite_seen_by(
    renderer: &mut RecordingRenderer,
    camera: &Camera,
    sprites: &Sprites,
    world: &World,
    selection: &[UnitId],
    mouse: Vec2,
) -> (render::TextureId, Vec2, Vec2) {
    let hud = Hud {
        active_player: PlayerId::ONE,
        selection,
        building: None,
        drag_box: None,
        replaying: false,
        cursor_world: mouse,
    };
    let previous: HashMap<UnitId, FxVec2> = HashMap::new();
    draw_world(
        renderer,
        camera,
        world.view(),
        &previous,
        0.0,
        &hud,
        sprites,
    );
    renderer
        .calls()
        .iter()
        .rev()
        .find_map(|call| match call {
            DrawCall::Sprite { texture, pos, size } => Some((*texture, *pos, *size)),
            _ => None,
        })
        .expect("a sprite drawn")
}

fn close(a: Vec2, b: Vec2) -> bool {
    (a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4
}

#[test]
fn with_a_villager_selected_the_cursor_over_a_tree_is_the_axe_on_top_of_everything() {
    let mut renderer = RecordingRenderer::new();
    let sprites = common::sprites(&mut renderer);
    let world = world();
    let mouse = over(&world, Resource::Wood);

    let (texture, at, size) = last_sprite(
        &mut renderer,
        &sprites,
        &world,
        &[unit(&world, UnitKind::VILLAGER)],
        mouse,
    );

    let axe = sprites.cursor(Some(Resource::Wood));
    assert_eq!(texture, axe.texture, "not the axe");
    // Drawn at its own pixel size whatever the zoom, its hotspot on the mouse.
    let zoom = camera().zoom;
    assert!(close(at, mouse - axe.hotspot * (1.0 / zoom)), "at {at:?}");
    assert!(
        close(size, Vec2::new(axe.width, axe.height) * (1.0 / zoom)),
        "size {size:?}"
    );
}

#[test]
fn with_a_villager_selected_each_resource_shows_its_own_job_cursor() {
    for resource in [Resource::Gold, Resource::Stone, Resource::Wood] {
        let mut renderer = RecordingRenderer::new();
        let sprites = common::sprites(&mut renderer);
        let world = world();

        let (texture, _, _) = last_sprite(
            &mut renderer,
            &sprites,
            &world,
            &[unit(&world, UnitKind::VILLAGER)],
            over(&world, resource),
        );

        assert_eq!(
            texture,
            sprites.cursor(Some(resource)).texture,
            "{resource:?}"
        );
    }
}

#[test]
fn without_a_villager_selected_the_cursor_over_a_tree_is_the_plain_arrow() {
    for selection in [vec![], vec![unit(&world(), UnitKind::SOLDIER)]] {
        let mut renderer = RecordingRenderer::new();
        let sprites = common::sprites(&mut renderer);
        let world = world();
        let mouse = over(&world, Resource::Wood);

        let (texture, at, _) = last_sprite(&mut renderer, &sprites, &world, &selection, mouse);

        let arrow = sprites.cursor(None);
        assert_eq!(texture, arrow.texture, "selection {selection:?}");
        let zoom = camera().zoom;
        assert!(close(at, mouse - arrow.hotspot * (1.0 / zoom)), "at {at:?}");
    }
}

#[test]
fn with_a_villager_selected_the_cursor_over_open_ground_is_the_plain_arrow() {
    let mut renderer = RecordingRenderer::new();
    let sprites = common::sprites(&mut renderer);
    let world = world();

    let (texture, _, _) = last_sprite(
        &mut renderer,
        &sprites,
        &world,
        &[unit(&world, UnitKind::VILLAGER)],
        to_screen_space(FxVec2::tile_center(TilePos::new(4, 12))),
    );

    assert_eq!(texture, sprites.cursor(None).texture);
}

#[test]
fn the_job_cursor_shows_exactly_where_a_right_click_would_pick_the_site() {
    // A villager stands in front of a tree. The cursor must agree with what
    // a right-click there would order: the axe where the click picks the
    // tree, the arrow where it picks the villager, whose click target is
    // grown to the session's smallest at this zoom.
    let world = World::new(
        1,
        MapSpec::new(16, 16)
            .with_building(BuildingKind::TOWN_CENTER, PlayerId::ONE, TilePos::new(1, 1))
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(6, 6))
            .with_unit(UnitKind::VILLAGER, PlayerId::ONE, TilePos::new(12, 4))
            .with_site(Resource::Wood, TilePos::new(12, 3), 100),
    );
    let selected = unit(&world, UnitKind::VILLAGER);
    let tree = over(&world, Resource::Wood);
    let mut renderer = RecordingRenderer::new();
    let sprites = common::sprites(&mut renderer);
    let bodies = sprites.bodies(world.view().content());
    let min = Session::MIN_PICK_PIXELS / camera().zoom;

    let mut disagreements = Vec::new();
    for dy in -15..=15 {
        for dx in -15..=15 {
            let mouse = tree + Vec2::new(dx as f32, dy as f32) * 0.2;
            let picks_site = matches!(
                entity_at(world.view(), mouse, &bodies, min),
                Some(EntityId::Site(_))
            );
            let mut renderer = RecordingRenderer::new();
            let sprites = common::sprites(&mut renderer);
            let (texture, _, _) = last_sprite(&mut renderer, &sprites, &world, &[selected], mouse);
            let axe = texture == sprites.cursor(Some(Resource::Wood)).texture;
            if axe != picks_site {
                disagreements.push(mouse);
            }
        }
    }
    assert!(disagreements.is_empty(), "{disagreements:?}");
}

#[test]
fn over_the_interface_the_cursor_is_the_plain_arrow_even_above_a_tree() {
    // A right-click on the interface orders nothing on the map beneath it,
    // so the cursor must not offer the job there. The camera puts the tree
    // under the bottom panel.
    let world = world();
    let tree = over(&world, Resource::Wood);
    let viewport = Vec2::new(800.0, 600.0);
    let on_panel = Vec2::new(400.0, 580.0);
    assert!(Layout::new(viewport).blocks(on_panel), "not on the panel");
    let zoom = 20.0;
    let camera = Camera::new(
        tree - (on_panel - viewport * 0.5) * (1.0 / zoom),
        zoom,
        viewport,
    );
    let mut renderer = RecordingRenderer::new();
    let sprites = common::sprites(&mut renderer);

    let (texture, _, _) = last_sprite_seen_by(
        &mut renderer,
        &camera,
        &sprites,
        &world,
        &[unit(&world, UnitKind::VILLAGER)],
        tree,
    );

    assert_eq!(texture, sprites.cursor(None).texture);
}
