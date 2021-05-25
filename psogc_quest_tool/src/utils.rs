use crc::{crc32, Hasher32};

pub fn crc32(bytes: &[u8]) -> u32 {
    let mut digest = crc32::Digest::new(crc32::IEEE);
    digest.write(bytes);
    digest.sum32()
}
