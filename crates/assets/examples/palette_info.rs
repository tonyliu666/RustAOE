//! Prints colours from the game palette: `palette_info INTERFAC.DRS`.
//!
//! A development aid for checking the reader against a real install. The
//! palette is file 50500 in `interfac.drs`.

use assets::{Drs, Palette};

const GAME_PALETTE: u32 = 50500;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: palette_info INTERFAC.DRS")?;
    let bytes = std::fs::read(&path)?;
    let drs = Drs::parse(&bytes)?;
    let file = drs
        .get(GAME_PALETTE)
        .ok_or("no palette 50500 in this archive")?;
    let palette = Palette::parse(file)?;
    for index in [0u8, 1, 2, 16, 55, 255] {
        println!("{index:3}: {:?}", palette.rgb(index));
    }
    Ok(())
}
