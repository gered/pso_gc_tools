use std::env;
use std::path::Path;

use anyhow::{Context, Result};

use gci_quest_extract::gci::extract_to_bindat;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn display_banner() {
    println!("gci_quest_extract v{}", VERSION);
}

fn display_help() {
    println!("Tool for extracting PSO Gamecube quests out of pre-decrypted .gci files.\n");
    println!("USAGE: gci_quest_extract <quest_1.gci> <quest_2.gci> <output.bin> <output.dat>");
}

fn main() -> Result<()> {
    display_banner();

    let args: Vec<String> = env::args().collect();
    if args.len() != 5 {
        display_help();
    } else {
        let gci1_path = Path::new(&args[1]);
        let gci2_path = Path::new(&args[2]);
        let output_bin_path = Path::new(&args[3]);
        let output_dat_path = Path::new(&args[4]);
        extract_to_bindat(gci1_path, gci2_path, output_bin_path, output_dat_path)
            .context("Failed to extract quest from GCI files")?;
    }

    Ok(())
}
