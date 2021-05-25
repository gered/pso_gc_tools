use std::fmt::Write;
use std::path::Path;

use byteorder::WriteBytesExt;
use thiserror::Error;

use crate::quest::bin::{QuestBin, QuestBinError};
use crate::quest::dat::{QuestDat, QuestDatError, QuestDatTableType};
use crate::quest::qst::{QuestQst, QuestQstError};
use crate::text::Language;
use crate::utils::crc32;

pub mod bin;
pub mod dat;
pub mod qst;

fn format_description_field(description: &String) -> String {
    description
        .trim()
        .replace("\n", "\n                            ")
}

#[derive(Error, Debug)]
pub enum QuestError {
    #[error("I/O error reading quest")]
    IoError(#[from] std::io::Error),

    #[error("Error processing quest bin")]
    QuestBinError(#[from] QuestBinError),

    #[error("Error processing quest dat")]
    QuestDatError(#[from] QuestDatError),

    #[error("Error processing quest qst")]
    QuestQstError(#[from] QuestQstError),
}

pub struct Quest {
    pub bin: QuestBin,
    pub dat: QuestDat,
}

impl Quest {
    pub fn from_bindat_files(bin_path: &Path, dat_path: &Path) -> Result<Quest, QuestError> {
        // try to load bin and dat files each as compressed files first as that is the normal
        // format that these are stored as. if that fails, then try one more time for each one
        // to load as an uncompressed file. if that fails too, return the error

        let bin = match QuestBin::from_compressed_file(bin_path) {
            Err(QuestBinError::PrsCompressionError(_)) => {
                QuestBin::from_uncompressed_file(bin_path)?
            }
            Err(e) => return Err(QuestError::QuestBinError(e)),
            Ok(bin) => bin,
        };

        let dat = match QuestDat::from_compressed_file(dat_path) {
            Err(QuestDatError::PrsCompressionError(_)) => {
                QuestDat::from_uncompressed_file(dat_path)?
            }
            Err(e) => return Err(QuestError::QuestDatError(e)),
            Ok(dat) => dat,
        };

        Ok(Quest { bin, dat })
    }

    pub fn from_qst_file(path: &Path) -> Result<Quest, QuestError> {
        let qst = QuestQst::from_file(path)?;
        Self::from_qst(qst)
    }

    pub fn from_qst(qst: QuestQst) -> Result<Quest, QuestError> {
        let bin = qst.extract_bin()?;
        let dat = qst.extract_dat()?;

        Ok(Quest { bin, dat })
    }

    pub fn as_qst(&self) -> Result<QuestQst, QuestError> {
        Ok(QuestQst::from_bindat(&self.bin, &self.dat)?)
    }

    pub fn write_as_qst_bytes<T: WriteBytesExt>(&self, writer: &mut T) -> Result<(), QuestError> {
        let qst = self.as_qst()?;
        Ok(qst.write_bytes(writer)?)
    }

    pub fn to_qst_file(&self, path: &Path) -> Result<(), QuestError> {
        let qst = QuestQst::from_bindat(&self.bin, &self.dat)?;
        Ok(qst.to_file(path)?)
    }

    pub fn to_compressed_bindat_files(
        &self,
        bin_path: &Path,
        dat_path: &Path,
    ) -> Result<(), QuestError> {
        self.bin.to_compressed_file(bin_path)?;
        self.dat.to_compressed_file(dat_path)?;
        Ok(())
    }

    pub fn to_uncompressed_bindat_files(
        &self,
        bin_path: &Path,
        dat_path: &Path,
    ) -> Result<(), QuestError> {
        self.bin.to_uncompressed_file(bin_path)?;
        self.dat.to_uncompressed_file(dat_path)?;
        Ok(())
    }

    pub fn name(&self) -> &String {
        &self.bin.header.name
    }

    pub fn short_description(&self) -> &String {
        &self.bin.header.short_description
    }

    pub fn long_description(&self) -> &String {
        &self.bin.header.long_description
    }

    pub fn language(&self) -> Language {
        self.bin.header.language
    }

    pub fn is_download(&self) -> bool {
        self.bin.header.is_download
    }

    pub fn set_is_download(&mut self, value: bool) {
        self.bin.header.is_download = value
    }

    pub fn quest_number(&self) -> u8 {
        self.bin.header.quest_number()
    }

    pub fn quest_number_u16(&self) -> u16 {
        self.bin.header.quest_number_u16()
    }

    pub fn episode(&self) -> u8 {
        self.bin.header.episode()
    }

    pub fn display_bin_info(&self) -> String {
        let object_code_crc32 = crc32(self.bin.object_code.as_ref());
        let function_offset_table_crc32 = crc32(self.bin.function_offset_table.as_ref());

        let mut s = String::new();

        // HACK: i'm just directly calling .unwrap() for all of these because we're writing into
        //       a string buffer that we own here, so this should really never fail and i didn't
        //       want to have this method return a Result<>

        writeln!(s, "QUEST .BIN FILE").unwrap();
        writeln!(
            s,
            "======================================================================"
        )
        .unwrap();
        writeln!(
            s,
            "Decompressed Size:          {}",
            self.bin.calculate_size()
        )
        .unwrap();
        writeln!(s, "Name:                       {}", self.bin.header.name).unwrap();
        writeln!(
            s,
            "object_code:                size: {}, crc32: {:08x}",
            self.bin.object_code.len(),
            object_code_crc32
        )
        .unwrap();
        writeln!(
            s,
            "function_offset_table:      size: {}, crc32: {:08x}",
            self.bin.function_offset_table.len(),
            function_offset_table_crc32
        )
        .unwrap();
        writeln!(
            s,
            "Is Download?                {}",
            self.bin.header.is_download
        )
        .unwrap();
        writeln!(
            s,
            "Quest Number/ID:            {0} (8-bit)  {1}, 0x{1:04x} (16-bit)",
            self.bin.header.quest_number(),
            self.bin.header.quest_number_u16()
        )
        .unwrap();
        writeln!(
            s,
            "Episode:                    {} (0x{:02x})",
            self.bin.header.episode() + 1,
            self.bin.header.episode()
        )
        .unwrap();
        writeln!(
            s,
            "Language:                   {:?}, encoding: {}",
            self.bin.header.language,
            self.bin.header.language.get_encoding().name()
        )
        .unwrap();
        writeln!(
            s,
            "Short Description:          {}\n",
            format_description_field(&self.bin.header.short_description)
        )
        .unwrap();
        writeln!(
            s,
            "Long Description:           {}\n",
            format_description_field(&self.bin.header.long_description)
        )
        .unwrap();

        s
    }

    pub fn display_dat_info(&self) -> String {
        let mut s = String::new();

        let episode = self.bin.header.episode() as u32;

        // HACK: i'm just directly calling .unwrap() for all of these because we're writing into
        //       a string buffer that we own here, so this should really never fail and i didn't
        //       want to have this method return a Result<>

        writeln!(s, "QUEST .DAT FILE").unwrap();
        writeln!(
            s,
            "================================================================================"
        )
        .unwrap();
        writeln!(s, "Decompressed size: {}\n", self.dat.calculate_size()).unwrap();
        writeln!(
            s,
            "(Using episode {} to lookup table area names)",
            episode + 1
        )
        .unwrap();

        writeln!(
            s,
            "Idx Size  Table Type            Area                           Count   CRC32"
        )
        .unwrap();

        for (index, table) in self.dat.tables.iter().enumerate() {
            let body_size = table.bytes.len();
            let body_crc32 = crc32(table.bytes.as_ref());

            match table.table_type() {
                QuestDatTableType::Object => {
                    let num_entities = body_size / 68;
                    writeln!(
                        s,
                        "{:3} {:5} {:<21} {:30} {:5}   {:08x}",
                        index,
                        body_size,
                        table.table_type().to_string(),
                        table.area_name(episode).to_string(),
                        num_entities,
                        body_crc32
                    )
                    .unwrap();
                }
                QuestDatTableType::NPC => {
                    let num_entities = body_size / 72;
                    writeln!(
                        s,
                        "{:3} {:5} {:<21} {:30} {:5}   {:08x}",
                        index,
                        body_size,
                        table.table_type().to_string(),
                        table.area_name(episode).to_string(),
                        num_entities,
                        body_crc32
                    )
                    .unwrap();
                }
                QuestDatTableType::Wave => {
                    writeln!(
                        s,
                        "{:3} {:5} {:<21} {:30}         {:08x}",
                        index,
                        body_size,
                        table.table_type().to_string(),
                        table.area_name(episode).to_string(),
                        body_crc32
                    )
                    .unwrap();
                }
                QuestDatTableType::ChallengeModeSpawns => {
                    writeln!(
                        s,
                        "{:3} {:5} {:<21} {:30}         {:08x}",
                        index,
                        body_size,
                        table.table_type().to_string(),
                        table.area_name(episode).to_string(),
                        body_crc32
                    )
                    .unwrap();
                }
                QuestDatTableType::ChallengeModeUnknown => {
                    writeln!(
                        s,
                        "{:3} {:5} {:<21} {:30}         {:08x}",
                        index,
                        body_size,
                        table.table_type().to_string(),
                        table.area_name(episode).to_string(),
                        body_crc32
                    )
                    .unwrap();
                }
                QuestDatTableType::Unknown(n) => {
                    writeln!(s, "{:3} {:5} Unknown: {}", index, body_size, n).unwrap();
                }
            };
        }

        s
    }
}

#[cfg(test)]
mod tests {
    use claim::*;
    use tempfile::*;

    use super::*;

    #[test]
    pub fn can_load_from_compressed_bindat_files() {
        let bin_path = Path::new("../test-assets/q058-ret-gc.bin");
        let dat_path = Path::new("../test-assets/q058-ret-gc.dat");
        assert_ok!(Quest::from_bindat_files(bin_path, dat_path));
    }

    #[test]
    pub fn can_load_from_uncompressed_bindat_files() {
        let bin_path = Path::new("../test-assets/q058-ret-gc.uncompressed.bin");
        let dat_path = Path::new("../test-assets/q058-ret-gc.uncompressed.dat");
        assert_ok!(Quest::from_bindat_files(bin_path, dat_path));
    }

    #[test]
    pub fn can_load_from_offline_qst_file() {
        let path = Path::new("../test-assets/q058-ret-gc.offline.qst");
        assert_ok!(Quest::from_qst_file(path));
    }

    #[test]
    pub fn can_load_from_online_qst_file() {
        let path = Path::new("../test-assets/q058-ret-gc.online.qst");
        assert_ok!(Quest::from_qst_file(path));
    }

    #[test]
    pub fn can_create_from_qst_struct() {
        let qst = QuestQst::from_file(Path::new("../test-assets/q058-ret-gc.online.qst")).unwrap();
        assert_ok!(Quest::from_qst(qst));
    }

    #[test]
    pub fn can_save_to_compressed_bindat_files() -> Result<(), QuestError> {
        let quest = Quest::from_bindat_files(
            Path::new("../test-assets/q058-ret-gc.bin"),
            Path::new("../test-assets/q058-ret-gc.dat"),
        )?;
        let tmp_dir = TempDir::new()?;
        let bin_save_path = tmp_dir.path().join("quest58.bin");
        let dat_save_path = tmp_dir.path().join("quest58.dat");
        assert_ok!(quest.to_compressed_bindat_files(&bin_save_path, &dat_save_path));
        assert_ok!(QuestBin::from_compressed_file(&bin_save_path));
        assert_ok!(QuestDat::from_compressed_file(&dat_save_path));
        Ok(())
    }

    #[test]
    pub fn can_save_to_uncompressed_bindat_files() -> Result<(), QuestError> {
        let quest = Quest::from_bindat_files(
            Path::new("../test-assets/q058-ret-gc.bin"),
            Path::new("../test-assets/q058-ret-gc.dat"),
        )?;
        let tmp_dir = TempDir::new()?;
        let bin_save_path = tmp_dir.path().join("quest58.bin");
        let dat_save_path = tmp_dir.path().join("quest58.dat");
        assert_ok!(quest.to_uncompressed_bindat_files(&bin_save_path, &dat_save_path));
        assert_ok!(QuestBin::from_uncompressed_file(&bin_save_path));
        assert_ok!(QuestDat::from_uncompressed_file(&dat_save_path));
        Ok(())
    }

    #[test]
    pub fn can_save_to_qst_file() -> Result<(), QuestError> {
        let quest = Quest::from_bindat_files(
            Path::new("../test-assets/q058-ret-gc.bin"),
            Path::new("../test-assets/q058-ret-gc.dat"),
        )?;
        let tmp_dir = TempDir::new()?;
        let qst_save_path = tmp_dir.path().join("quest58.qst");
        assert_ok!(quest.to_qst_file(&qst_save_path));
        assert_ok!(QuestQst::from_file(&qst_save_path));
        Ok(())
    }
}
