use std::path::Path;

use thiserror::Error;

use crate::bytes::{ReadBytesError, WriteBytesError};
use crate::quest::bin::QuestBin;
use crate::quest::dat::QuestDat;
use crate::text::LanguageError;

pub mod bin;
pub mod dat;

#[derive(Error, Debug)]
pub enum QuestError {
    #[error("I/O error reading quest")]
    IoError(#[from] std::io::Error),

    #[error("String encoding error during processing of quest string field")]
    StringEncodingError(#[from] LanguageError),

    #[error("Error reading quest from bytes")]
    ReadFromBytesError(#[from] ReadBytesError),

    #[error("Error writing quest as bytes")]
    WriteAsBytesError(#[from] WriteBytesError),
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
