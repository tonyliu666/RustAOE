//! Decodes sprites to raw RGBA files for inspection:
//! `slp_dump INTERFAC.DRS SPRITES.DRS OUT_DIR PLAYER [FRAME] -- ID...`.
//!
//! A development aid for checking the decoder against a real install and for
//! finding which sprite is which. Each sprite's frame is written as
//! `OUT_DIR/<id>_<width>x<height>.rgba`, 4 bytes per pixel, row by row. The
//! output is game art: keep it outside the repository.

use assets::{Drs, Palette, Slp};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let split = args
        .iter()
        .position(|a| a == "--")
        .ok_or("missing -- before IDs")?;
    let (head, ids) = (&args[..split], &args[split + 1..]);
    let [interfac, sprites, out, player, rest @ ..] = head else {
        return Err(
            "usage: slp_dump INTERFAC.DRS SPRITES.DRS OUT_DIR PLAYER [FRAME] -- ID...".into(),
        );
    };
    let frame: usize = rest.first().map_or(Ok(0), |f| f.parse())?;
    let player: u8 = player.parse()?;

    let interfac = std::fs::read(interfac)?;
    let palette = Palette::parse(Drs::parse(&interfac)?.get(50500).ok_or("no palette")?)?;
    let sprites = std::fs::read(sprites)?;
    let drs = Drs::parse(&sprites)?;

    for id in ids {
        let id: u32 = id.parse()?;
        let Some(bytes) = drs.get(id) else {
            eprintln!("{id}: not in archive");
            continue;
        };
        match Slp::parse(bytes).and_then(|slp| {
            let frames = slp.frames().count();
            slp.decode(frame.min(frames.saturating_sub(1)), &palette, player)
                .map(|image| (image, frames))
        }) {
            Ok((image, frames)) => {
                let path = format!("{out}/{id}_{}x{}.rgba", image.width, image.height);
                std::fs::write(&path, &image.rgba)?;
                println!("{id}: {frames} frames, {}x{}", image.width, image.height);
            }
            Err(error) => eprintln!("{id}: {error}"),
        }
    }
    Ok(())
}
