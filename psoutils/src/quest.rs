use std::path::Path;

use thiserror::Error;

use crate::quest::bin::{QuestBin, QuestBinError};
use crate::quest::dat::{QuestDat, QuestDatError};

pub mod bin;
pub mod dat;

#[derive(Error, Debug)]
pub enum QuestError {
    #[error("I/O error reading quest")]
    IoError(#[from] std::io::Error),

    #[error("Error processing quest bin")]
    QuestBinError(#[from] QuestBinError),

    #[error("Error processing quest dat")]
    QuestDatError(#[from] QuestDatError),
}

pub struct Quest {
    pub bin: QuestBin,
    pub dat: QuestDat,
}

impl Quest {
    pub fn from_compressed_bindat(bin_path: &Path, dat_path: &Path) -> Result<Quest, QuestError> {
        let bin = QuestBin::from_compressed_file(bin_path)?;
        let dat = QuestDat::from_compressed_file(dat_path)?;

        Ok(Quest { bin, dat })
    }

    pub fn from_uncompressed_bindat(bin_path: &Path, dat_path: &Path) -> Result<Quest, QuestError> {
        let bin = QuestBin::from_uncompressed_file(bin_path)?;
        let dat = QuestDat::from_uncompressed_file(dat_path)?;

        Ok(Quest { bin, dat })
    }
}

#[cfg(test)]
mod tests {}
