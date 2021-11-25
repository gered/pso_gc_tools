use std::env;
use std::path::Path;

use anyhow::{Context, Result};

use decrypt_packets::pcap::analyze;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn display_banner() {
    println!("decrypt_packets v{}", VERSION);
}

fn display_help() {
    println!("Tool for decrypting and displaying raw packets captured from a PSO client/server session.\n");
    println!("USAGE: decrypt_packets <capture.pcapng>");
}

fn main() -> Result<()> {
    display_banner();

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        display_help();
    } else {
        let pcap_path = Path::new(&args[1]);
        analyze(pcap_path).context("Failed to analyze pcap file")?;
    }

    Ok(())
}
