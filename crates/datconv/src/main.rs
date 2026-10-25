//! `datconv --install DIR --out FILE.ron [--all] [--civ N]`
//!
//! Reads `DIR/data/empires2_x1_p1.dat` and writes one civilisation's units
//! in the game's content format. The report (what was rounded, left out, or
//! drawn in pieces) goes to standard error.

use datconv::{convert, Selection};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: datconv --install DIR --out FILE.ron [--all] [--civ N]

  --install DIR  the extracted Age of Empires II install (the folder holding
                 data/empires2_x1_p1.dat)
  --out FILE     where to write the content (RON)
  --all          every trainable unit and building, not only the four types
                 the game has today
  --civ N        the civilisation to read (default 1, the British; 0 is Gaia)";

struct Options {
    install: PathBuf,
    out: PathBuf,
    selection: Selection,
    civ: usize,
}

fn options(args: impl Iterator<Item = String>) -> Result<Options, String> {
    let (mut install, mut out, mut selection, mut civ) = (None, None, Selection::Default, 1);
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--install" => install = Some(PathBuf::from(value("--install")?)),
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--all" => selection = Selection::All,
            "--civ" => {
                civ = value("--civ")?
                    .parse()
                    .map_err(|_| "--civ needs a number".to_string())?
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(Options {
        install: install.ok_or("--install is required")?,
        out: out.ok_or("--out is required")?,
        selection,
        civ,
    })
}

fn run(options: &Options) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let path = options.install.join("data").join("empires2_x1_p1.dat");
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let data = assets::dat::decompress(&bytes)?;
    let dat = assets::dat::Dat::parse(&data)?;
    let sprites = datconv::sprites_in(&options.install)?;
    let converted = convert(&dat, options.civ, options.selection, &|slp| {
        sprites.contains(&slp)
    })?;
    std::fs::write(&options.out, &converted.ron)
        .map_err(|e| format!("{}: {e}", options.out.display()))?;
    Ok(converted.report)
}

fn main() -> ExitCode {
    let options = match options(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(problem) => {
            eprintln!("{problem}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&options) {
        Ok(report) => {
            for line in &report {
                eprintln!("{line}");
            }
            eprintln!("wrote {}", options.out.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("datconv: {error}");
            ExitCode::FAILURE
        }
    }
}
