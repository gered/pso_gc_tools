use std::fs::File;
use std::io::{BufReader, Cursor, Write};
use std::path::Path;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use itertools::Itertools;
use rand::random;
use thiserror::Error;

use crate::bytes::FixedLengthByteArrays;
use crate::encryption::{Crypter, PCCrypter};
use crate::packets::quest::*;
use crate::packets::{PacketError, PacketHeader};
use crate::quest::bin::{QuestBin, QuestBinError};
use crate::quest::dat::{QuestDat, QuestDatError};
use crate::text::LanguageError;

#[derive(Error, Debug)]
pub enum QuestQstError {
    #[error("I/O error while processing quest qst")]
    IoError(#[from] std::io::Error),

    #[error("String encoding error during processing of quest qst string data")]
    StringEncodingError(#[from] LanguageError),

    #[error("Error reading quest qst data packet")]
    DataPacketError(#[from] PacketError),

    #[error("Bad quest qst data format: {0}")]
    DataFormatError(String),

    #[error("Error processing quest bin")]
    QuestBinError(#[from] QuestBinError),

    #[error("Error processing quest dat")]
    QuestDatError(#[from] QuestDatError),
}

pub struct QuestQst {
    bin_header: QuestHeaderPacket,
    dat_header: QuestHeaderPacket,
    bin_chunks: Box<[QuestDataPacket]>,
    dat_chunks: Box<[QuestDataPacket]>,
}

fn encrypt_quest_data(
    quest_data: &mut [u8],
    decompressed_size: usize,
) -> Result<Box<[u8]>, QuestQstError> {
    let crypt_key = random::<u32>();

    // yes, PC encryption is used even for gamecube qst files
    let mut crypter = PCCrypter::new(crypt_key);
    crypter.crypt(quest_data);

    let mut result = Vec::<u8>::with_capacity(8 + quest_data.len());
    result.write_u32::<LittleEndian>(decompressed_size as u32)?;
    result.write_u32::<LittleEndian>(crypt_key)?;
    result.write_all(quest_data)?;
    Ok(result.into_boxed_slice())
}

fn decrypt_quest_data(quest_data: &mut [u8]) -> Result<&[u8], QuestQstError> {
    let mut prefix = &quest_data[0..8];
    let _decompressed_size = prefix.read_u32::<LittleEndian>()?;
    let crypt_key = prefix.read_u32::<LittleEndian>()?;

    // yes, PC encryption is used even for gamecube qst files
    let mut crypter = PCCrypter::new(crypt_key);
    let mut result = &mut quest_data[8..];
    crypter.crypt(&mut result);
    Ok(result)
}

fn create_quest_data_chunks(
    quest_data: &[u8],
    filename: &str,
    is_online_quest: bool,
) -> Result<Box<[QuestDataPacket]>, QuestQstError> {
    let mut chunks = Vec::<QuestDataPacket>::new();
    for (index, chunk) in quest_data.chunks(QUEST_DATA_PACKET_DATA_SIZE).enumerate() {
        let mut chunk = QuestDataPacket::new(&filename, chunk, is_online_quest)?;
        chunk.header.flags = index as u8;
        chunks.push(chunk);
    }
    Ok(chunks.into_boxed_slice())
}

fn extract_quest_chunk_data(
    chunks: &[QuestDataPacket],
    is_online_quest: bool,
) -> Result<Vec<u8>, QuestQstError> {
    // TODO: rewrite this function, it is kinda sloppy ...

    let mut data = Vec::<u8>::new();
    for chunk in chunks.iter() {
        data.write_all(&chunk.data[0..(chunk.size as usize)])?;
    }

    let actual_data = if is_online_quest {
        data
    } else {
        decrypt_quest_data(&mut data)?.into()
    };

    Ok(actual_data)
}

impl QuestQst {
    pub fn from_bindat(bin: &QuestBin, dat: &QuestDat) -> Result<QuestQst, QuestQstError> {
        let is_online = !bin.header.is_download; // "download quest" = "offline quest" (because it is played from a memory card ...)
        let quest_name = &bin.header.name;
        let quest_number = bin.header.quest_number_u16(); // i hate the quest .bin quest_number u8/u16 confusion amongst PSO tools ...
        let bin_filename = format!("quest{}.bin", quest_number);
        let dat_filename = format!("quest{}.dat", quest_number);

        let mut bin_bytes = bin.to_compressed_bytes()?;
        let mut dat_bytes = dat.to_compressed_bytes()?;
        if !is_online {
            // offline quests are encrypted with some extra bits added before the encrypted data
            bin_bytes = encrypt_quest_data(bin_bytes.as_mut(), bin.calculate_size())?;
            dat_bytes = encrypt_quest_data(dat_bytes.as_mut(), dat.calculate_size())?;
        }

        let bin_header = QuestHeaderPacket::new(
            quest_name,
            bin.header.language,
            &bin_filename,
            bin_bytes.len(),
            is_online,
        )?;

        let dat_header = QuestHeaderPacket::new(
            quest_name,
            bin.header.language,
            &dat_filename,
            dat_bytes.len(),
            is_online,
        )?;

        let bin_chunks = create_quest_data_chunks(bin_bytes.as_ref(), &bin_filename, is_online)?;
        let dat_chunks = create_quest_data_chunks(dat_bytes.as_ref(), &dat_filename, is_online)?;

        Ok(QuestQst {
            bin_header,
            dat_header,
            bin_chunks,
            dat_chunks,
        })
    }

    pub fn from_file(path: &Path) -> Result<QuestQst, QuestQstError> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        Ok(Self::from_bytes(&mut reader)?)
    }

    pub fn from_bytes<T: ReadBytesExt>(reader: &mut T) -> Result<QuestQst, QuestQstError> {
        let mut bin_header: Option<QuestHeaderPacket> = None;
        let mut dat_header: Option<QuestHeaderPacket> = None;
        let mut bin_chunks = Vec::<QuestDataPacket>::new();
        let mut dat_chunks = Vec::<QuestDataPacket>::new();
        let mut bin_data_counter: usize = 0;
        let mut dat_data_counter: usize = 0;

        // loop, continuing to read packets until we have ALL of the following:
        // - a bin header
        // - a dat header
        // - bin data chunks that contain the exact number of bytes specified by the bin header
        // - dat data chunks that contain the exact number of bytes specified by the dat header
        //
        // the way this reading works should allow for the maximum amount of flexibility of the qst
        // file layout. though, most (all?) things that create qst files will follow this ordering:
        // - bin and dat header (either bin+dat or dat+bin)
        // - interleaved bin and dat chunks
        //
        // however, i have observed that fuzziqer servers (newserv, khyller) generally sends out
        // quest packets un-interleaved. that is, these servers send out bin header + bin data, and
        // then dat header + dat data (actually, i think the ordering might be dat first ...? meh)
        //
        // thus, i decided that even if there is only a very small chance that someone out there
        // saved a qst file in such a "non-standard" format, that we could easily account for any
        // of those variations here
        while (bin_header.is_none()
            || (bin_header.is_some()
                && bin_data_counter < bin_header.as_ref().unwrap().size as usize))
            || (dat_header.is_none()
                || (dat_header.is_some()
                    && dat_data_counter < dat_header.as_ref().unwrap().size as usize))
        {
            // what type of packet is this?
            let packet_header = PacketHeader::from_bytes(reader)?;
            match packet_header.id {
                PACKET_ID_QUEST_HEADER_ONLINE | PACKET_ID_QUEST_HEADER_OFFLINE => {
                    // there can only be one bin and dat header per qst file
                    if bin_header.is_some() && dat_header.is_some() {
                        return Err(QuestQstError::DataFormatError(String::from(
                            "Encountered more than two header packets",
                        )));
                    }

                    let header = QuestHeaderPacket::from_header_and_bytes(packet_header, reader)?;

                    // the header packet must include a filename, as this is used to determine
                    // whether it is for a .bin or .dat file
                    if header.filename.as_unpadded_slice().len() == 0 {
                        return Err(QuestQstError::DataFormatError(String::from(
                            "Encountered header packet with blank filename",
                        )));
                    }

                    match header.file_type() {
                        QuestPacketFileType::Bin => {
                            if bin_header.is_some() {
                                return Err(QuestQstError::DataFormatError(String::from(
                                    "Encountered duplicate bin file header packet",
                                )));
                            } else {
                                bin_header = Some(header);
                            }
                        }
                        QuestPacketFileType::Dat => {
                            if dat_header.is_some() {
                                return Err(QuestQstError::DataFormatError(String::from(
                                    "Encountered duplicate dat file header packet",
                                )));
                            } else {
                                dat_header = Some(header);
                            }
                        }
                        QuestPacketFileType::Unknown => {
                            return Err(QuestQstError::DataFormatError(String::from(
                                "Unable to determine file type from filename in header packet",
                            )));
                        }
                    }
                }
                PACKET_ID_QUEST_DATA_ONLINE | PACKET_ID_QUEST_DATA_OFFLINE => {
                    // data chunk packets must come after its associated header packet
                    // (e.g. .bin data chunks must follow the .bin header, same for .dat ...)
                    if bin_header.is_none() && dat_header.is_none() {
                        return Err(QuestQstError::DataFormatError(String::from(
                            "Encountered data chunk packet before any header packets",
                        )));
                    }

                    let chunk = QuestDataPacket::from_header_and_bytes(packet_header, reader)?;

                    // the data chunk packet must include a filename, as this is used to determine
                    // whether it is for a .bin or .dat file
                    if chunk.filename.as_unpadded_slice().len() == 0 {
                        return Err(QuestQstError::DataFormatError(String::from(
                            "Encountered data chunk packet with blank filename",
                        )));
                    }

                    // small sanity check, technically would not be a problem, but there shouldn't
                    // be any "blank" data chunk packets
                    if chunk.size == 0 {
                        return Err(QuestQstError::DataFormatError(String::from(
                            "Encountered data chunk packet with zero-length data",
                        )));
                    }

                    match chunk.file_type() {
                        QuestPacketFileType::Bin => {
                            if bin_header.is_none() {
                                return Err(QuestQstError::DataFormatError(String::from("Encountered data chunk packet for bin file before its header packet")));
                            } else {
                                bin_data_counter += chunk.size as usize;
                                bin_chunks.push(chunk);
                            }
                        }
                        QuestPacketFileType::Dat => {
                            if dat_header.is_none() {
                                return Err(QuestQstError::DataFormatError(String::from("Encountered data chunk packet for dat file before its header packet")));
                            } else {
                                dat_data_counter += chunk.size as usize;
                                dat_chunks.push(chunk);
                            }
                        }
                        QuestPacketFileType::Unknown => {
                            return Err(QuestQstError::DataFormatError(String::from(
                                "Unable to determine file type from filename in data chunk packet",
                            )))
                        }
                    }
                }
                other_id => {
                    return Err(QuestQstError::DataFormatError(format!(
                        "Unexpected packet id found in quest qst data: {}",
                        other_id
                    )))
                }
            }
        }

        let bin_header = bin_header.unwrap();
        let dat_header = dat_header.unwrap();

        // validate that the file bin/dat data chunk byte counts matched what was specified in the
        // bin/dat headers

        if bin_data_counter as u32 != bin_header.size {
            let size = bin_header.size;
            return Err(QuestQstError::DataFormatError(format!(
                "Read {} bytes of bin data, but the bin header specified {} bytes would be present",
                bin_data_counter, size
            )));
        }
        if dat_data_counter as u32 != dat_header.size {
            let size = dat_header.size;
            return Err(QuestQstError::DataFormatError(format!(
                "Read {} bytes of dat data, but the dat header specified {} bytes would be present",
                dat_data_counter, size
            )));
        }

        // validate that all packets encountered (header and data chunk) were of the same category
        // the entire qst file should have only contained packet IDs:
        // - PACKET_ID_QUEST_HEADER_ONLINE and PACKET_ID_QUEST_DATA_ONLINE, or
        // - PACKET_ID_QUEST_HEADER_OFFLINE and PACKET_ID_QUEST_DATA_OFFLINE

        if bin_header.header.id != dat_header.header.id {
            return Err(QuestQstError::DataFormatError(String::from(
                "Packet header ID mismatch between bin and dat headers",
            )));
        }
        let expected_chunk_packets_id = if bin_header.header.id == PACKET_ID_QUEST_HEADER_ONLINE {
            PACKET_ID_QUEST_DATA_ONLINE
        } else {
            PACKET_ID_QUEST_DATA_OFFLINE
        };

        if bin_chunks
            .iter()
            .filter(|chunk| chunk.header.id != expected_chunk_packets_id)
            .count()
            != 0
        {
            return Err(QuestQstError::DataFormatError(format!(
                "One or more bin data chunk packets were not of the expected type: {}",
                expected_chunk_packets_id
            )));
        }
        if dat_chunks
            .iter()
            .filter(|chunk| chunk.header.id != expected_chunk_packets_id)
            .count()
            != 0
        {
            return Err(QuestQstError::DataFormatError(format!(
                "One or more dat data chunk packets were not of the expected type: {}",
                expected_chunk_packets_id
            )));
        }

        Ok(QuestQst {
            bin_header,
            dat_header,
            bin_chunks: bin_chunks.into_boxed_slice(),
            dat_chunks: dat_chunks.into_boxed_slice(),
        })
    }

    pub fn write_bytes<T: WriteBytesExt>(&self, writer: &mut T) -> Result<(), QuestQstError> {
        self.bin_header.write_bytes(writer)?;
        self.dat_header.write_bytes(writer)?;
        for chunk in self.bin_chunks.iter().interleave(self.dat_chunks.iter()) {
            chunk.write_bytes(writer)?;
        }
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<Box<[u8]>, QuestQstError> {
        let mut buffer = Cursor::new(Vec::<u8>::new());
        self.write_bytes(&mut buffer)?;
        Ok(buffer.into_inner().into_boxed_slice())
    }

    pub fn is_online(&self) -> bool {
        // assumes that a QuestQst could never be created with bin/dat headers containing
        // different packet IDs ...
        self.bin_header.header.id == PACKET_ID_QUEST_HEADER_ONLINE
    }

    pub fn extract_bin_bytes(&self) -> Result<Box<[u8]>, QuestQstError> {
        Ok(extract_quest_chunk_data(&self.bin_chunks, self.is_online())?.into_boxed_slice())
    }

    pub fn extract_bin(&self) -> Result<QuestBin, QuestQstError> {
        let data = self.extract_bin_bytes()?;
        Ok(QuestBin::from_compressed_bytes(data.as_ref())?)
    }

    pub fn extract_dat_bytes(&self) -> Result<Box<[u8]>, QuestQstError> {
        Ok(extract_quest_chunk_data(&self.dat_chunks, self.is_online())?.into_boxed_slice())
    }

    pub fn extract_dat(&self) -> Result<QuestDat, QuestQstError> {
        let data = self.extract_dat_bytes()?;
        Ok(QuestDat::from_compressed_bytes(data.as_ref())?)
    }
}

#[cfg(test)]
mod tests {
    use crate::quest::bin::tests::{validate_quest_118_bin, validate_quest_58_bin};
    use crate::quest::dat::tests::{validate_quest_118_dat, validate_quest_58_dat};

    use super::*;

    fn get_num_chunks_for_size(size: usize) -> usize {
        ((size as f32) / (QUEST_DATA_PACKET_DATA_SIZE as f32)).ceil() as usize
    }

    fn validate_quest_58_qst(
        qst: &QuestQst,
        bin_size: usize,
        dat_size: usize,
        is_online: bool,
    ) -> Result<(), QuestQstError> {
        let (expected_header_id, expected_chunk_id) = if is_online {
            (PACKET_ID_QUEST_HEADER_ONLINE, PACKET_ID_QUEST_DATA_ONLINE)
        } else {
            (PACKET_ID_QUEST_HEADER_OFFLINE, PACKET_ID_QUEST_DATA_OFFLINE)
        };

        assert_eq!(qst.is_online(), is_online);

        assert_eq!(qst.bin_header.header.id, expected_header_id);
        assert_eq!(qst.bin_header.name_str()?, "Lost HEAT SWORD");
        assert_eq!(qst.bin_header.filename_str()?, "quest58.bin");
        assert_eq!(qst.bin_header.file_type(), QuestPacketFileType::Bin);
        let size = qst.bin_header.size as usize;
        assert_eq!(size, bin_size);

        let num_chunks = get_num_chunks_for_size(bin_size);
        assert_eq!(qst.bin_chunks.len(), num_chunks);
        for chunk in qst.bin_chunks.iter() {
            assert_eq!(chunk.header.id, expected_chunk_id);
            assert_eq!(chunk.filename_str()?, "quest58.bin");
            assert_eq!(chunk.file_type(), QuestPacketFileType::Bin);
            assert!(chunk.data().len() > 0);
        }

        assert_eq!(qst.dat_header.header.id, expected_header_id);
        assert_eq!(qst.dat_header.name_str()?, "Lost HEAT SWORD");
        assert_eq!(qst.dat_header.filename_str()?, "quest58.dat");
        assert_eq!(qst.dat_header.file_type(), QuestPacketFileType::Dat);
        let size = qst.dat_header.size as usize;
        assert_eq!(size, dat_size);

        let num_chunks = get_num_chunks_for_size(dat_size);
        assert_eq!(qst.dat_chunks.len(), num_chunks);
        for chunk in qst.dat_chunks.iter() {
            assert_eq!(chunk.header.id, expected_chunk_id);
            assert_eq!(chunk.filename_str()?, "quest58.dat");
            assert_eq!(chunk.file_type(), QuestPacketFileType::Dat);
            assert!(chunk.data().len() > 0);
        }

        let mut bin = qst.extract_bin()?;
        if !is_online {
            assert_eq!(true, bin.header.is_download);
            bin.header.is_download = false;
        }
        validate_quest_58_bin(&bin);

        let dat = qst.extract_dat()?;
        validate_quest_58_dat(&dat);

        Ok(())
    }

    fn validate_quest_118_qst(
        qst: &QuestQst,
        bin_size: usize,
        dat_size: usize,
        is_online: bool,
    ) -> Result<(), QuestQstError> {
        let (expected_header_id, expected_chunk_id) = if is_online {
            (PACKET_ID_QUEST_HEADER_ONLINE, PACKET_ID_QUEST_DATA_ONLINE)
        } else {
            (PACKET_ID_QUEST_HEADER_OFFLINE, PACKET_ID_QUEST_DATA_OFFLINE)
        };

        assert_eq!(qst.is_online(), is_online);

        assert_eq!(qst.bin_header.header.id, expected_header_id);
        assert_eq!(qst.bin_header.name_str()?, "Towards the Future");
        assert_eq!(qst.bin_header.filename_str()?, "quest118.bin");
        assert_eq!(qst.bin_header.file_type(), QuestPacketFileType::Bin);
        let size = qst.bin_header.size as usize;
        assert_eq!(size, bin_size);

        let num_chunks = get_num_chunks_for_size(bin_size);
        assert_eq!(qst.bin_chunks.len(), num_chunks);
        for chunk in qst.bin_chunks.iter() {
            assert_eq!(chunk.header.id, expected_chunk_id);
            assert_eq!(chunk.filename_str()?, "quest118.bin");
            assert_eq!(chunk.file_type(), QuestPacketFileType::Bin);
            assert!(chunk.data().len() > 0);
        }

        assert_eq!(qst.dat_header.header.id, expected_header_id);
        assert_eq!(qst.dat_header.name_str()?, "Towards the Future");
        assert_eq!(qst.dat_header.filename_str()?, "quest118.dat");
        assert_eq!(qst.dat_header.file_type(), QuestPacketFileType::Dat);
        let size = qst.dat_header.size as usize;
        assert_eq!(size, dat_size);

        let num_chunks = get_num_chunks_for_size(dat_size);
        assert_eq!(qst.dat_chunks.len(), num_chunks);
        for chunk in qst.dat_chunks.iter() {
            assert_eq!(chunk.header.id, expected_chunk_id);
            assert_eq!(chunk.filename_str()?, "quest118.dat");
            assert_eq!(chunk.file_type(), QuestPacketFileType::Dat);
            assert!(chunk.data().len() > 0);
        }

        let mut bin = qst.extract_bin()?;
        if !is_online {
            assert_eq!(true, bin.header.is_download);
            bin.header.is_download = false;
        }
        validate_quest_118_bin(&bin);

        let dat = qst.extract_dat()?;
        validate_quest_118_dat(&dat);

        Ok(())
    }

    #[test]
    pub fn read_quest_58_qst_from_file() -> Result<(), QuestQstError> {
        let qst = QuestQst::from_file(Path::new("assets/test/q058-ret-gc.online.qst"))?;
        validate_quest_58_qst(&qst, 1438, 15097, true)?;

        let qst = QuestQst::from_file(Path::new("assets/test/q058-ret-gc.offline.qst"))?;
        validate_quest_58_qst(&qst, 1571, 15105, false)?;

        Ok(())
    }

    #[test]
    pub fn read_quest_118_qst_from_file() -> Result<(), QuestQstError> {
        let qst = QuestQst::from_file(Path::new("assets/test/q118-vr-gc.online.qst"))?;
        validate_quest_118_qst(&qst, 14208, 11802, true)?;

        let qst = QuestQst::from_file(Path::new("assets/test/q118-vr-gc.offline.qst"))?;
        validate_quest_118_qst(&qst, 14801, 11810, false)?;

        Ok(())
    }

    #[test]
    pub fn create_qst_from_quest_58_bindat_files() -> Result<(), QuestQstError> {
        let mut bin = QuestBin::from_compressed_file(Path::new("assets/test/q058-ret-gc.bin"))?;
        let dat = QuestDat::from_compressed_file(Path::new("assets/test/q058-ret-gc.dat"))?;

        let qst = QuestQst::from_bindat(&bin, &dat)?;
        validate_quest_58_qst(&qst, 1565, 15507, true)?;

        bin.header.is_download = true;
        let qst = QuestQst::from_bindat(&bin, &dat)?;
        validate_quest_58_qst(&qst, 1573, 15515, false)?;

        Ok(())
    }

    #[test]
    pub fn create_qst_from_quest_118_bindat_files() -> Result<(), QuestQstError> {
        let mut bin = QuestBin::from_compressed_file(Path::new("assets/test/q118-vr-gc.bin"))?;
        let dat = QuestDat::from_compressed_file(Path::new("assets/test/q118-vr-gc.dat"))?;

        let qst = QuestQst::from_bindat(&bin, &dat)?;
        validate_quest_118_qst(&qst, 14794, 12277, true)?;

        bin.header.is_download = true;
        let qst = QuestQst::from_bindat(&bin, &dat)?;
        validate_quest_118_qst(&qst, 14803, 12285, false)?;

        Ok(())
    }
}
