use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Write};
use std::path::Path;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use thiserror::Error;

use crate::compression::{prs_compress, prs_decompress, PrsCompressionError};

pub const QUEST_DAT_TABLE_HEADER_SIZE: usize = 16;

pub const QUEST_DAT_AREAS: [[&str; 18]; 2] = [
    [
        "Pioneer 2",
        "Forest 1",
        "Forest 2",
        "Caves 1",
        "Caves 2",
        "Caves 3",
        "Mines 1",
        "Mines 2",
        "Ruins 1",
        "Ruins 2",
        "Ruins 3",
        "Under the Dome",
        "Underground Channel",
        "Monitor Room",
        "????",
        "Visual Lobby",
        "VR Spaceship Alpha",
        "VR Temple Alpha",
    ],
    [
        "Lab",
        "VR Temple Alpha",
        "VR Temple Beta",
        "VR Spaceship Alpha",
        "VR Spaceship Beta",
        "Central Control Area",
        "Jungle North",
        "Jungle East",
        "Mountain",
        "Seaside",
        "Seabed Upper",
        "Seabed Lower",
        "Cliffs of Gal Da Val",
        "Test Subject Disposal Area",
        "VR Temple Final",
        "VR Spaceship Final",
        "Seaside Night",
        "Control Tower",
    ],
];

#[derive(Error, Debug)]
pub enum QuestDatError {
    #[error("I/O error while processing quest dat")]
    IoError(#[from] std::io::Error),

    #[error("PRS compression failed")]
    PrsCompressionError(#[from] PrsCompressionError),

    #[error("Bad quest dat data format: {0}")]
    DataFormatError(String),
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum QuestDatTableType {
    Object,
    NPC,
    Wave,
    ChallengeModeSpawns,
    ChallengeModeUnknown,
    Unknown(u32),
}

impl Display for QuestDatTableType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use QuestDatTableType::*;
        match self {
            Object => write!(f, "Object"),
            NPC => write!(f, "NPC"),
            Wave => write!(f, "Wave"),
            ChallengeModeSpawns => write!(f, "Challenge Mode Spawns"),
            ChallengeModeUnknown => write!(f, "Challenge Mode Unknown"),
            Unknown(n) => write!(f, "Unknown value ({})", n),
        }
    }
}

impl From<u32> for QuestDatTableType {
    fn from(value: u32) -> Self {
        // TODO: is there some way to cast an int back to an enum?
        use QuestDatTableType::*;
        match value {
            1 => Object,
            2 => NPC,
            3 => Wave,
            4 => ChallengeModeSpawns,
            5 => ChallengeModeUnknown,
            n => Unknown(n),
        }
    }
}

impl From<&QuestDatTableType> for u32 {
    fn from(value: &QuestDatTableType) -> Self {
        use QuestDatTableType::*;
        match *value {
            Object => 1,
            NPC => 2,
            Wave => 3,
            ChallengeModeSpawns => 4,
            ChallengeModeUnknown => 5,
            Unknown(n) => n,
        }
    }
}

#[derive(Debug)]
pub struct QuestDatTableHeader {
    pub table_type: QuestDatTableType,
    pub area: u32,
}

#[derive(Debug)]
pub struct QuestDatTable {
    pub header: QuestDatTableHeader,
    pub bytes: Box<[u8]>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum QuestArea {
    Area(&'static str),
    InvalidArea(u32),
    InvalidEpisode(u32),
}

impl Display for QuestArea {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use QuestArea::*;
        match self {
            Area(s) => write!(f, "{}", s),
            InvalidArea(n) => write!(f, "Invalid Area: {}", n),
            InvalidEpisode(n) => write!(f, "Invalid Episode: {}", n),
        }
    }
}

impl QuestDatTable {
    pub fn table_type(&self) -> QuestDatTableType {
        self.header.table_type
    }

    pub fn area_name(&self, episode: u32) -> QuestArea {
        use QuestArea::*;
        match QUEST_DAT_AREAS.get(episode as usize) {
            Some(list) => match list.get(self.header.area as usize) {
                Some(area) => Area(area),
                None => InvalidArea(self.header.area),
            },
            None => InvalidEpisode(episode),
        }
    }

    pub fn calculate_size(&self) -> usize {
        QUEST_DAT_TABLE_HEADER_SIZE + self.bytes.as_ref().len()
    }

    fn body_size(&self) -> usize {
        self.bytes.as_ref().len()
    }
}

#[derive(Debug)]
pub struct QuestDat {
    pub tables: Box<[QuestDatTable]>,
}

impl QuestDat {
    pub fn from_compressed_bytes(bytes: &[u8]) -> Result<QuestDat, QuestDatError> {
        let decompressed = prs_decompress(&bytes)?;
        let mut reader = Cursor::new(decompressed);
        Ok(QuestDat::from_uncompressed_bytes(&mut reader)?)
    }

    pub fn from_uncompressed_bytes<T: ReadBytesExt>(
        reader: &mut T,
    ) -> Result<QuestDat, QuestDatError> {
        let mut tables = Vec::new();
        let mut index = 0;
        loop {
            let table_type = reader.read_u32::<LittleEndian>()?;
            let table_size = reader.read_u32::<LittleEndian>()?;
            let area = reader.read_u32::<LittleEndian>()?;
            let table_body_size = reader.read_u32::<LittleEndian>()?;

            // quest .dat files appear to always use a "zero-table" to mark the end of the file
            if table_type == 0 && table_size == 0 && area == 0 && table_body_size == 0 {
                break;
            }

            if table_size != table_body_size.wrapping_add(QUEST_DAT_TABLE_HEADER_SIZE as u32) {
                return Err(QuestDatError::DataFormatError(format!(
                    "Malformed table at index {}. table_size != table_body_size + 16",
                    index
                )));
            }

            let table_type: QuestDatTableType = match table_type.into() {
                QuestDatTableType::Unknown(n) => {
                    return Err(QuestDatError::DataFormatError(format!(
                        "Invalid table_type {} for table at index {}",
                        n, index
                    )))
                }
                otherwise => otherwise,
            };

            // note: both episode area lists are the same size
            if area >= QUEST_DAT_AREAS[0].len() as u32 {
                return Err(QuestDatError::DataFormatError(format!(
                    "Invalid area {} for table at index {}",
                    area, index
                )));
            }

            let mut body_bytes = vec![0u8; table_body_size as usize];
            reader.read_exact(&mut body_bytes)?;

            tables.push(QuestDatTable {
                header: QuestDatTableHeader { table_type, area },
                bytes: body_bytes.into_boxed_slice(),
            });

            index += 1;
        }

        // i wrote this check thinking that an empty .dat file is the most useless thing ever,
        // but maybe it is possible to exist in a legitimate quest for a totally script-driven
        // "quest" ... e.g. some sort of "utility quest" that exists just for the purpose of letting
        // a user interact with the script stored in the .bin? dunno really, but i guess i might
        // as well disable this check ...
        //
        //if tables.len() == 0 {
        //    return Err(QuestDatError::DataFormatError(String::from(
        //        "no tables found, probably not a .dat file?",
        //    )));
        //}

        Ok(QuestDat {
            tables: tables.into_boxed_slice(),
        })
    }

    pub fn from_compressed_file(path: &Path) -> Result<QuestDat, QuestDatError> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        QuestDat::from_compressed_bytes(&buffer)
    }

    pub fn from_uncompressed_file(path: &Path) -> Result<QuestDat, QuestDatError> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        Ok(QuestDat::from_uncompressed_bytes(&mut reader)?)
    }

    pub fn write_uncompressed_bytes<T: WriteBytesExt>(
        &self,
        writer: &mut T,
    ) -> Result<(), QuestDatError> {
        for table in self.tables.iter() {
            let table_size = table.calculate_size() as u32;
            let table_body_size = table.body_size() as u32;

            writer.write_u32::<LittleEndian>((&table.header.table_type).into())?;
            writer.write_u32::<LittleEndian>(table_size)?;
            writer.write_u32::<LittleEndian>(table.header.area)?;
            writer.write_u32::<LittleEndian>(table_body_size)?;

            writer.write_all(table.bytes.as_ref())?;
        }

        // write "zero table" at eof. this seems to be a convention used everywhere for quest .dat
        writer.write_u32::<LittleEndian>(0)?; // table_type
        writer.write_u32::<LittleEndian>(0)?; // table_size
        writer.write_u32::<LittleEndian>(0)?; // area
        writer.write_u32::<LittleEndian>(0)?; // table_body_size

        Ok(())
    }

    pub fn to_compressed_file(&self, path: &Path) -> Result<(), QuestDatError> {
        let compressed_bytes = self.to_compressed_bytes()?;
        let mut file = File::create(path)?;
        file.write_all(compressed_bytes.as_ref())?;
        Ok(())
    }

    pub fn to_uncompressed_file(&self, path: &Path) -> Result<(), QuestDatError> {
        let mut file = File::create(path)?;
        self.write_uncompressed_bytes(&mut file)?;
        Ok(())
    }

    pub fn to_uncompressed_bytes(&self) -> Result<Box<[u8]>, QuestDatError> {
        let mut buffer = Cursor::new(Vec::<u8>::new());
        self.write_uncompressed_bytes(&mut buffer)?;
        Ok(buffer.into_inner().into_boxed_slice())
    }

    pub fn to_compressed_bytes(&self) -> Result<Box<[u8]>, QuestDatError> {
        let uncompressed = self.to_uncompressed_bytes()?;
        Ok(prs_compress(uncompressed.as_ref())?)
    }

    pub fn calculate_size(&self) -> usize {
        self.tables
            .iter()
            .map(|table| QUEST_DAT_TABLE_HEADER_SIZE + table.body_size() as usize)
            .sum()
    }
}

#[cfg(test)]
pub mod tests {
    use claim::*;
    use rand::prelude::StdRng;
    use rand::{Fill, SeedableRng};
    use tempfile::TempDir;

    use super::*;

    pub fn validate_quest_58_dat(dat: &QuestDat) {
        let episode = 0;

        assert_eq!(11, dat.tables.len());

        let table = &dat.tables[0];
        assert_eq!(QuestDatTableType::Object, table.table_type());
        assert_eq!(2260, table.calculate_size());
        assert_eq!(2244, table.body_size());
        assert_eq!(QuestArea::Area("Pioneer 2"), table.area_name(episode));

        let table = &dat.tables[1];
        assert_eq!(QuestDatTableType::Object, table.table_type());
        assert_eq!(7020, table.calculate_size());
        assert_eq!(7004, table.body_size());
        assert_eq!(QuestArea::Area("Forest 1"), table.area_name(episode));

        let table = &dat.tables[2];
        assert_eq!(QuestDatTableType::Object, table.table_type());
        assert_eq!(9536, table.calculate_size());
        assert_eq!(9520, table.body_size());
        assert_eq!(QuestArea::Area("Forest 2"), table.area_name(episode));

        let table = &dat.tables[3];
        assert_eq!(QuestDatTableType::Object, table.table_type());
        assert_eq!(1376, table.calculate_size());
        assert_eq!(1360, table.body_size());
        assert_eq!(QuestArea::Area("Under the Dome"), table.area_name(episode));

        let table = &dat.tables[4];
        assert_eq!(QuestDatTableType::NPC, table.table_type());
        assert_eq!(1672, table.calculate_size());
        assert_eq!(1656, table.body_size());
        assert_eq!(QuestArea::Area("Pioneer 2"), table.area_name(episode));

        let table = &dat.tables[5];
        assert_eq!(QuestDatTableType::NPC, table.table_type());
        assert_eq!(6064, table.calculate_size());
        assert_eq!(6048, table.body_size());
        assert_eq!(QuestArea::Area("Forest 1"), table.area_name(episode));

        let table = &dat.tables[6];
        assert_eq!(QuestDatTableType::NPC, table.table_type());
        assert_eq!(7432, table.calculate_size());
        assert_eq!(7416, table.body_size());
        assert_eq!(QuestArea::Area("Forest 2"), table.area_name(episode));

        let table = &dat.tables[7];
        assert_eq!(QuestDatTableType::NPC, table.table_type());
        assert_eq!(88, table.calculate_size());
        assert_eq!(72, table.body_size());
        assert_eq!(QuestArea::Area("Under the Dome"), table.area_name(episode));

        let table = &dat.tables[8];
        assert_eq!(QuestDatTableType::Wave, table.table_type());
        assert_eq!(560, table.calculate_size());
        assert_eq!(544, table.body_size());
        assert_eq!(QuestArea::Area("Forest 1"), table.area_name(episode));

        let table = &dat.tables[9];
        assert_eq!(QuestDatTableType::Wave, table.table_type());
        assert_eq!(736, table.calculate_size());
        assert_eq!(720, table.body_size());
        assert_eq!(QuestArea::Area("Forest 2"), table.area_name(episode));

        let table = &dat.tables[10];
        assert_eq!(QuestDatTableType::Wave, table.table_type());
        assert_eq!(60, table.calculate_size());
        assert_eq!(44, table.body_size());
        assert_eq!(QuestArea::Area("Under the Dome"), table.area_name(episode));
    }

    pub fn validate_quest_118_dat(dat: &QuestDat) {
        let episode = 0;

        assert_eq!(25, dat.tables.len());

        let table = &dat.tables[0];
        assert_eq!(QuestDatTableType::Object, table.table_type());
        assert_eq!(1988, table.calculate_size());
        assert_eq!(1972, table.body_size());
        assert_eq!(QuestArea::Area("Forest 2"), table.area_name(episode));

        let table = &dat.tables[1];
        assert_eq!(QuestDatTableType::Object, table.table_type());
        assert_eq!(2872, table.calculate_size());
        assert_eq!(2856, table.body_size());
        assert_eq!(QuestArea::Area("Caves 3"), table.area_name(episode));

        let table = &dat.tables[2];
        assert_eq!(QuestDatTableType::Object, table.table_type());
        assert_eq!(2532, table.calculate_size());
        assert_eq!(2516, table.body_size());
        assert_eq!(QuestArea::Area("Mines 2"), table.area_name(episode));

        let table = &dat.tables[3];
        assert_eq!(QuestDatTableType::Object, table.table_type());
        assert_eq!(2668, table.calculate_size());
        assert_eq!(2652, table.body_size());
        assert_eq!(QuestArea::Area("Ruins 3"), table.area_name(episode));

        let table = &dat.tables[4];
        assert_eq!(QuestDatTableType::Object, table.table_type());
        assert_eq!(1580, table.calculate_size());
        assert_eq!(1564, table.body_size());
        assert_eq!(QuestArea::Area("Under the Dome"), table.area_name(episode));

        let table = &dat.tables[5];
        assert_eq!(QuestDatTableType::Object, table.table_type());
        assert_eq!(1104, table.calculate_size());
        assert_eq!(1088, table.body_size());
        assert_eq!(
            QuestArea::Area("Underground Channel"),
            table.area_name(episode)
        );

        let table = &dat.tables[6];
        assert_eq!(QuestDatTableType::Object, table.table_type());
        assert_eq!(2056, table.calculate_size());
        assert_eq!(2040, table.body_size());
        assert_eq!(QuestArea::Area("Monitor Room"), table.area_name(episode));

        let table = &dat.tables[7];
        assert_eq!(QuestDatTableType::Object, table.table_type());
        assert_eq!(2396, table.calculate_size());
        assert_eq!(2380, table.body_size());
        assert_eq!(QuestArea::Area("????"), table.area_name(episode));

        let table = &dat.tables[8];
        assert_eq!(QuestDatTableType::Object, table.table_type());
        assert_eq!(1784, table.calculate_size());
        assert_eq!(1768, table.body_size());
        assert_eq!(QuestArea::Area("Pioneer 2"), table.area_name(episode));

        let table = &dat.tables[9];
        assert_eq!(QuestDatTableType::NPC, table.table_type());
        assert_eq!(1528, table.calculate_size());
        assert_eq!(1512, table.body_size());
        assert_eq!(QuestArea::Area("Pioneer 2"), table.area_name(episode));

        let table = &dat.tables[10];
        assert_eq!(QuestDatTableType::NPC, table.table_type());
        assert_eq!(2392, table.calculate_size());
        assert_eq!(2376, table.body_size());
        assert_eq!(QuestArea::Area("Forest 2"), table.area_name(episode));

        let table = &dat.tables[11];
        assert_eq!(QuestDatTableType::NPC, table.table_type());
        assert_eq!(3760, table.calculate_size());
        assert_eq!(3744, table.body_size());
        assert_eq!(QuestArea::Area("Caves 3"), table.area_name(episode));

        let table = &dat.tables[12];
        assert_eq!(QuestDatTableType::NPC, table.table_type());
        assert_eq!(4480, table.calculate_size());
        assert_eq!(4464, table.body_size());
        assert_eq!(QuestArea::Area("Mines 2"), table.area_name(episode));

        let table = &dat.tables[13];
        assert_eq!(QuestDatTableType::NPC, table.table_type());
        assert_eq!(3256, table.calculate_size());
        assert_eq!(3240, table.body_size());
        assert_eq!(QuestArea::Area("Ruins 3"), table.area_name(episode));

        let table = &dat.tables[14];
        assert_eq!(QuestDatTableType::NPC, table.table_type());
        assert_eq!(88, table.calculate_size());
        assert_eq!(72, table.body_size());
        assert_eq!(QuestArea::Area("Under the Dome"), table.area_name(episode));

        let table = &dat.tables[15];
        assert_eq!(QuestDatTableType::NPC, table.table_type());
        assert_eq!(88, table.calculate_size());
        assert_eq!(72, table.body_size());
        assert_eq!(
            QuestArea::Area("Underground Channel"),
            table.area_name(episode)
        );

        let table = &dat.tables[16];
        assert_eq!(QuestDatTableType::NPC, table.table_type());
        assert_eq!(160, table.calculate_size());
        assert_eq!(144, table.body_size());
        assert_eq!(QuestArea::Area("Monitor Room"), table.area_name(episode));

        let table = &dat.tables[17];
        assert_eq!(QuestDatTableType::NPC, table.table_type());
        assert_eq!(88, table.calculate_size());
        assert_eq!(72, table.body_size());
        assert_eq!(QuestArea::Area("????"), table.area_name(episode));

        let table = &dat.tables[18];
        assert_eq!(QuestDatTableType::Wave, table.table_type());
        assert_eq!(232, table.calculate_size());
        assert_eq!(216, table.body_size());
        assert_eq!(QuestArea::Area("Forest 2"), table.area_name(episode));

        let table = &dat.tables[19];
        assert_eq!(QuestDatTableType::Wave, table.table_type());
        assert_eq!(532, table.calculate_size());
        assert_eq!(516, table.body_size());
        assert_eq!(QuestArea::Area("Caves 3"), table.area_name(episode));

        let table = &dat.tables[20];
        assert_eq!(QuestDatTableType::Wave, table.table_type());
        assert_eq!(768, table.calculate_size());
        assert_eq!(752, table.body_size());
        assert_eq!(QuestArea::Area("Mines 2"), table.area_name(episode));

        let table = &dat.tables[21];
        assert_eq!(QuestDatTableType::Wave, table.table_type());
        assert_eq!(368, table.calculate_size());
        assert_eq!(352, table.body_size());
        assert_eq!(QuestArea::Area("Ruins 3"), table.area_name(episode));

        let table = &dat.tables[22];
        assert_eq!(QuestDatTableType::Wave, table.table_type());
        assert_eq!(60, table.calculate_size());
        assert_eq!(44, table.body_size());
        assert_eq!(QuestArea::Area("Under the Dome"), table.area_name(episode));

        let table = &dat.tables[23];
        assert_eq!(QuestDatTableType::Wave, table.table_type());
        assert_eq!(60, table.calculate_size());
        assert_eq!(44, table.body_size());
        assert_eq!(
            QuestArea::Area("Underground Channel"),
            table.area_name(episode)
        );

        let table = &dat.tables[24];
        assert_eq!(QuestDatTableType::Wave, table.table_type());
        assert_eq!(68, table.calculate_size());
        assert_eq!(52, table.body_size());
        assert_eq!(QuestArea::Area("????"), table.area_name(episode));
    }

    #[test]
    pub fn read_compressed_quest_58_dat() -> Result<(), QuestDatError> {
        let path = Path::new("../test-assets/q058-ret-gc.dat");
        let dat = QuestDat::from_compressed_file(&path)?;
        validate_quest_58_dat(&dat);
        Ok(())
    }

    #[test]
    pub fn write_compressed_quest_58_dat() -> Result<(), QuestDatError> {
        let data = include_bytes!("../../../test-assets/q058-ret-gc.dat");
        let dat = QuestDat::from_compressed_bytes(data)?;
        let tmp_dir = TempDir::new()?;
        let dat_path = tmp_dir.path().join("quest58.dat");
        dat.to_compressed_file(&dat_path)?;
        let dat = QuestDat::from_compressed_file(&dat_path)?;
        validate_quest_58_dat(&dat);
        Ok(())
    }

    #[test]
    pub fn read_uncompressed_quest_58_dat() -> Result<(), QuestDatError> {
        let path = Path::new("../test-assets/q058-ret-gc.uncompressed.dat");
        let dat = QuestDat::from_uncompressed_file(&path)?;
        validate_quest_58_dat(&dat);
        Ok(())
    }

    #[test]
    pub fn write_uncompressed_quest_58_dat() -> Result<(), QuestDatError> {
        let data = include_bytes!("../../../test-assets/q058-ret-gc.dat");
        let dat = QuestDat::from_compressed_bytes(data)?;
        let tmp_dir = TempDir::new()?;
        let dat_path = tmp_dir.path().join("quest58.dat");
        dat.to_uncompressed_file(&dat_path)?;
        let dat = QuestDat::from_uncompressed_file(&dat_path)?;
        validate_quest_58_dat(&dat);
        Ok(())
    }

    #[test]
    pub fn read_compressed_quest_118_dat() -> Result<(), QuestDatError> {
        let path = Path::new("../test-assets/q118-vr-gc.dat");
        let dat = QuestDat::from_compressed_file(&path)?;
        validate_quest_118_dat(&dat);
        Ok(())
    }

    #[test]
    pub fn write_compressed_quest_118_dat() -> Result<(), QuestDatError> {
        let data = include_bytes!("../../../test-assets/q118-vr-gc.dat");
        let dat = QuestDat::from_compressed_bytes(data)?;
        let tmp_dir = TempDir::new()?;
        let dat_path = tmp_dir.path().join("quest118.dat");
        dat.to_compressed_file(&dat_path)?;
        let dat = QuestDat::from_compressed_file(&dat_path)?;
        validate_quest_118_dat(&dat);
        Ok(())
    }

    #[test]
    pub fn read_uncompressed_quest_118_dat() -> Result<(), QuestDatError> {
        let path = Path::new("../test-assets/q118-vr-gc.uncompressed.dat");
        let dat = QuestDat::from_uncompressed_file(&path)?;
        validate_quest_118_dat(&dat);
        Ok(())
    }

    #[test]
    pub fn write_uncompressed_quest_118_dat() -> Result<(), QuestDatError> {
        let data = include_bytes!("../../../test-assets/q118-vr-gc.dat");
        let dat = QuestDat::from_compressed_bytes(data)?;
        let tmp_dir = TempDir::new()?;
        let dat_path = tmp_dir.path().join("quest118.dat");
        dat.to_uncompressed_file(&dat_path)?;
        let dat = QuestDat::from_uncompressed_file(&dat_path)?;
        validate_quest_118_dat(&dat);
        Ok(())
    }

    #[test]
    pub fn error_on_load_from_zero_bytes() {
        let mut data: &[u8] = &[];
        assert_matches!(
            QuestDat::from_uncompressed_bytes(&mut data),
            Err(QuestDatError::IoError(..))
        );
        assert_matches!(
            QuestDat::from_compressed_bytes(&mut data),
            Err(QuestDatError::PrsCompressionError(..))
        );
    }

    #[test]
    pub fn error_on_load_from_garbage_bytes() {
        let mut data: &[u8] = b"This is definitely not a quest";
        assert_matches!(
            QuestDat::from_uncompressed_bytes(&mut data),
            Err(QuestDatError::DataFormatError(..))
        );
        assert_matches!(
            QuestDat::from_compressed_bytes(&mut data),
            Err(QuestDatError::PrsCompressionError(..))
        );
    }

    #[test]
    pub fn errpr_on_table_header_with_bad_table_size() {
        // dat table header with a table_size issue (table_size != table_body_size + 16)
        let mut header: &[u8] = &[
            0x01, 0x00, 0x00, 0x00, // table_type
            0xD3, 0x08, 0x00, 0x00, // table_size
            0x00, 0x00, 0x00, 0x00, // area
            0xC4, 0x08, 0x00, 0x00, // table_body_size
        ];
        assert_matches!(
            QuestDat::from_uncompressed_bytes(&mut header),
            Err(QuestDatError::DataFormatError(..))
        );
    }

    #[test]
    pub fn error_on_table_header_with_bad_table_type() {
        // dat table header with a table_type issue
        let mut header: &[u8] = &[
            0x11, 0x00, 0x00, 0x00, // table_type
            0xD4, 0x08, 0x00, 0x00, // table_size
            0x00, 0x00, 0x00, 0x00, // area
            0xC4, 0x08, 0x00, 0x00, // table_body_size
        ];
        assert_matches!(
            QuestDat::from_uncompressed_bytes(&mut header),
            Err(QuestDatError::DataFormatError(..))
        );
    }

    #[test]
    pub fn error_on_table_with_body_data_that_is_too_small() {
        // a valid dat table header ...
        let header: &[u8] = &[
            0x01, 0x00, 0x00, 0x00, // table_type
            0xD4, 0x08, 0x00, 0x00, // table_size
            0x00, 0x00, 0x00, 0x00, // area
            0xC4, 0x08, 0x00, 0x00, // table_body_size
        ];
        // ... with not enough random garbage in the table body area
        let mut random_garbage = [0u8; 256];
        let mut rng = StdRng::seed_from_u64(76478964);
        random_garbage.try_fill(&mut rng).unwrap();
        let data = [header, &random_garbage].concat();
        assert_matches!(
            QuestDat::from_uncompressed_bytes(&mut data.as_slice()),
            Err(QuestDatError::IoError(..))
        );
    }
}
