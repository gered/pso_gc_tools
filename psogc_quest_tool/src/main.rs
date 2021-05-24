use std::env;

use anyhow::{Context, Result};

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
    println!("            - info input.bin input.dat");
    println!("            - info input.qst");
    println!("  convert - Converts a quest to a different file format");
    println!("            - convert <input bin+dat or qst> <format_type> <output bin+dat or qst>");
    println!("            Where format_type should be one of: raw_bindat, prs_bindat, online_qst, offline_qst");
}

fn main() -> Result<()> {
    display_banner();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        display_help();
    }
    let command = &args[1];
    let remaining_args = &args[2..];
    match command.to_lowercase().as_str() {
        "info" => quest_info(&remaining_args),
        "convert" => quest_convert(&remaining_args),
        _ => {
            println!("Unrecognized command");
            display_help();
            Ok(())
        }
    }?;

    Ok(())
}
