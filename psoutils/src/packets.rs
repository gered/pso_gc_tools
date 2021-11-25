use crate::text::{Language, LanguageError};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use thiserror::Error;

pub mod quest;

pub const PACKET_DEFAULT_LANGUAGE: Language = Language::English;

#[derive(Error, Debug)]
pub enum PacketError {
    #[error("Packet ID {0} is wrong for this packet type")]
    WrongId(u8),

    #[error("Packet size {0} is wrong for this packet type")]
    WrongSize(u16),

    #[error("I/O error while processing packet data")]
    IoError(#[from] std::io::Error),

    #[error("String field encoding error")]
    LanguageError(#[from] LanguageError),

    #[error("Packet data format error: {0}")]
    DataFormatError(String),
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct PacketHeader {
    pub id: u8,
    pub flags: u8,
    pub size: u16,
}

impl PacketHeader {
    pub const fn header_size() -> usize {
        std::mem::size_of::<Self>()
    }

    pub fn from_bytes<T: ReadBytesExt>(reader: &mut T) -> Result<PacketHeader, PacketError>
    where
        Self: Sized,
    {
        let id = reader.read_u8()?;
        let flags = reader.read_u8()?;
        let size = reader.read_u16::<LittleEndian>()?;
        Ok(PacketHeader { id, flags, size })
    }

    pub fn write_bytes<T: WriteBytesExt>(&self, writer: &mut T) -> Result<(), PacketError> {
        writer.write_u8(self.id)?;
        writer.write_u8(self.flags)?;
        writer.write_u16::<LittleEndian>(self.size)?;
        Ok(())
    }

    pub fn id(&self) -> u8 {
        self.id
    }

    pub fn size(&self) -> u16 {
        self.size
    }
}

#[derive(Debug, Clone)]
pub struct GenericPacket {
    pub header: PacketHeader,
    pub body: Box<[u8]>,
}

impl GenericPacket {
    pub fn new(header: PacketHeader, body: Box<[u8]>) -> GenericPacket {
        GenericPacket { header, body }
    }

    pub fn from_bytes<T: ReadBytesExt>(reader: &mut T) -> Result<GenericPacket, PacketError> {
        let header = PacketHeader::from_bytes(reader)?;
        let data_length = header.size as usize - PacketHeader::header_size();
        let mut body = vec![0u8; data_length];
        reader.read_exact(&mut body)?;
        Ok(GenericPacket {
            header,
            body: body.into(),
        })
    }

    pub fn size(&self) -> usize {
        self.header.size as usize + self.body.len()
    }
}
