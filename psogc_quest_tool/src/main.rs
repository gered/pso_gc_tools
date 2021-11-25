use std::env;

use anyhow::Result;

use psogc_quest_tool::convert::quest_convert;
use psogc_quest_tool::info::quest_info;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn display_banner() {
    println!("psogc_quest_tool v{}", VERSION);
}

fn display_help() {
    println!("Tool for PSO Gamecube quest bin/dat and/or qst files.\n");
    println!("USAGE: psogc_quest_tool <COMMAND> <ARGS...>\n");
    println!("COMMANDS:");
    println!("  info    - Displays info about a quest.");
    println!("             - info <input.bin> <input.dat>");
    println!("             - info <input.qst>");
    println!("  convert - Converts a quest to a different file format");
    println!("             - convert <input files> <output_format_type> <output files>");
    println!("            Where the arguments:");
    println!("             - \"input files\" and \"output files\" should either be:");
    println!("                a) two files, a .bin and .dat file; or");
    println!("                b) a single .qst file");
    println!("             - \"output_format_type\" should be one of: ");
    println!("                - raw_bindat (produces a .bin and .dat, both uncompressed)");
    println!("                - prs_bindat (produces a .bin and .dat, both PRS compressed)");
    println!("                - online_qst (produces a .qst, for online play via a server)");
    println!("                - offline_qst (produces a .qst, for offline play from a mem");
    println!("                               card when downloaded from a server)");
}

fn main() -> Result<()> {
    display_banner();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        display_help();
    } else {
        let command = &args[1];
        let remaining_args = &args[2..];
        match command.to_lowercase().as_str() {
            "info" => quest_info(&remaining_args)?,
            "convert" => quest_convert(&remaining_args)?,
            _ => {
                println!("Unrecognized command");
                display_help();
            }
        };
    }
    Ok(())
}
