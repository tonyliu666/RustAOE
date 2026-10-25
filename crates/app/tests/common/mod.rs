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

/// One fake sprite per type of the default content and per player, two grass variations, a tree, and a
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
        units: vec![villager, soldier],
        buildings: vec![town_center, barracks],
    }
}

// --- a fake game install ------------------------------------------------------

use std::path::{Path, PathBuf};

/// A DRS archive holding `files` as `(type tag, id, bytes)`, one table per
/// distinct tag, in the game's layout.
pub fn drs(files: &[([u8; 4], u32, Vec<u8>)]) -> Vec<u8> {
    let mut tags: Vec<[u8; 4]> = Vec::new();
    for (tag, _, _) in files {
        if !tags.contains(tag) {
            tags.push(*tag);
        }
    }
    let mut header = b"Copyright (c) 1997 Ensemble Studios.\x1a".to_vec();
    header.resize(40, 0);
    header.extend(b"1.00tribe\0\0\0\0\0\0\0");
    let first_file = 64 + 12 * tags.len() + 12 * files.len();
    header.extend((tags.len() as u32).to_le_bytes());
    header.extend((first_file as u32).to_le_bytes());

    let (mut records, mut entries, mut data) = (Vec::new(), Vec::new(), Vec::new());
    let mut entry_at = 64 + 12 * tags.len();
    for tag in &tags {
        let mine: Vec<_> = files.iter().filter(|(t, _, _)| t == tag).collect();
        records.extend(tag);
        records.extend((entry_at as u32).to_le_bytes());
        records.extend((mine.len() as u32).to_le_bytes());
        for (_, id, bytes) in &mine {
            entries.extend(id.to_le_bytes());
            entries.extend(((first_file + data.len()) as u32).to_le_bytes());
            entries.extend((bytes.len() as u32).to_le_bytes());
            data.extend(bytes);
        }
        entry_at += 12 * mine.len();
    }
    [header, records, entries, data].concat()
}

pub const SLP: [u8; 4] = *b" pls";
pub const BINA: [u8; 4] = *b"anib";

/// A palette where colour `i` is `(i, 255 - i, 7)`.
pub fn palette() -> Vec<u8> {
    let mut text = String::from("JASC-PAL\r\n0100\r\n256\r\n");
    for i in 0..256 {
        text += &format!("{i} {} 7\r\n", 255 - i);
    }
    text.into_bytes()
}

/// The RGBA a palette index decodes to with [`palette`].
pub fn colour(i: u8) -> [u8; 4] {
    [i, 255 - i, 7, 255]
}

/// An SLP sprite of `frames` frames, each one row of `commands`, `width`
/// pixels wide, with its hotspot at (1, 0).
pub fn slp(frames: usize, width: i32, commands: &[u8]) -> Vec<u8> {
    let mut out = b"2.0N".to_vec();
    out.extend((frames as i32).to_le_bytes());
    out.extend([0u8; 24]);
    let body_at = 32 + 32 * frames;
    let body_len = 4 + 4 + commands.len();
    for f in 0..frames {
        let at = body_at + body_len * f;
        out.extend(((at + 4) as u32).to_le_bytes()); // command table
        out.extend((at as u32).to_le_bytes()); // outline table
        out.extend(0u32.to_le_bytes());
        out.extend(0u32.to_le_bytes());
        out.extend(width.to_le_bytes());
        out.extend(1i32.to_le_bytes());
        out.extend(1i32.to_le_bytes());
        out.extend(0i32.to_le_bytes());
    }
    for f in 0..frames {
        let at = body_at + body_len * f;
        out.extend([0, 0, 0, 0]); // no padding
        out.extend(((at + 8) as u32).to_le_bytes());
        out.extend(commands);
    }
    out
}

/// One player-colour pixel (value 3), then one plain pixel (colour 200).
pub const TWO_PIXELS: [u8; 5] = [(1 << 4) | 0x06, 3, 1 << 2, 200, 0x0F];

/// The sprite IDs the game loads, with the sprite each gets.
pub fn graphics(overrides: &[(u32, Vec<u8>)]) -> Vec<([u8; 4], u32, Vec<u8>)> {
    [1473u32, 1181, 901, 131, 1270, 4479]
        .into_iter()
        .map(|id| {
            let bytes = overrides
                .iter()
                .find(|(o, _)| *o == id)
                .map_or_else(|| slp(1, 2, &TWO_PIXELS), |(_, b)| b.clone());
            (SLP, id, bytes)
        })
        .collect()
}

/// Writes a fake install to a fresh scratch directory: the palette, every
/// sprite the game loads, and `grass` as terrain 15001.
pub fn install(
    name: &str,
    graphics_files: Vec<([u8; 4], u32, Vec<u8>)>,
    grass: Vec<u8>,
) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aoe-install-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("data")).expect("scratch directory");
    let write = |file: &str, bytes: Vec<u8>| {
        std::fs::write(dir.join("data").join(file), bytes).expect("write archive")
    };
    write("interfac.drs", drs(&[(BINA, 50500, palette())]));
    write("graphics.drs", drs(&graphics_files));
    write("terrain.drs", drs(&[(SLP, 15001, grass)]));
    dir
}

pub fn remove(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}
