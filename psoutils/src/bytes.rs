use byteorder::{ReadBytesExt, WriteBytesExt};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReadBytesError {
    #[error("Unexpected error while reading bytes: {0}")]
    UnexpectedError(String),

    #[error("I/O error reading bytes")]
    IoError(#[from] std::io::Error),
}

pub trait ReadFromBytes<T: ReadBytesExt>: Sized {
    fn read_from_bytes(reader: &mut T) -> Result<Self, ReadBytesError>;
}

#[derive(Error, Debug)]
pub enum WriteBytesError {
    #[error("Unexpected error while writing bytes: {0}")]
    UnexpectedError(String),

    #[error("I/O error writing bytes")]
    IoError(#[from] std::io::Error),
}

pub trait WriteAsBytes<T: WriteBytesExt> {
    fn write_as_bytes(&self, writer: &mut T) -> Result<(), WriteBytesError>;
}

pub trait FixedLengthByteArrays {
    fn as_unpadded_slice(&self) -> &[u8];
    fn to_fixed_length(&self, length: usize) -> Vec<u8>;
}

impl<T: AsRef<[u8]> + ?Sized> FixedLengthByteArrays for T {
    fn as_unpadded_slice(&self) -> &[u8] {
        let end = self.as_ref().iter().take_while(|&b| *b != 0).count();
        &self.as_ref()[0..end]
        /*
        self.as_ref()
            .iter()
            .take_while(|&b| *b != 0u8)
            .map(|b| *b)
            .collect()
             */
    }

    fn to_fixed_length(&self, length: usize) -> Vec<u8> {
        let mut result = self.as_ref().to_vec();
        if result.len() != length {
            result.resize(length, 0u8);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn fixed_length_byte_arrays() {
        let bytes: &[u8] = &[
            0x54, 0x68, 0x65, 0x20, 0x45, 0x61, 0x73, 0x74, 0x20, 0x54, 0x6f, 0x77, 0x65, 0x72,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];
        assert_eq!(
            vec![
                0x54, 0x68, 0x65, 0x20, 0x45, 0x61, 0x73, 0x74, 0x20, 0x54, 0x6f, 0x77, 0x65, 0x72
            ],
            bytes.as_unpadded_slice()
        );

        let bytes: &[u8] = &[
            0x54, 0x68, 0x65, 0x20, 0x45, 0x61, 0x73, 0x74, 0x20, 0x54, 0x6f, 0x77, 0x65, 0x72,
        ];
        assert_eq!(
            vec![
                0x54, 0x68, 0x65, 0x20, 0x45, 0x61, 0x73, 0x74, 0x20, 0x54, 0x6f, 0x77, 0x65, 0x72
            ],
            bytes.as_unpadded_slice()
        );

        let bytes: &[u8] = &[
            0x54, 0x68, 0x65, 0x20, 0x45, 0x61, 0x73, 0x74, 0x20, 0x54, 0x6f, 0x77, 0x65, 0x72,
        ];
        assert_eq!(
            vec![
                0x54, 0x68, 0x65, 0x20, 0x45, 0x61, 0x73, 0x74, 0x20, 0x54, 0x6f, 0x77, 0x65, 0x72,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00,
            ],
            bytes.to_fixed_length(32)
        );

        let bytes: &[u8] = &[
            0x54, 0x68, 0x65, 0x20, 0x45, 0x61, 0x73, 0x74, 0x20, 0x54, 0x6f, 0x77, 0x65, 0x72,
        ];
        assert_eq!(
            vec![
                0x54, 0x68, 0x65, 0x20, 0x45, 0x61, 0x73, 0x74, 0x20, 0x54, 0x6f, 0x77, 0x65, 0x72,
            ],
            bytes.to_fixed_length(14)
        );

        let bytes: &[u8] = &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
        assert_eq!(vec![0x01, 0x02, 0x03, 0x04], bytes.to_fixed_length(4));
    }
}
