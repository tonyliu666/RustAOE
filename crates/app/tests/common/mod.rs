//! A fake sprite set for draw tests: real textures loaded into the recording
//! renderer, with made-up sizes and hotspots, so no game data is needed.

#![allow(dead_code)] // Each test binary uses a different part of this.

use app::{Sprite, Sprites};
use render::{RecordingRenderer, Renderer, Vec2};

/// A fake sprite: a texture of `size` pixels with its ground point at
/// `hotspot`.
pub fn sprite(renderer: &mut RecordingRenderer, size: (u32, u32), hotspot: (f32, f32)) -> Sprite {
    let texture = renderer.load_texture(size.0, size.1, &vec![0; (size.0 * size.1 * 4) as usize]);
    Sprite {
        texture,
        width: size.0 as f32,
        height: size.1 as f32,
        hotspot: Vec2::new(hotspot.0, hotspot.1),
    }
}

/// One fake sprite per kind and player, two grass variations, a tree, and a
/// gold mine.
pub fn sprites(renderer: &mut RecordingRenderer) -> Sprites {
    let mut per_player = |size, hotspot| std::array::from_fn(|_| sprite(renderer, size, hotspot));
    let villager = per_player((21, 42), (10.0, 40.0));
    let soldier = per_player((28, 47), (14.0, 44.0));
    let town_center = per_player((200, 240), (100.0, 150.0));
    let barracks = per_player((300, 260), (150.0, 160.0));
    let grass = vec![
        renderer.load_texture(4, 4, &[0; 64]),
        renderer.load_texture(4, 4, &[0; 64]),
    ];
    let tree = sprite(renderer, (110, 129), (55.0, 120.0));
    let gold_mine = sprite(renderer, (82, 40), (41.0, 22.0));
    Sprites {
        grass,
        tree,
        gold_mine,
        villager,
        soldier,
        town_center,
        barracks,
    }
}
