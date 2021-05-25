use std::path::Path;

use anyhow::{Context, Result};

use psoutils::quest::bin::QuestBin;
use psoutils::quest::dat::{QuestDat, QuestDatTableType};
use psoutils::quest::Quest;

use crate::utils::crc32;

fn format_description_field(description: &String) -> String {
    description
        .trim()
        .replace("\n", "\n                            ")
}

fn display_quest_bin_info(bin: &QuestBin) {
    let object_code_crc32 = crc32(bin.object_code.as_ref());
    let function_offset_table_crc32 = crc32(bin.function_offset_table.as_ref());

    println!("QUEST .BIN FILE");
    println!("======================================================================");
    println!("name:                       {}", bin.header.name);
    println!(
        "object_code:                size: {}, crc32: {:08x}",
        bin.object_code.len(),
        object_code_crc32
    );
    println!(
        "function_offset_table:      size: {}, crc32: {:08x}",
        bin.function_offset_table.len(),
        function_offset_table_crc32
    );
    println!("is_download:                {}", bin.header.is_download);
    println!(
        "quest_number:               {0} (8-bit)  {1}, 0x{1:04x} (16-bit)",
        bin.header.quest_number(),
        bin.header.quest_number_u16()
    );
    println!(
        "episode:                    {} (0x{:02x})",
        bin.header.episode() + 1,
        bin.header.episode()
    );
    println!("language:                   {:?}", bin.header.language);
    println!(
        "short_description:          {}\n",
        format_description_field(&bin.header.short_description)
    );
    println!(
        "long_description:           {}\n",
        format_description_field(&bin.header.long_description)
    );
    println!()
}

fn display_quest_dat_info(dat: &QuestDat, episode: u32) {
    println!("QUEST .DAT FILE");
    println!("================================================================================");

    for (index, table) in dat.tables.iter().enumerate() {
        let body_size = table.bytes.len();
        let body_crc32 = crc32(table.bytes.as_ref());

        match table.table_type() {
            QuestDatTableType::Object => {
                let num_entities = body_size / 68;
                println!(
                    "{:3} {:5} {:<21} {:30} {:5}   {:08x}",
                    index,
                    body_size,
                    table.table_type().to_string(),
                    table.area_name(episode).to_string(),
                    num_entities,
                    body_crc32
                );
            }
            QuestDatTableType::NPC => {
                let num_entities = body_size / 72;
                println!(
                    "{:3} {:5} {:<21} {:30} {:5}   {:08x}",
                    index,
                    body_size,
                    table.table_type().to_string(),
                    table.area_name(episode).to_string(),
                    num_entities,
                    body_crc32
                );
            }
            QuestDatTableType::Wave => {
                println!(
                    "{:3} {:5} {:<21} {:30}         {:08x}",
                    index,
                    body_size,
                    table.table_type().to_string(),
                    table.area_name(episode).to_string(),
                    body_crc32
                );
            }
            QuestDatTableType::ChallengeModeSpawns => {
                println!(
                    "{:3} {:5} {:<21} {:30}         {:08x}",
                    index,
                    body_size,
                    table.table_type().to_string(),
                    table.area_name(episode).to_string(),
                    body_crc32
                );
            }
            QuestDatTableType::ChallengeModeUnknown => {
                println!(
                    "{:3} {:5} {:<21} {:30}         {:08x}",
                    index,
                    body_size,
                    table.table_type().to_string(),
                    table.area_name(episode).to_string(),
                    body_crc32
                );
            }
            QuestDatTableType::Unknown(n) => {
                println!("{:3} {:5} Unknown: {}", index, body_size, n);
            }
        };
    }
}

pub fn quest_info(args: &[String]) -> Result<()> {
    let quest = match args.len() {
        0 => {
            println!("No quest file(s) specified.");
            std::process::exit(1);
        }
        1 => {
            println!("Loading quest from .qst file ...");
            let qst_path = Path::new(&args[0]);
            Quest::from_qst_file(qst_path).context("Unable to load .qst file")?
        }
        2 => {
            println!("Loading quest from .bin and .dat file ...");
            let bin_path = Path::new(&args[0]);
            let dat_path = Path::new(&args[1]);
            Quest::from_bindat_files(bin_path, dat_path)
                .context("Unable to load .bin/.dat files")?
        }
        _ => {
            println!("Too many arguments. Should only specify either a single .qst file, or a .bin and .dat file.");
            std::process::exit(1);
        }
    };

    display_quest_bin_info(&quest.bin);
    display_quest_dat_info(&quest.dat, quest.bin.header.episode() as u32);

    Ok(())
}
