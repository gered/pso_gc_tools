use byteorder::ReadBytesExt;
use std::io::Error;

pub trait FixedLengthByteArrays {
    fn as_unpadded_slice(&self) -> &[u8];
    fn to_fixed_length(&self, length: usize) -> Vec<u8>;
    fn to_array<const N: usize>(&self) -> [u8; N];
}

impl<T: AsRef<[u8]> + ?Sized> FixedLengthByteArrays for T {
    fn as_unpadded_slice(&self) -> &[u8] {
        let end = self.as_ref().iter().take_while(|&b| *b != 0).count();
        &self.as_ref()[0..end]
    }

    fn to_fixed_length(&self, length: usize) -> Vec<u8> {
        let mut result = self.as_ref().to_vec();
        if result.len() != length {
            result.resize(length, 0u8);
        }
        result
    }

    fn to_array<const N: usize>(&self) -> [u8; N] {
        assert_ne!(N, 0);
        let mut array = [0u8; N];
        if N <= self.as_ref().len() {
            array.copy_from_slice(&self.as_ref()[0..N]);
        } else {
            array[0..self.as_ref().len()].copy_from_slice(&self.as_ref())
        }
        array
    }
}

pub trait ReadFixedLengthByteArray {
    fn read_bytes<const N: usize>(&mut self) -> Result<[u8; N], std::io::Error>;
}

impl<T: ReadBytesExt> ReadFixedLengthByteArray for T {
    fn read_bytes<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        assert_ne!(N, 0);
        let mut array = [0u8; N];
        self.read_exact(&mut array)?;
        Ok(array)
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
