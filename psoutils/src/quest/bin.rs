use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::bytes::*;
use crate::compression::{prs_compress, prs_decompress};
use crate::quest::QuestError;
use crate::text::Language;

pub const QUEST_BIN_NAME_LENGTH: usize = 32;
pub const QUEST_BIN_SHORT_DESCRIPTION_LENGTH: usize = 128;
pub const QUEST_BIN_LONG_DESCRIPTION_LENGTH: usize = 288;

pub const QUEST_BIN_HEADER_SIZE: usize = 20
    + QUEST_BIN_NAME_LENGTH
    + QUEST_BIN_SHORT_DESCRIPTION_LENGTH
    + QUEST_BIN_LONG_DESCRIPTION_LENGTH;

#[derive(Copy, Clone)]
pub struct QuestNumberAndEpisode {
    pub number: u8,
    pub episode: u8,
}

pub union QuestNumber {
    pub number_and_episode: QuestNumberAndEpisode,
    pub number: u16,
}

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

pub struct QuestBin {
    pub header: QuestBinHeader,
    pub object_code: Box<[u8]>,
    pub function_offset_table: Box<[u8]>,
}

impl QuestBin {
    pub fn from_compressed_bytes(bytes: &[u8]) -> Result<QuestBin, QuestError> {
        let decompressed = prs_decompress(&bytes);
        let mut reader = Cursor::new(decompressed);
        Ok(QuestBin::read_from_bytes(&mut reader)?)
    }

    pub fn from_compressed_file(path: &Path) -> Result<QuestBin, QuestError> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        QuestBin::from_compressed_bytes(&buffer)
    }

    pub fn from_uncompressed_file(path: &Path) -> Result<QuestBin, QuestError> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        Ok(QuestBin::read_from_bytes(&mut reader)?)
    }

    pub fn to_compressed_bytes(&self) -> Result<Box<[u8]>, WriteBytesError> {
        let bytes = self.to_uncompressed_bytes()?;
        Ok(prs_compress(bytes.as_ref()))
    }

    pub fn to_uncompressed_bytes(&self) -> Result<Box<[u8]>, WriteBytesError> {
        let mut bytes = Cursor::new(Vec::new());
        self.write_as_bytes(&mut bytes)?;
        Ok(bytes.into_inner().into_boxed_slice())
    }

    pub fn calculate_size(&self) -> usize {
        QUEST_BIN_HEADER_SIZE
            + self.object_code.as_ref().len()
            + self.function_offset_table.as_ref().len()
    }
}

impl<T: ReadBytesExt> ReadFromBytes<T> for QuestBin {
    fn read_from_bytes(reader: &mut T) -> Result<Self, ReadBytesError> {
        let object_code_offset = reader.read_u32::<LittleEndian>()?;
        let function_offset_table_offset = reader.read_u32::<LittleEndian>()?;
        let bin_size = reader.read_u32::<LittleEndian>()?;
        let _xfffffff = reader.read_u32::<LittleEndian>()?; // always 0xffffffff
        let is_download = reader.read_u8()?;
        let language = reader.read_u8()?;
        let quest_number_and_episode = reader.read_u16::<LittleEndian>()?;

        let is_download = if is_download == 0 { false } else { true };
        let quest_number = QuestNumber {
            number: quest_number_and_episode,
        };

        let language = match Language::from_number(language) {
            Err(e) => {
                return Err(ReadBytesError::UnexpectedError(format!(
                    "Unsupported language value found in quest header: {}",
                    e
                )))
            }
            Ok(encoding) => encoding,
        };

        let mut name_bytes = [0u8; QUEST_BIN_NAME_LENGTH];
        reader.read_exact(&mut name_bytes)?;
        let name = match language.decode_text(name_bytes.as_unpadded_slice()) {
            Err(e) => {
                return Err(ReadBytesError::UnexpectedError(format!(
                    "Error decoding string in quest 'name' field: {}",
                    e
                )))
            }
            Ok(value) => value,
        };

        let mut short_description_bytes = [0u8; QUEST_BIN_SHORT_DESCRIPTION_LENGTH];
        reader.read_exact(&mut short_description_bytes)?;
        let short_description =
            match language.decode_text(short_description_bytes.as_unpadded_slice()) {
                Err(e) => {
                    return Err(ReadBytesError::UnexpectedError(format!(
                        "Error decoding string in quest 'short_description' field: {}",
                        e
                    )))
                }
                Ok(value) => value,
            };

        let mut long_description_bytes = [0u8; QUEST_BIN_LONG_DESCRIPTION_LENGTH];
        reader.read_exact(&mut long_description_bytes)?;
        let long_description =
            match language.decode_text(long_description_bytes.as_unpadded_slice()) {
                Err(e) => {
                    return Err(ReadBytesError::UnexpectedError(format!(
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
            return Err(ReadBytesError::UnexpectedError(String::from("Non-dword-sized bytes found in quest bin where function offset table is expected (probably a PRS decompression issue?)")));
        }
        let mut function_offset_table = vec![0u8; function_offset_table_size as usize];
        reader.read_exact(&mut function_offset_table)?;

        Ok(QuestBin {
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
        })
    }
}

impl<T: WriteBytesExt> WriteAsBytes<T> for QuestBin {
    fn write_as_bytes(&self, writer: &mut T) -> Result<(), WriteBytesError> {
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
                return Err(WriteBytesError::UnexpectedError(format!(
                    "Error encoding string for quest 'name' field: {}",
                    e
                )))
            }
            Ok(value) => value,
        };
        writer.write_all(&name_bytes.to_fixed_length(QUEST_BIN_NAME_LENGTH))?;

        let short_description_bytes = match language.encode_text(&self.header.short_description) {
            Err(e) => {
                return Err(WriteBytesError::UnexpectedError(format!(
                    "Error encoding string for quest 'short_description_bytes' field: {}",
                    e
                )))
            }
            Ok(value) => value,
        };
        writer.write_all(
            &short_description_bytes.to_fixed_length(QUEST_BIN_SHORT_DESCRIPTION_LENGTH),
        )?;

        let long_description_bytes = match language.encode_text(&self.header.long_description) {
            Err(e) => {
                return Err(WriteBytesError::UnexpectedError(format!(
                    "Error encoding string for quest 'long_description_bytes' field: {}",
                    e
                )))
            }
            Ok(value) => value,
        };
        writer.write_all(
            &long_description_bytes.to_fixed_length(QUEST_BIN_LONG_DESCRIPTION_LENGTH),
        )?;

        writer.write_all(self.object_code.as_ref())?;
        writer.write_all(self.function_offset_table.as_ref())?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn validate_quest_58_bin(bin: &QuestBin) {
        assert_eq!(2000, bin.object_code.len());
        assert_eq!(4008, bin.function_offset_table.len());
        assert_eq!(6476, bin.calculate_size());

        assert_eq!(58, unsafe { bin.header.quest_number.number });
        assert_eq!(0, unsafe {
            bin.header.quest_number.number_and_episode.episode
        });
        assert_eq!(58, unsafe {
            bin.header.quest_number.number_and_episode.number
        });

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

        assert_eq!(118, unsafe { bin.header.quest_number.number });
        assert_eq!(0, unsafe {
            bin.header.quest_number.number_and_episode.episode
        });
        assert_eq!(118, unsafe {
            bin.header.quest_number.number_and_episode.number
        });

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
    pub fn read_compressed_quest_58_bin() -> Result<(), QuestError> {
        let path = Path::new("assets/test/q058-ret-gc.bin");
        let bin = QuestBin::from_compressed_file(&path)?;
        validate_quest_58_bin(&bin);
        Ok(())
    }

    #[test]
    pub fn read_uncompressed_quest_58_bin() -> Result<(), QuestError> {
        let path = Path::new("assets/test/q058-ret-gc.uncompressed.bin");
        let bin = QuestBin::from_uncompressed_file(&path)?;
        validate_quest_58_bin(&bin);
        Ok(())
    }

    #[test]
    pub fn read_compressed_quest_118_bin() -> Result<(), QuestError> {
        let path = Path::new("assets/test/q118-vr-gc.bin");
        let bin = QuestBin::from_compressed_file(&path)?;
        validate_quest_118_bin(&bin);
        Ok(())
    }

    #[test]
    pub fn read_uncompressed_quest_118_bin() -> Result<(), QuestError> {
        let path = Path::new("assets/test/q118-vr-gc.uncompressed.bin");
        let bin = QuestBin::from_uncompressed_file(&path)?;
        validate_quest_118_bin(&bin);
        Ok(())
    }
}
