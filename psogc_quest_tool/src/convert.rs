use std::convert::TryFrom;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use psoutils::quest::Quest;

#[derive(Debug, Eq, PartialEq)]
pub enum ConvertFormat {
    RawBinDat,
    PrsBinDat,
    OnlineQst,
    OfflineQst,
}

impl TryFrom<&str> for ConvertFormat {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        use ConvertFormat::*;
        match value.to_lowercase().as_str() {
            "raw_bindat" => Ok(RawBinDat),
            "prs_bindat" => Ok(PrsBinDat),
            "online_qst" => Ok(OnlineQst),
            "offline_qst" => Ok(OfflineQst),
            other => Err(format!("Not a valid conversion format: {}", other)),
        }
    }
}

fn collect_args(args: &[String]) -> Result<(&[String], ConvertFormat, &[String])> {
    if args.len() < 3 {
        return Err(anyhow!("Not enough arguments supplied"));
    }

    let mut convert_format_arg_index = None;
    let mut convert_format = None;
    // find the ConvertFormat argument, wherever it may be
    for (index, arg) in args.iter().enumerate() {
        if let Ok(format) = ConvertFormat::try_from(arg.as_str()) {
            if convert_format.is_some() {
                return Err(anyhow!("More than one conversion format specified"));
            }

            convert_format_arg_index = Some(index);
            convert_format = Some(format);
        }
    }

    if let Some(index) = convert_format_arg_index {
        // the ConvertFormat arg should be specified in-between the input file argument(s) and the
        // output file argument(s), so it should never exist at the very beginning or very end of
        // the arguments list.
        if index == 0 {
            return Err(anyhow!("No input file(s) provided"));
        } else if index == (args.len() - 1) {
            return Err(anyhow!("No output file(s) provided"));
        }

        let input_file_args = &args[0..index];
        let convert_format = convert_format.unwrap();
        let output_file_args = &args[(index + 1)..];
        Ok((input_file_args, convert_format, output_file_args))
    } else {
        return Err(anyhow!("No conversion format specified"));
    }
}

fn load_quest(input_files: &[String]) -> Result<Quest> {
    if input_files.len() == 2 {
        println!(
            "Loading quest from:\n    .bin file: {}\n    .dat file: {}",
            &input_files[0], &input_files[1]
        );
        let bin_path = Path::new(&input_files[0]);
        let dat_path = Path::new(&input_files[1]);
        Quest::from_bindat_files(bin_path, dat_path)
            .context("Failed to load quest from .bin/.dat files")
    } else {
        println!("Loading quest from:\n    .qst file: {}", &input_files[0]);
        let qst_path = Path::new(&input_files[0]);
        Quest::from_qst_file(qst_path).context("Failed to load quest from .qst file")
    }
}

fn convert_to_raw_bindat(input_files: &[String], output_files: &[String]) -> Result<()> {
    println!("Performing conversion to raw/uncompressed .bin/.dat quest files");

    if input_files.len() > 2 {
        return Err(anyhow!(
            "Too many input files specified. Expected either: two (.bin + .dat) or one (.qst)"
        ));
    }
    if output_files.len() != 2 {
        return Err(anyhow!(
            "Incorrect number of output files specified. Expected two: a .bin and a .dat file."
        ));
    }

    let quest = load_quest(input_files)?;

    println!(
        "Saving converted quest to:\n    .bin file: {}\n    .dat file: {}",
        &output_files[0], &output_files[1]
    );
    let output_bin_path = Path::new(&output_files[0]);
    let output_dat_path = Path::new(&output_files[1]);
    quest
        .to_uncompressed_bindat_files(output_bin_path, output_dat_path)
        .context("Failed to save quest to uncompressed .bin/.dat files")?;

    Ok(())
}

fn convert_to_prs_bindat(input_files: &[String], output_files: &[String]) -> Result<()> {
    println!("Performing conversion to PRS-compressed .bin/.dat quest files");

    if input_files.len() > 2 {
        return Err(anyhow!(
            "Too many input files specified. Expected either: two (.bin + .dat) or one (.qst)"
        ));
    }
    if output_files.len() != 2 {
        return Err(anyhow!(
            "Incorrect number of output files specified. Expected two: a .bin and a .dat file."
        ));
    }

    let quest = load_quest(input_files)?;

    println!(
        "Saving converted quest to:\n    .bin file: {}\n    .dat file: {}",
        &output_files[0], &output_files[1]
    );
    let output_bin_path = Path::new(&output_files[0]);
    let output_dat_path = Path::new(&output_files[1]);
    quest
        .to_compressed_bindat_files(output_bin_path, output_dat_path)
        .context("Failed to save quest to compressed .bin/.dat files")?;

    Ok(())
}

fn convert_to_online_qst(input_files: &[String], output_files: &[String]) -> Result<()> {
    println!("Performing conversion to server/online .qst quest file");

    if input_files.len() > 2 {
        return Err(anyhow!(
            "Too many input files specified. Expected either: two (.bin + .dat) or one (.qst)"
        ));
    }
    if output_files.len() != 1 {
        return Err(anyhow!(
            "Incorrect number of output files specified. Expected one .qst file."
        ));
    }

    let mut quest = load_quest(input_files)?;

    // turn download flag off (download = offline)
    quest.set_is_download(false);

    println!(
        "Saving converted quest to:\n    .qst file: {}",
        &output_files[0]
    );
    let output_qst_path = Path::new(&output_files[0]);
    quest
        .to_qst_file(output_qst_path)
        .context("Failed to save quest to server/online .qst file")?;

    Ok(())
}

fn convert_to_offline_qst(input_files: &[String], output_files: &[String]) -> Result<()> {
    println!("Performing conversion to download/offline .qst quest file");

    if input_files.len() > 2 {
        return Err(anyhow!(
            "Too many input files specified. Expected either: two (.bin + .dat) or one (.qst)"
        ));
    }
    if output_files.len() != 1 {
        return Err(anyhow!(
            "Incorrect number of output files specified. Expected one .qst file."
        ));
    }

    let mut quest = load_quest(input_files)?;

    // turn download flag on (download = offline)
    quest.set_is_download(true);

    println!(
        "Saving converted quest to:\n    .qst file: {}",
        &output_files[0]
    );
    let output_qst_path = Path::new(&output_files[0]);
    quest
        .to_qst_file(output_qst_path)
        .context("Failed to save quest to download/offline .qst file")?;

    Ok(())
}

pub fn quest_convert(args: &[String]) -> Result<()> {
    use ConvertFormat::*;

    let (input_file_args, convert_format, output_file_args) = collect_args(args)?;

    match convert_format {
        RawBinDat => convert_to_raw_bindat(input_file_args, output_file_args)
            .context("Failed converting to raw/uncompressed .bin/.dat quest")?,
        PrsBinDat => convert_to_prs_bindat(input_file_args, output_file_args)
            .context("Failed converting to PRS-compressed .bin/.dat quest")?,
        OnlineQst => convert_to_online_qst(input_file_args, output_file_args)
            .context("Failed converting to online .qst quest")?,
        OfflineQst => convert_to_offline_qst(input_file_args, output_file_args)
            .context("Failed converting to offline .qst quest")?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use claim::*;
    use tempfile::*;

    use psoutils::quest::bin::QuestBin;
    use psoutils::quest::dat::QuestDat;
    use psoutils::quest::qst::QuestQst;

    use super::*;

    #[test]
    pub fn collect_args_fails_with_less_than_minimum_arg_count() {
        let args: &[String] = &[];
        assert_matches!(collect_args(args), Err(_));

        let args = &["a".to_string(), "b".to_string()];
        assert_matches!(collect_args(args), Err(_));
    }

    #[test]
    pub fn collect_args_succeeds_in_expected_cases() {
        let args = &[
            "input.bin".to_string(),
            "input.dat".to_string(),
            "raw_bindat".to_string(),
            "output.bin".to_string(),
            "output.dat".to_string(),
        ];
        let (input, format, output) = collect_args(args).unwrap();
        assert_eq!(input, ["input.bin", "input.dat"]);
        assert_eq!(format, ConvertFormat::RawBinDat);
        assert_eq!(output, ["output.bin", "output.dat"]);

        let args = &[
            "input.qst".to_string(),
            "prs_bindat".to_string(),
            "output.bin".to_string(),
            "output.dat".to_string(),
        ];
        let (input, format, output) = collect_args(args).unwrap();
        assert_eq!(input, ["input.qst"]);
        assert_eq!(format, ConvertFormat::PrsBinDat);
        assert_eq!(output, ["output.bin", "output.dat"]);

        let args = &[
            "input.bin".to_string(),
            "input.dat".to_string(),
            "online_qst".to_string(),
            "output.qst".to_string(),
        ];
        let (input, format, output) = collect_args(args).unwrap();
        assert_eq!(input, ["input.bin", "input.dat"]);
        assert_eq!(format, ConvertFormat::OnlineQst);
        assert_eq!(output, ["output.qst"]);

        let args = &[
            "input.qst".to_string(),
            "offline_qst".to_string(),
            "output.qst".to_string(),
        ];
        let (input, format, output) = collect_args(args).unwrap();
        assert_eq!(input, ["input.qst"]);
        assert_eq!(format, ConvertFormat::OfflineQst);
        assert_eq!(output, ["output.qst"]);
    }

    #[test]
    pub fn collect_args_fails_when_no_convert_format_arg_is_provided() {
        let args = &[
            "input.bin".to_string(),
            "input.dat".to_string(),
            "output.bin".to_string(),
            "output.dat".to_string(),
        ];
        assert_matches!(collect_args(args), Err(_));
    }

    #[test]
    pub fn collect_args_fails_when_convert_format_arg_is_provided_multiple_times() {
        let args = &[
            "input.bin".to_string(),
            "input.dat".to_string(),
            "online_qst".to_string(),
            "online_qst".to_string(),
            "output.qst".to_string(),
        ];
        assert_matches!(collect_args(args), Err(_));
    }

    #[test]
    pub fn collect_args_fails_when_no_output_file_args_provided() {
        let args = &[
            "input.bin".to_string(),
            "input.dat".to_string(),
            "online_qst".to_string(),
        ];
        assert_matches!(collect_args(args), Err(_));
    }

    #[test]
    pub fn can_convert_to_raw_bindat() {
        let tmp_dir = TempDir::new().unwrap();
        let bin_save_path = tmp_dir.path().join("quest58.bin");
        let dat_save_path = tmp_dir.path().join("quest58.dat");

        let args = &[
            "../test-assets/q058-ret-gc.online.qst".to_string(),
            "raw_bindat".to_string(),
            bin_save_path.to_string_lossy().into_owned(),
            dat_save_path.to_string_lossy().into_owned(),
        ];
        assert_ok!(quest_convert(args));
        assert_ok!(QuestBin::from_uncompressed_file(&bin_save_path));
        assert_ok!(QuestDat::from_uncompressed_file(&dat_save_path));
    }

    #[test]
    pub fn can_convert_to_prs_bindat() {
        let tmp_dir = TempDir::new().unwrap();
        let bin_save_path = tmp_dir.path().join("quest58.bin");
        let dat_save_path = tmp_dir.path().join("quest58.dat");

        let args = &[
            "../test-assets/q058-ret-gc.offline.qst".to_string(),
            "prs_bindat".to_string(),
            bin_save_path.to_string_lossy().into_owned(),
            dat_save_path.to_string_lossy().into_owned(),
        ];
        assert_ok!(quest_convert(args));
        assert_ok!(QuestBin::from_compressed_file(&bin_save_path));
        assert_ok!(QuestDat::from_compressed_file(&dat_save_path));
    }

    #[test]
    pub fn can_convert_to_online_qst() {
        let tmp_dir = TempDir::new().unwrap();
        let qst_save_path = tmp_dir.path().join("quest58.qst");

        let args = &[
            "../test-assets/q058-ret-gc.bin".to_string(),
            "../test-assets/q058-ret-gc.dat".to_string(),
            "online_qst".to_string(),
            qst_save_path.to_string_lossy().into_owned(),
        ];
        assert_ok!(quest_convert(args));
        assert_ok!(QuestQst::from_file(&qst_save_path));
    }

    #[test]
    pub fn can_convert_to_offline_qst() {
        let tmp_dir = TempDir::new().unwrap();
        let qst_save_path = tmp_dir.path().join("quest58.qst");

        let args = &[
            "../test-assets/q058-ret-gc.uncompressed.bin".to_string(),
            "../test-assets/q058-ret-gc.uncompressed.dat".to_string(),
            "offline_qst".to_string(),
            qst_save_path.to_string_lossy().into_owned(),
        ];
        assert_ok!(quest_convert(args));
        assert_ok!(QuestQst::from_file(&qst_save_path));
    }
}
