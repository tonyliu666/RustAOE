//! Lists the tables of DRS archives: `drs_info FILE...`.
//!
//! A development aid for checking the reader against a real install. It reads
//! the files it is given and prints counts; it writes nothing.

use assets::Drs;

fn main() {
    let mut failed = false;
    for path in std::env::args().skip(1) {
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!("{path}: {error}");
                failed = true;
                continue;
            }
        };
        match Drs::parse(&bytes) {
            Ok(drs) => {
                let tables: Vec<String> = drs
                    .tables()
                    .map(|t| format!("{} {}", t.len(), t.kind()))
                    .collect();
                let ids: Vec<u32> = drs.ids().collect();
                println!(
                    "{path}: {} (ids {}..={})",
                    tables.join(", "),
                    ids.first().copied().unwrap_or(0),
                    ids.last().copied().unwrap_or(0)
                );
            }
            Err(error) => {
                eprintln!("{path}: {error}");
                failed = true;
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}
