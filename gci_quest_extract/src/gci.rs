use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use byteorder::{BigEndian, ReadBytesExt};

use psoutils::bytes::ReadFixedLengthByteArray;
use psoutils::quest::bin::QuestBin;
use psoutils::quest::dat::QuestDat;
use psoutils::quest::Quest;

// see https://github.com/suloku/gcmm/blob/master/source/gci.h for detailed GCI file format header
// we will not be re-defining that struct here, since we're only interested in a handful of fields

const GCI_HEADER_SIZE: usize = 64;
const CARD_FILE_HEADER: usize = 0x2040;
const DATA_START_OFFSET: usize = GCI_HEADER_SIZE + CARD_FILE_HEADER;

fn extract_quest_data(path: &Path) -> Result<Box<[u8]>> {
    let mut file = File::open(path)?;

    let gamecode: [u8; 4] = file.read_bytes()?;
    if &gamecode != b"GPOJ" && &gamecode != b"GPOE" && &gamecode != b"GPOP" {
        return Err(anyhow!(
            "GCI header 'gamecode' field does not match any expected string: {:02x?}",
            gamecode
        ));
    }

    let company: [u8; 2] = file.read_bytes()?;
    if &company != b"8P" {
        return Err(anyhow!(
            "GCI header 'company' field is not the expected value: {:02x?}",
            company
        ));
    }

    // move past the majority of GCI header and the actual Gamecube memory card header
    file.seek(SeekFrom::Start(DATA_START_OFFSET as u64))?;

    // this "size" value actually accounts for an extra dword value that we do not care about
    let data_size = file.read_u32::<BigEndian>()? - 4;

    // move past the remaining bits of the header to the actual start of the quest data
    file.seek(SeekFrom::Current(20))?;

    // there will be remaining junk after the data which we probably don't want, so only read
    // the exact amount of bytes indicated in the header
    let mut buffer = vec![0u8; data_size as usize];
    file.read_exact(&mut buffer)?;

    Ok(buffer.into_boxed_slice())
}

fn load_quest_from_gci_files(gci1: &Path, gci2: &Path) -> Result<Quest> {
    let gci1_bytes = extract_quest_data(gci1).context(format!(
        "Failed to extract quest data from: {}",
        gci1.to_string_lossy()
    ))?;
    let gci2_bytes = extract_quest_data(gci2).context(format!(
        "Failed to extract quest data from: {}",
        gci2.to_string_lossy()
    ))?;

    // now try to figure out which is the .bin and which is the .dat
    let bin: QuestBin;
    let dat: QuestDat;
    if let Ok(loaded) = QuestBin::from_compressed_bytes(gci1_bytes.as_ref()) {
        bin = loaded;
        dat = QuestDat::from_compressed_bytes(gci2_bytes.as_ref())
            .context("Failed to load second GCI file as quest .dat")?;
    } else if let Ok(loaded) = QuestDat::from_compressed_bytes(gci1_bytes.as_ref()) {
        dat = loaded;
        bin = QuestBin::from_compressed_bytes(gci2_bytes.as_ref())
            .context("Failed to load second GCI file as quest .bin")?;
    } else {
        return Err(anyhow!("Unable to load first GCI file as either a quest .bin or .dat file. It might not contain quest data, or it might not be pre-decrypted, or it might be corrupted."));
    }

    Ok(Quest { bin, dat })
}

pub fn extract_to_bindat(
    gci1: &Path,
    gci2: &Path,
    output_bin: &Path,
    output_dat: &Path,
) -> Result<()> {
    println!(
        "Reading quest data from GCI files:\n    - {}\n    - {}",
        gci1.to_string_lossy(),
        gci2.to_string_lossy()
    );

    let mut quest = load_quest_from_gci_files(gci1, gci2)?;

    println!("Loaded quest .bin and .dat data successfully.\n");
    println!(
        "{}\n{}\n",
        quest.display_bin_info(),
        quest.display_dat_info()
    );

    if quest.is_download() {
        println!("Turning 'download' flag off before saving.");
        quest.set_is_download(false);
    }

    println!(
        "Saving quest as PRS-compressed bin/dat files:\n    .bin file: {}\n    .dat file: {}",
        output_bin.to_string_lossy(),
        output_dat.to_string_lossy()
    );

    quest
        .to_compressed_bindat_files(output_bin, output_dat)
        .context("Failed to save quest to bin/dat files")?;

    Ok(())
}
