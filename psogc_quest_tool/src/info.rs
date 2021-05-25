use std::path::Path;

use anyhow::{anyhow, Context, Result};

use psoutils::quest::Quest;

pub fn quest_info(args: &[String]) -> Result<()> {
    println!("Showing quest information");

    let quest = match args.len() {
        0 => {
            return Err(anyhow!("No quest file(s) specified."));
        }
        1 => {
            println!("Loading quest from:\n    .qst file: {}", &args[0]);
            let qst_path = Path::new(&args[0]);
            Quest::from_qst_file(qst_path).context("Failed to load quest from .qst file")?
        }
        2 => {
            println!(
                "Loading quest from:\n    .bin file: {}\n    .dat file: {}",
                &args[0], &args[1]
            );
            let bin_path = Path::new(&args[0]);
            let dat_path = Path::new(&args[1]);
            Quest::from_bindat_files(bin_path, dat_path)
                .context("Failed to load quest from .bin/.dat files")?
        }
        _ => {
            return Err(anyhow!("Too many arguments. Should only specify either a single .qst file, or a .bin and .dat file."));
        }
    };

    println!();
    println!("{}", quest.display_bin_info());
    println!();
    println!("{}", quest.display_dat_info());
    println!();

    Ok(())
}

#[cfg(test)]
mod tests {
    use claim::*;

    use super::*;

    // TODO: some way to match the specific error message string? or probably should just replace
    //       anyhow usage with a specific error type ...

    #[test]
    pub fn no_args_fails_with_error() {
        let args: &[String] = &[];
        assert_matches!(quest_info(args), Err(_));
    }

    #[test]
    pub fn too_many_args_fails_with_error() {
        let args = &["a".to_string(), "b".to_string(), "c".to_string()];
        assert_matches!(quest_info(args), Err(_));
    }

    #[test]
    pub fn succeeds_with_single_file_arg() {
        let args = &["../test-assets/q058-ret-gc.online.qst".to_string()];
        assert_ok!(quest_info(args));
    }

    #[test]
    pub fn succeeds_with_two_file_args() {
        let args = &[
            "../test-assets/q058-ret-gc.bin".to_string(),
            "../test-assets/q058-ret-gc.dat".to_string(),
        ];
        assert_ok!(quest_info(args));
    }

    #[test]
    pub fn fails_when_bin_dat_file_args_in_wrong_order() {
        let args = &[
            "../test-assets/q058-ret-gc.dat".to_string(),
            "../test-assets/q058-ret-gc.bin".to_string(),
        ];
        assert_matches!(quest_info(args), Err(_));
    }
}
