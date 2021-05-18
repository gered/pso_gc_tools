use crate::text::LanguageError;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use thiserror::Error;

pub mod quest;

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
