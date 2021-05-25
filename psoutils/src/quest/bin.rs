use std::fmt::{Debug, Formatter};
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Write};
use std::path::Path;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use thiserror::Error;

use crate::bytes::*;
use crate::compression::{prs_compress, prs_decompress, PrsCompressionError};
use crate::text::Language;

pub const QUEST_BIN_NAME_LENGTH: usize = 32;
pub const QUEST_BIN_SHORT_DESCRIPTION_LENGTH: usize = 128;
pub const QUEST_BIN_LONG_DESCRIPTION_LENGTH: usize = 288;

pub const QUEST_BIN_HEADER_SIZE: usize = 20
    + QUEST_BIN_NAME_LENGTH
    + QUEST_BIN_SHORT_DESCRIPTION_LENGTH
    + QUEST_BIN_LONG_DESCRIPTION_LENGTH;

#[derive(Error, Debug)]
pub enum QuestBinError {
    #[error("I/O error while processing quest bin")]
    IoError(#[from] std::io::Error),

    #[error("PRS compression failed")]
    PrsCompressionError(#[from] PrsCompressionError),

    #[error("Bad quest bin data format: {0}")]
    DataFormatError(String),
}

#[derive(Debug, Copy, Clone)]
pub struct QuestNumberAndEpisode {
    pub number: u8,
    pub episode: u8,
}

pub union QuestNumber {
    pub number_and_episode: QuestNumberAndEpisode,
    pub number: u16,
}

impl Debug for QuestNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "QuestNumber {{ number: {}, number_and_episode: {:?} }}",
            unsafe { self.number },
            unsafe { self.number_and_episode },
        )?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct QuestBinHeader {
    pub is_download: bool,
    pub language: Language,
    pub quest_number: QuestNumber,
    pub name: String,
    pub short_description: String,
    pub long_description: String,
}

impl QuestBinHeader {
    // the reality is that i kind of have to support access to the quest_number/episode as u8's as
    // well as the quest_number as a u16 simultaneously. it appears that all of sega's quests (at
    // least, all of the ones i've looked at in detail) used the quest_number and episode fields as
    // individual u8's, but there are quite a bunch of custom quests that stored quest_number
    // values as a u16 (i believe this is Qedit's fault?)

    pub fn quest_number(&self) -> u8 {
        unsafe { self.quest_number.number_and_episode.number }
    }

    pub fn quest_number_u16(&self) -> u16 {
        unsafe { self.quest_number.number }
    }

    pub fn episode(&self) -> u8 {
        unsafe { self.quest_number.number_and_episode.episode }
    }
}

#[derive(Debug)]
pub struct QuestBin {
    pub header: QuestBinHeader,
    pub object_code: Box<[u8]>,
    pub function_offset_table: Box<[u8]>,
}

impl QuestBin {
    pub fn from_compressed_bytes(bytes: &[u8]) -> Result<QuestBin, QuestBinError> {
        let decompressed = prs_decompress(&bytes)?;
        let mut reader = Cursor::new(decompressed);
        Ok(QuestBin::from_uncompressed_bytes(&mut reader)?)
    }

    pub fn from_uncompressed_bytes<T: ReadBytesExt>(
        reader: &mut T,
    ) -> Result<QuestBin, QuestBinError> {
        let object_code_offset = reader.read_u32::<LittleEndian>()?;
        if object_code_offset != QUEST_BIN_HEADER_SIZE as u32 {
            return Err(QuestBinError::DataFormatError(format!(
                "Invalid object_code_offset found: {}",
                object_code_offset
            )));
        }

        let function_offset_table_offset = reader.read_u32::<LittleEndian>()?;
        if function_offset_table_offset <= object_code_offset {
            return Err(QuestBinError::DataFormatError(format!(
                "function_offset_table_offset points to a location that occurs before the object_code"
            )));
        }

        let bin_size = reader.read_u32::<LittleEndian>()?;
        let _xfffffff = reader.read_u32::<LittleEndian>()?; // always expected to be 0xffffffff
        let is_download = reader.read_u8()?;
        let is_download = is_download != 0;

        let language = reader.read_u8()?;
        let language = match Language::from_number(language) {
            Err(e) => {
                return Err(QuestBinError::DataFormatError(format!(
                    "Unsupported language value found in quest header: {}",
                    e
                )))
            }
            Ok(encoding) => encoding,
        };

        let quest_number_and_episode = reader.read_u16::<LittleEndian>()?;
        let quest_number = QuestNumber {
            number: quest_number_and_episode,
        };

        let name_bytes: [u8; QUEST_BIN_NAME_LENGTH] = reader.read_bytes()?;
        let name = match language.decode_text(name_bytes.as_unpadded_slice()) {
            Err(e) => {
                return Err(QuestBinError::DataFormatError(format!(
                    "Error decoding string in quest 'name' field: {}",
                    e
                )))
            }
            Ok(value) => value,
        };

        let short_description_bytes: [u8; QUEST_BIN_SHORT_DESCRIPTION_LENGTH] =
            reader.read_bytes()?;
        let short_description =
            match language.decode_text(short_description_bytes.as_unpadded_slice()) {
                Err(e) => {
                    return Err(QuestBinError::DataFormatError(format!(
                        "Error decoding string in quest 'short_description' field: {}",
                        e
                    )))
                }
                Ok(value) => value,
            };

        let long_description_bytes: [u8; QUEST_BIN_LONG_DESCRIPTION_LENGTH] =
            reader.read_bytes()?;
        let long_description =
            match language.decode_text(long_description_bytes.as_unpadded_slice()) {
                Err(e) => {
                    return Err(QuestBinError::DataFormatError(format!(
                        "Error decoding string in quest 'long_description' field: {}",
                        e
                    )))
                }
                Ok(value) => value,
            };

        let mut object_code =
            vec![0u8; (function_offset_table_offset - object_code_offset) as usize];
        reader.read_exact(&mut object_code)?;

        let function_offset_table_size = bin_size - function_offset_table_offset;
        if function_offset_table_size % 4 != 0 {
            return Err(QuestBinError::DataFormatError(
                format!(
                    "Non-dword-sized data segment found in quest bin where function offset table is expected. Function offset table data size: {}",
                    function_offset_table_size
                )
            ));
        }
        let mut function_offset_table = vec![0u8; function_offset_table_size as usize];
        reader.read_exact(&mut function_offset_table)?;

        let bin = QuestBin {
            header: QuestBinHeader {
                is_download,
                language,
                quest_number,
                name,
                short_description,
                long_description,
            },
            object_code: object_code.into_boxed_slice(),
            function_offset_table: function_offset_table.into_boxed_slice(),
        };

        let our_bin_size = bin.calculate_size();
        if our_bin_size != bin_size as usize {
            return Err(QuestBinError::DataFormatError(format!(
                "bin_size value {} found in header does not match size of data actually read {}",
                bin_size, our_bin_size
            )));
        }

        Ok(bin)
    }

    pub fn from_compressed_file(path: &Path) -> Result<QuestBin, QuestBinError> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        QuestBin::from_compressed_bytes(&buffer)
    }

    pub fn from_uncompressed_file(path: &Path) -> Result<QuestBin, QuestBinError> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        Ok(QuestBin::from_uncompressed_bytes(&mut reader)?)
    }

    pub fn write_uncompressed_bytes<T: WriteBytesExt>(
        &self,
        writer: &mut T,
    ) -> Result<(), QuestBinError> {
        let bin_size = self.calculate_size();
        let object_code_offset = QUEST_BIN_HEADER_SIZE;
        let function_offset_table_offset = QUEST_BIN_HEADER_SIZE + self.object_code.len();

        writer.write_u32::<LittleEndian>(object_code_offset as u32)?;
        writer.write_u32::<LittleEndian>(function_offset_table_offset as u32)?;
        writer.write_u32::<LittleEndian>(bin_size as u32)?;
        writer.write_u32::<LittleEndian>(0xfffffff)?; // always 0xffffffff
        writer.write_u8(self.header.is_download as u8)?;
        writer.write_u8(self.header.language as u8)?;
        writer.write_u16::<LittleEndian>(unsafe { self.header.quest_number.number })?;

        let language = self.header.language;

        let name_bytes = match language.encode_text(&self.header.name) {
            Err(e) => {
                return Err(QuestBinError::DataFormatError(format!(
                    "Error encoding string for quest 'name' field: {}",
                    e
                )))
            }
            Ok(value) => value,
        };
        writer.write_all(&name_bytes.to_array::<QUEST_BIN_NAME_LENGTH>())?;

        let short_description_bytes = match language.encode_text(&self.header.short_description) {
            Err(e) => {
                return Err(QuestBinError::DataFormatError(format!(
                    "Error encoding string for quest 'short_description_bytes' field: {}",
                    e
                )))
            }
            Ok(value) => value,
        };
        writer
            .write_all(&short_description_bytes.to_array::<QUEST_BIN_SHORT_DESCRIPTION_LENGTH>())?;

        let long_description_bytes = match language.encode_text(&self.header.long_description) {
            Err(e) => {
                return Err(QuestBinError::DataFormatError(format!(
                    "Error encoding string for quest 'long_description_bytes' field: {}",
                    e
                )))
            }
            Ok(value) => value,
        };
        writer
            .write_all(&long_description_bytes.to_array::<QUEST_BIN_LONG_DESCRIPTION_LENGTH>())?;

        writer.write_all(self.object_code.as_ref())?;
        writer.write_all(self.function_offset_table.as_ref())?;

        Ok(())
    }

    pub fn to_compressed_file(&self, path: &Path) -> Result<(), QuestBinError> {
        let compressed_bytes = self.to_compressed_bytes()?;
        let mut file = File::create(path)?;
        file.write_all(compressed_bytes.as_ref())?;
        Ok(())
    }

    pub fn to_uncompressed_file(&self, path: &Path) -> Result<(), QuestBinError> {
        let mut file = File::create(path)?;
        self.write_uncompressed_bytes(&mut file)?;
        Ok(())
    }

    pub fn to_uncompressed_bytes(&self) -> Result<Box<[u8]>, QuestBinError> {
        let mut buffer = Cursor::new(Vec::<u8>::new());
        self.write_uncompressed_bytes(&mut buffer)?;
        Ok(buffer.into_inner().into_boxed_slice())
    }

    pub fn to_compressed_bytes(&self) -> Result<Box<[u8]>, QuestBinError> {
        let uncompressed = self.to_uncompressed_bytes()?;
        Ok(prs_compress(uncompressed.as_ref())?)
    }

    pub fn calculate_size(&self) -> usize {
        QUEST_BIN_HEADER_SIZE
            + self.object_code.as_ref().len()
            + self.function_offset_table.as_ref().len()
    }
}

#[cfg(test)]
pub mod tests {
    use claim::*;
    use rand::prelude::StdRng;
    use rand::{Fill, SeedableRng};
    use tempfile::TempDir;

    use super::*;

    pub fn validate_quest_58_bin(bin: &QuestBin) {
        assert_eq!(2000, bin.object_code.len());
        assert_eq!(4008, bin.function_offset_table.len());
        assert_eq!(6476, bin.calculate_size());

        assert_eq!(58, bin.header.quest_number());
        assert_eq!(0, bin.header.episode());
        assert_eq!(58, bin.header.quest_number_u16());

        assert_eq!(false, bin.header.is_download);
        assert_eq!(Language::Japanese, bin.header.language);

        assert_eq!("Lost HEAT SWORD", bin.header.name);
        assert_eq!(
            "Retrieve a\nweapon from\na Dragon!",
            bin.header.short_description
        );
        assert_eq!(
            "Client:  Hopkins, hunter\nQuest:\n My weapon was taken\n from me when I was\n fighting a Dragon.\nReward:  ??? Meseta\n\n\n",
            bin.header.long_description
        );
    }

    pub fn validate_quest_118_bin(bin: &QuestBin) {
        assert_eq!(32860, bin.object_code.len());
        assert_eq!(22004, bin.function_offset_table.len());
        assert_eq!(55332, bin.calculate_size());

        assert_eq!(118, bin.header.quest_number());
        assert_eq!(0, bin.header.episode());
        assert_eq!(118, bin.header.quest_number_u16());

        assert_eq!(false, bin.header.is_download);
        assert_eq!(Language::Japanese, bin.header.language);

        assert_eq!("Towards the Future", bin.header.name);
        assert_eq!(
            "Challenge the\nnew simulator.",
            bin.header.short_description
        );
        assert_eq!(
            "Client: Principal\nQuest: Wishes to have\nhunters challenge the\nnew simulator\nReward: ??? Meseta",
            bin.header.long_description
        );
    }

    #[test]
    pub fn read_compressed_quest_58_bin() -> Result<(), QuestBinError> {
        let path = Path::new("test-assets/q058-ret-gc.bin");
        let bin = QuestBin::from_compressed_file(&path)?;
        validate_quest_58_bin(&bin);
        Ok(())
    }

    #[test]
    pub fn write_compressed_quest_58_bin() -> Result<(), QuestBinError> {
        let data = include_bytes!("../../test-assets/q058-ret-gc.bin");
        let bin = QuestBin::from_compressed_bytes(data)?;
        let tmp_dir = TempDir::new()?;
        let bin_path = tmp_dir.path().join("quest58.bin");
        bin.to_compressed_file(&bin_path)?;
        let bin = QuestBin::from_compressed_file(&bin_path)?;
        validate_quest_58_bin(&bin);
        Ok(())
    }

    #[test]
    pub fn read_uncompressed_quest_58_bin() -> Result<(), QuestBinError> {
        let path = Path::new("test-assets/q058-ret-gc.uncompressed.bin");
        let bin = QuestBin::from_uncompressed_file(&path)?;
        validate_quest_58_bin(&bin);
        Ok(())
    }

    #[test]
    pub fn write_uncompressed_quest_58_bin() -> Result<(), QuestBinError> {
        let data = include_bytes!("../../test-assets/q058-ret-gc.bin");
        let bin = QuestBin::from_compressed_bytes(data)?;
        let tmp_dir = TempDir::new()?;
        let bin_path = tmp_dir.path().join("quest58.bin");
        bin.to_uncompressed_file(&bin_path)?;
        let bin = QuestBin::from_uncompressed_file(&bin_path)?;
        validate_quest_58_bin(&bin);
        Ok(())
    }

    #[test]
    pub fn read_compressed_quest_118_bin() -> Result<(), QuestBinError> {
        let path = Path::new("test-assets/q118-vr-gc.bin");
        let bin = QuestBin::from_compressed_file(&path)?;
        validate_quest_118_bin(&bin);
        Ok(())
    }

    #[test]
    pub fn write_compressed_quest_118_bin() -> Result<(), QuestBinError> {
        let data = include_bytes!("../../test-assets/q118-vr-gc.bin");
        let bin = QuestBin::from_compressed_bytes(data)?;
        let tmp_dir = TempDir::new()?;
        let bin_path = tmp_dir.path().join("quest118.bin");
        bin.to_compressed_file(&bin_path)?;
        let bin = QuestBin::from_compressed_file(&bin_path)?;
        validate_quest_118_bin(&bin);
        Ok(())
    }

    #[test]
    pub fn read_uncompressed_quest_118_bin() -> Result<(), QuestBinError> {
        let path = Path::new("test-assets/q118-vr-gc.uncompressed.bin");
        let bin = QuestBin::from_uncompressed_file(&path)?;
        validate_quest_118_bin(&bin);
        Ok(())
    }

    #[test]
    pub fn write_uncompressed_quest_118_bin() -> Result<(), QuestBinError> {
        let data = include_bytes!("../../test-assets/q118-vr-gc.bin");
        let bin = QuestBin::from_compressed_bytes(data)?;
        let tmp_dir = TempDir::new()?;
        let bin_path = tmp_dir.path().join("quest118.bin");
        bin.to_uncompressed_file(&bin_path)?;
        let bin = QuestBin::from_uncompressed_file(&bin_path)?;
        validate_quest_118_bin(&bin);
        Ok(())
    }

    #[test]
    pub fn error_on_load_from_zero_bytes() {
        let mut data: &[u8] = &[];
        assert_matches!(
            QuestBin::from_uncompressed_bytes(&mut data),
            Err(QuestBinError::IoError(..))
        );
        assert_matches!(
            QuestBin::from_compressed_bytes(&mut data),
            Err(QuestBinError::PrsCompressionError(..))
        );
    }

    #[test]
    pub fn error_on_load_from_garbage_bytes() {
        let mut data: &[u8] = b"This is definitely not a quest";
        assert_matches!(
            QuestBin::from_uncompressed_bytes(&mut data),
            Err(QuestBinError::DataFormatError(..))
        );
        assert_matches!(
            QuestBin::from_compressed_bytes(&mut data),
            Err(QuestBinError::PrsCompressionError(..))
        );
    }

    #[test]
    pub fn error_on_mostly_junk_header() {
        // the only correct part of this header is the object_code_offset
        let mut data = Vec::<u8>::new();
        data.write_u32::<LittleEndian>(QUEST_BIN_HEADER_SIZE as u32)
            .unwrap();
        data.write_all(b"This is also definitely not a quest")
            .unwrap();
        assert_matches!(
            QuestBin::from_uncompressed_bytes(&mut data.as_slice()),
            Err(QuestBinError::DataFormatError(..))
        );
    }

    #[test]
    pub fn error_on_header_with_bad_language_value() {
        // otherwise valid looking bin_header, but bad language value
        let mut data: &[u8] = &[
            0xD4, 0x01, 0x00, 0x00, // object_code_offset
            0xA4, 0x09, 0x00, 0x00, // function_offset_table_offset
            0x4C, 0x19, 0x00, 0x00, // bin_size
            0xFF, 0xFF, 0xFF, 0xFF, // xfffffff
            0x00, // is_download
            0x42, // language
            0x3A, 0x00, // quest_number_and_episode
        ];
        assert_matches!(
            QuestBin::from_uncompressed_bytes(&mut data),
            Err(QuestBinError::DataFormatError(..))
        );
    }

    #[test]
    pub fn error_on_header_with_garbage_text_fields() {
        // valid bin_header (so far) ...
        let header: &[u8] = &[
            0xD4, 0x01, 0x00, 0x00, // object_code_offset
            0xA4, 0x09, 0x00, 0x00, // function_offset_table_offset
            0x4C, 0x19, 0x00, 0x00, // bin_size
            0xFF, 0xFF, 0xFF, 0xFF, // xfffffff
            0x00, // is_download
            0x00, // language
            0x3A, 0x00, // quest_number_and_episode
        ];
        // ... and lets append random garbage to it.
        // this garbage will read as text fields (name, etc) and be decoded from shift jis which
        // will fail ...
        let mut random_garbage = [0u8; 4096];
        let mut rng = StdRng::seed_from_u64(123456);
        random_garbage.try_fill(&mut rng).unwrap();
        let data = [header, &random_garbage].concat();
        assert_matches!(
            QuestBin::from_uncompressed_bytes(&mut data.as_slice()),
            Err(QuestBinError::DataFormatError(..))
        );
    }

    #[test]
    pub fn error_on_bin_that_is_too_small() {
        // a complete valid bin_header ...
        let header: &[u8] = &[
            0xD4, 0x01, 0x00, 0x00, 0xA4, 0x09, 0x00, 0x00, 0x4C, 0x19, 0x00, 0x00, 0xFF, 0xFF,
            0xFF, 0xFF, 0x00, 0x00, 0x3A, 0x00, 0x4C, 0x6F, 0x73, 0x74, 0x20, 0x48, 0x45, 0x41,
            0x54, 0x20, 0x53, 0x57, 0x4F, 0x52, 0x44, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x52, 0x65, 0x74, 0x72,
            0x69, 0x65, 0x76, 0x65, 0x20, 0x61, 0x0A, 0x77, 0x65, 0x61, 0x70, 0x6F, 0x6E, 0x20,
            0x66, 0x72, 0x6F, 0x6D, 0x0A, 0x61, 0x20, 0x44, 0x72, 0x61, 0x67, 0x6F, 0x6E, 0x21,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43, 0x6C,
            0x69, 0x65, 0x6E, 0x74, 0x3A, 0x20, 0x20, 0x48, 0x6F, 0x70, 0x6B, 0x69, 0x6E, 0x73,
            0x2C, 0x20, 0x68, 0x75, 0x6E, 0x74, 0x65, 0x72, 0x0A, 0x51, 0x75, 0x65, 0x73, 0x74,
            0x3A, 0x0A, 0x20, 0x4D, 0x79, 0x20, 0x77, 0x65, 0x61, 0x70, 0x6F, 0x6E, 0x20, 0x77,
            0x61, 0x73, 0x20, 0x74, 0x61, 0x6B, 0x65, 0x6E, 0x0A, 0x20, 0x66, 0x72, 0x6F, 0x6D,
            0x20, 0x6D, 0x65, 0x20, 0x77, 0x68, 0x65, 0x6E, 0x20, 0x49, 0x20, 0x77, 0x61, 0x73,
            0x0A, 0x20, 0x66, 0x69, 0x67, 0x68, 0x74, 0x69, 0x6E, 0x67, 0x20, 0x61, 0x20, 0x44,
            0x72, 0x61, 0x67, 0x6F, 0x6E, 0x2E, 0x0A, 0x52, 0x65, 0x77, 0x61, 0x72, 0x64, 0x3A,
            0x20, 0x20, 0x3F, 0x3F, 0x3F, 0x20, 0x4D, 0x65, 0x73, 0x65, 0x74, 0x61, 0x0A, 0x0A,
            0x0A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        // ... and lets again append random garbage to it
        let mut random_garbage = [0u8; 4096];
        let mut rng = StdRng::seed_from_u64(35645128);
        random_garbage.try_fill(&mut rng).unwrap();
        let data = [header, &random_garbage].concat();
        // note that the only reason this fails is because the buffer we've provided is not large
        // enough. this call would return successfully if random_garbage was large enough (based on
        // the sizes loaded from the bin_header) since we don't parse/validate either the
        // object_code or function_offset_table data
        assert_matches!(
            QuestBin::from_uncompressed_bytes(&mut data.as_slice()),
            Err(QuestBinError::IoError(..))
        );
    }

    #[test]
    pub fn error_on_non_dword_sized_function_offset_table() {
        // i don't think this scenario being tested is really important ... it would mean something
        // is generating garbage function_offset_tables to begin with ...

        // a complete valid bin_header ...
        let header: &[u8] = &[
            0xD4, 0x01, 0x00, 0x00, 0xA4, 0x09, 0x00, 0x00, 0x4A, 0x19, 0x00, 0x00, 0xFF, 0xFF,
            0xFF, 0xFF, 0x00, 0x00, 0x3A, 0x00, 0x4C, 0x6F, 0x73, 0x74, 0x20, 0x48, 0x45, 0x41,
            0x54, 0x20, 0x53, 0x57, 0x4F, 0x52, 0x44, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x52, 0x65, 0x74, 0x72,
            0x69, 0x65, 0x76, 0x65, 0x20, 0x61, 0x0A, 0x77, 0x65, 0x61, 0x70, 0x6F, 0x6E, 0x20,
            0x66, 0x72, 0x6F, 0x6D, 0x0A, 0x61, 0x20, 0x44, 0x72, 0x61, 0x67, 0x6F, 0x6E, 0x21,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x43, 0x6C,
            0x69, 0x65, 0x6E, 0x74, 0x3A, 0x20, 0x20, 0x48, 0x6F, 0x70, 0x6B, 0x69, 0x6E, 0x73,
            0x2C, 0x20, 0x68, 0x75, 0x6E, 0x74, 0x65, 0x72, 0x0A, 0x51, 0x75, 0x65, 0x73, 0x74,
            0x3A, 0x0A, 0x20, 0x4D, 0x79, 0x20, 0x77, 0x65, 0x61, 0x70, 0x6F, 0x6E, 0x20, 0x77,
            0x61, 0x73, 0x20, 0x74, 0x61, 0x6B, 0x65, 0x6E, 0x0A, 0x20, 0x66, 0x72, 0x6F, 0x6D,
            0x20, 0x6D, 0x65, 0x20, 0x77, 0x68, 0x65, 0x6E, 0x20, 0x49, 0x20, 0x77, 0x61, 0x73,
            0x0A, 0x20, 0x66, 0x69, 0x67, 0x68, 0x74, 0x69, 0x6E, 0x67, 0x20, 0x61, 0x20, 0x44,
            0x72, 0x61, 0x67, 0x6F, 0x6E, 0x2E, 0x0A, 0x52, 0x65, 0x77, 0x61, 0x72, 0x64, 0x3A,
            0x20, 0x20, 0x3F, 0x3F, 0x3F, 0x20, 0x4D, 0x65, 0x73, 0x65, 0x74, 0x61, 0x0A, 0x0A,
            0x0A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let object_code_data = [0u8; 2000];
        let function_offset_table_data = [0u8; 4006]; // size here matches header above, but it is still a bad size
        let data = [header, &object_code_data, &function_offset_table_data].concat();

        assert_matches!(
            QuestBin::from_uncompressed_bytes(&mut data.as_slice()),
            Err(QuestBinError::DataFormatError(..))
        );
    }
}
