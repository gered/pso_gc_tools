use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::convert::{TryFrom, TryInto};
use std::io::Cursor;

use crate::bytes::ReadFixedLengthByteArray;
use crate::packets::{GenericPacket, PacketError, PacketHeader};

pub const COPYRIGHT_MESSAGE_SIZE: usize = 64;

pub const LOGIN_SERVER_COPYRIGHT_MESSAGE: &[u8; COPYRIGHT_MESSAGE_SIZE] =
    b"DreamCast Port Map. Copyright SEGA Enterprises. 1999\0\0\0\0\0\0\0\0\0\0\0\0";
pub const SHIP_SERVER_COPYRIGHT_MESSAGE: &[u8; COPYRIGHT_MESSAGE_SIZE] =
    b"DreamCast Lobby Server. Copyright SEGA Enterprises. 1999\0\0\0\0\0\0\0\0";

pub const PACKET_ID_INIT_ENCRYPTION_LOGIN_SERVER: u8 = 0x17;
pub const PACKET_ID_INIT_ENCRYPTION_SHIP_SERVER: u8 = 0x02;

#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
pub struct InitEncryptionPacket {
    pub header: PacketHeader,
    pub copyright_message: [u8; COPYRIGHT_MESSAGE_SIZE],
    pub server_key: u32,
    pub client_key: u32,
}

impl InitEncryptionPacket {
    pub const fn packet_size() -> usize {
        std::mem::size_of::<Self>()
    }

    pub fn new(
        is_login_server: bool,
        server_key: u32,
        client_key: u32,
    ) -> Result<InitEncryptionPacket, PacketError> {
        Ok(InitEncryptionPacket {
            header: PacketHeader {
                id: if is_login_server {
                    PACKET_ID_INIT_ENCRYPTION_LOGIN_SERVER
                } else {
                    PACKET_ID_INIT_ENCRYPTION_SHIP_SERVER
                },
                flags: 0,
                size: Self::packet_size() as u16,
            },
            copyright_message: if is_login_server {
                LOGIN_SERVER_COPYRIGHT_MESSAGE.clone()
            } else {
                SHIP_SERVER_COPYRIGHT_MESSAGE.clone()
            },
            server_key,
            client_key,
        })
    }

    pub fn from_bytes<T: ReadBytesExt>(reader: &mut T) -> Result<InitEncryptionPacket, PacketError>
    where
        Self: Sized,
    {
        let header = PacketHeader::from_bytes(reader)?;
        Self::from_header_and_bytes(header, reader)
    }

    pub fn from_header_and_bytes<T: ReadBytesExt>(
        header: PacketHeader,
        reader: &mut T,
    ) -> Result<InitEncryptionPacket, PacketError>
    where
        Self: Sized,
    {
        if header.id != PACKET_ID_INIT_ENCRYPTION_LOGIN_SERVER
            && header.id != PACKET_ID_INIT_ENCRYPTION_SHIP_SERVER
        {
            return Err(PacketError::WrongId(header.id));
        }
        if header.size < Self::packet_size() as u16 {
            return Err(PacketError::WrongSize(header.size));
        }

        let copyright_message: [u8; COPYRIGHT_MESSAGE_SIZE] = reader.read_bytes()?;
        if copyright_message.ne(LOGIN_SERVER_COPYRIGHT_MESSAGE)
            && copyright_message.ne(SHIP_SERVER_COPYRIGHT_MESSAGE)
        {
            return Err(PacketError::DataFormatError(String::from(
                "Unexpected copyright message string",
            )));
        }

        let server_key = reader.read_u32::<LittleEndian>()?;
        let client_key = reader.read_u32::<LittleEndian>()?;

        // if the packet contained extra bytes we need to read them from the buffer.
        // but we don't actually care about these extra bytes. we're not going to keep them ...
        if header.size > Self::packet_size() as u16 {
            let remaining_length = header.size as usize - Self::packet_size();
            let mut _throw_away = vec![0u8; remaining_length];
            reader.read_exact(&mut _throw_away)?;
        }

        Ok(InitEncryptionPacket {
            header,
            copyright_message,
            server_key,
            client_key,
        })
    }

    pub fn write_body_bytes<T: WriteBytesExt>(&self, writer: &mut T) -> Result<(), PacketError> {
        writer.write_all(&self.copyright_message)?;
        writer.write_u32::<LittleEndian>(self.server_key)?;
        writer.write_u32::<LittleEndian>(self.client_key)?;
        Ok(())
    }

    pub fn write_bytes<T: WriteBytesExt>(&self, writer: &mut T) -> Result<(), PacketError> {
        self.header.write_bytes(writer)?;
        self.write_body_bytes(writer)?;
        Ok(())
    }

    pub fn server_key(&self) -> u32 {
        self.server_key
    }

    pub fn client_key(&self) -> u32 {
        self.client_key
    }
}

impl TryFrom<GenericPacket> for InitEncryptionPacket {
    type Error = PacketError;

    fn try_from(value: GenericPacket) -> Result<Self, Self::Error> {
        let mut reader = Cursor::new(value.body);
        Ok(InitEncryptionPacket::from_header_and_bytes(
            value.header,
            &mut reader,
        )?)
    }
}

impl TryInto<GenericPacket> for InitEncryptionPacket {
    type Error = PacketError;

    fn try_into(self) -> Result<GenericPacket, Self::Error> {
        let header = self.header;
        let mut body = Vec::new();
        self.write_body_bytes(&mut body)?;
        Ok(GenericPacket {
            header,
            body: body.into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use claim::*;

    use super::*;

    #[test]
    pub fn create_init_encryption_packet_from_bytes() -> Result<(), PacketError> {
        // login server packet

        let mut bytes: &[u8] = &[
            0x17, 0x00, 0x4c, 0x00, 0x44, 0x72, 0x65, 0x61, 0x6d, 0x43, 0x61, 0x73, 0x74, 0x20,
            0x50, 0x6f, 0x72, 0x74, 0x20, 0x4d, 0x61, 0x70, 0x2e, 0x20, 0x43, 0x6f, 0x70, 0x79,
            0x72, 0x69, 0x67, 0x68, 0x74, 0x20, 0x53, 0x45, 0x47, 0x41, 0x20, 0x45, 0x6e, 0x74,
            0x65, 0x72, 0x70, 0x72, 0x69, 0x73, 0x65, 0x73, 0x2e, 0x20, 0x31, 0x39, 0x39, 0x39,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x50, 0x5f,
            0x48, 0x1e, 0x50, 0x5f, 0x48, 0x1e,
        ];

        let packet = InitEncryptionPacket::from_bytes(&mut bytes)?;
        assert_eq!(packet.header.id(), PACKET_ID_INIT_ENCRYPTION_LOGIN_SERVER);
        assert_eq!(
            packet.header.size(),
            InitEncryptionPacket::packet_size() as u16
        );
        assert_eq!(packet.copyright_message, *LOGIN_SERVER_COPYRIGHT_MESSAGE);
        assert_eq!(packet.server_key(), 0x1e485f50);
        assert_eq!(packet.client_key(), 0x1e485f50);

        // ship server packet

        let mut bytes: &[u8] = &[
            0x02, 0x00, 0x4c, 0x00, 0x44, 0x72, 0x65, 0x61, 0x6d, 0x43, 0x61, 0x73, 0x74, 0x20,
            0x4c, 0x6f, 0x62, 0x62, 0x79, 0x20, 0x53, 0x65, 0x72, 0x76, 0x65, 0x72, 0x2e, 0x20,
            0x43, 0x6f, 0x70, 0x79, 0x72, 0x69, 0x67, 0x68, 0x74, 0x20, 0x53, 0x45, 0x47, 0x41,
            0x20, 0x45, 0x6e, 0x74, 0x65, 0x72, 0x70, 0x72, 0x69, 0x73, 0x65, 0x73, 0x2e, 0x20,
            0x31, 0x39, 0x39, 0x39, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x37, 0xf7,
            0x50, 0x90, 0x23, 0xe5, 0x6e, 0x1c,
        ];

        let packet = InitEncryptionPacket::from_bytes(&mut bytes)?;
        assert_eq!(packet.header.id(), PACKET_ID_INIT_ENCRYPTION_SHIP_SERVER);
        assert_eq!(
            packet.header.size(),
            InitEncryptionPacket::packet_size() as u16
        );
        assert_eq!(packet.copyright_message, *SHIP_SERVER_COPYRIGHT_MESSAGE);
        assert_eq!(packet.server_key(), 0x9050f737);
        assert_eq!(packet.client_key(), 0x1c6ee523);

        Ok(())
    }

    #[test]
    pub fn create_init_encryption_packet_via_new() -> Result<(), PacketError> {
        // login server packet

        let packet = InitEncryptionPacket::new(true, 0x11223344, 0x55667788)?;
        assert_eq!(packet.header.id(), PACKET_ID_INIT_ENCRYPTION_LOGIN_SERVER);
        assert_eq!(
            packet.header.size(),
            InitEncryptionPacket::packet_size() as u16
        );
        assert_eq!(packet.copyright_message, *LOGIN_SERVER_COPYRIGHT_MESSAGE);
        assert_eq!(packet.server_key(), 0x11223344);
        assert_eq!(packet.client_key(), 0x55667788);

        // ship server packet

        let packet = InitEncryptionPacket::new(false, 0x44332211, 0x88776655)?;
        assert_eq!(packet.header.id(), PACKET_ID_INIT_ENCRYPTION_SHIP_SERVER);
        assert_eq!(
            packet.header.size(),
            InitEncryptionPacket::packet_size() as u16
        );
        assert_eq!(packet.copyright_message, *SHIP_SERVER_COPYRIGHT_MESSAGE);
        assert_eq!(packet.server_key(), 0x44332211);
        assert_eq!(packet.client_key(), 0x88776655);

        Ok(())
    }

    #[test]
    pub fn can_create_init_encryption_packet_from_bytes_with_extra_bytes_included(
    ) -> Result<(), PacketError> {
        // the extra bytes that can be included in this packet are basically always ignored
        // and we don't even provide any way to access them once the packet struct is created
        // (they are skipped over).
        // i don't think this is a big deal, but it is worth mentioning ...

        let mut bytes: &[u8] = &[
            0x17, 0x00, 0x10, 0x01, 0x44, 0x72, 0x65, 0x61, 0x6d, 0x43, 0x61, 0x73, 0x74, 0x20,
            0x50, 0x6f, 0x72, 0x74, 0x20, 0x4d, 0x61, 0x70, 0x2e, 0x20, 0x43, 0x6f, 0x70, 0x79,
            0x72, 0x69, 0x67, 0x68, 0x74, 0x20, 0x53, 0x45, 0x47, 0x41, 0x20, 0x45, 0x6e, 0x74,
            0x65, 0x72, 0x70, 0x72, 0x69, 0x73, 0x65, 0x73, 0x2e, 0x20, 0x31, 0x39, 0x39, 0x39,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x94, 0x39,
            0x25, 0x4b, 0x1a, 0x23, 0x2f, 0x72, 0x54, 0x68, 0x69, 0x73, 0x20, 0x73, 0x65, 0x72,
            0x76, 0x65, 0x72, 0x20, 0x69, 0x73, 0x20, 0x69, 0x6e, 0x20, 0x6e, 0x6f, 0x20, 0x77,
            0x61, 0x79, 0x20, 0x61, 0x66, 0x66, 0x69, 0x6c, 0x69, 0x61, 0x74, 0x65, 0x64, 0x2c,
            0x20, 0x73, 0x70, 0x6f, 0x6e, 0x73, 0x6f, 0x72, 0x65, 0x64, 0x2c, 0x20, 0x6f, 0x72,
            0x20, 0x73, 0x75, 0x70, 0x70, 0x6f, 0x72, 0x74, 0x65, 0x64, 0x20, 0x62, 0x79, 0x20,
            0x53, 0x45, 0x47, 0x41, 0x20, 0x45, 0x6e, 0x74, 0x65, 0x72, 0x70, 0x72, 0x69, 0x73,
            0x65, 0x73, 0x20, 0x6f, 0x72, 0x20, 0x53, 0x4f, 0x4e, 0x49, 0x43, 0x54, 0x45, 0x41,
            0x4d, 0x2e, 0x20, 0x54, 0x68, 0x65, 0x20, 0x70, 0x72, 0x65, 0x63, 0x65, 0x64, 0x69,
            0x6e, 0x67, 0x20, 0x6d, 0x65, 0x73, 0x73, 0x61, 0x67, 0x65, 0x20, 0x65, 0x78, 0x69,
            0x73, 0x74, 0x73, 0x20, 0x6f, 0x6e, 0x6c, 0x79, 0x20, 0x69, 0x6e, 0x20, 0x6f, 0x72,
            0x64, 0x65, 0x72, 0x20, 0x74, 0x6f, 0x20, 0x72, 0x65, 0x6d, 0x61, 0x69, 0x6e, 0x20,
            0x63, 0x6f, 0x6d, 0x70, 0x61, 0x74, 0x69, 0x62, 0x6c, 0x65, 0x20, 0x77, 0x69, 0x74,
            0x68, 0x20, 0x70, 0x72, 0x6f, 0x67, 0x72, 0x61, 0x6d, 0x73, 0x20, 0x74, 0x68, 0x61,
            0x74, 0x20, 0x65, 0x78, 0x70, 0x65, 0x63, 0x74, 0x20, 0x69, 0x74, 0x2e, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let packet = InitEncryptionPacket::from_bytes(&mut bytes)?;
        assert_eq!(packet.header.id(), PACKET_ID_INIT_ENCRYPTION_LOGIN_SERVER);
        assert_ge!(
            packet.header.size(),
            InitEncryptionPacket::packet_size() as u16
        );
        assert_eq!(packet.copyright_message, *LOGIN_SERVER_COPYRIGHT_MESSAGE);
        assert_eq!(packet.server_key(), 0x4b253994);
        assert_eq!(packet.client_key(), 0x722f231a);

        Ok(())
    }
}
