use byte_slice_cast::AsMutSliceOf;
use thiserror::Error;

const PC_STREAM_LENGTH: usize = 57;
const GC_STREAM_LENGTH: usize = 521;

#[derive(Error, Debug, PartialEq)]
pub enum EncryptionError {
    #[error("Error casting input data slice")]
    InputDataCastingError(#[from] byte_slice_cast::Error),
}

pub trait Crypter {
    fn crypt(&mut self, data: &mut [u8]) -> Result<(), EncryptionError>;
}

pub struct GCCrypter {
    stream: [u32; GC_STREAM_LENGTH],
    offset: usize,
}

impl GCCrypter {
    pub fn new(seed: u32) -> GCCrypter {
        let mut seed = seed;
        let mut basekey = 0;
        let mut stream = [0u32; GC_STREAM_LENGTH];
        let mut offset = 0;

        for _ in 0..=16 {
            for _ in 0..32 {
                seed = seed.wrapping_mul(0x5d588b65);
                basekey >>= 1;
                seed = seed.wrapping_add(1);
                if seed & 0x80000000 != 0 {
                    basekey |= 0x80000000;
                } else {
                    basekey &= 0x7fffffff;
                }
            }
            stream[offset] = basekey;
            offset += 1;
        }

        stream[offset - 1] = ((stream[0] >> 9) ^ (stream[offset - 1] << 23)) ^ stream[15];
        let mut source1 = 0;
        let mut source2 = 1;
        let mut source3 = offset - 1;
        while offset != GC_STREAM_LENGTH {
            stream[offset] = stream[source3]
                ^ (((stream[source1] << 23) & 0xff800000) ^ ((stream[source2] >> 9) & 0x007fffff));
            offset += 1;
            source1 += 1;
            source2 += 1;
            source3 += 1;
        }

        let mut crypter = GCCrypter { stream, offset };
        crypter.update_stream();
        crypter.update_stream();
        crypter.update_stream();
        crypter.offset = GC_STREAM_LENGTH - 1;

        crypter
    }

    fn update_stream(&mut self) {
        let mut r5: u32 = 0;
        let mut r6: u32 = 489;
        let mut r7: u32 = 0;

        while r6 != GC_STREAM_LENGTH as u32 {
            self.stream[r5 as usize] ^= self.stream[r6 as usize];
            r5 += 1;
            r6 += 1;
        }

        while r5 != GC_STREAM_LENGTH as u32 {
            self.stream[r5 as usize] ^= self.stream[r7 as usize];
            r5 += 1;
            r7 += 1;
        }

        self.offset = 0;
    }

    fn next(&mut self) -> u32 {
        self.offset += 1;
        if self.offset == GC_STREAM_LENGTH {
            self.update_stream();
        }
        self.stream[self.offset]
    }
}

impl Crypter for GCCrypter {
    fn crypt(&mut self, data: &mut [u8]) -> Result<(), EncryptionError> {
        let data = data.as_mut_slice_of::<u32>()?;

        for dword in data.iter_mut() {
            *dword ^= self.next().to_le();
        }

        Ok(())
    }
}

pub struct PCCrypter {
    stream: [u32; PC_STREAM_LENGTH],
    offset: usize,
}

impl PCCrypter {
    pub fn new(seed: u32) -> PCCrypter {
        let mut esi: u32 = 1;
        let mut ebx: u32 = seed;
        let mut edi: u32 = 0x15;

        let mut stream = [0u32; PC_STREAM_LENGTH];
        stream[56] = ebx;
        stream[55] = ebx;

        while edi <= 0x46e {
            let eax = edi;
            let var1 = eax / 55;
            let edx = eax.wrapping_sub(var1 * 55);
            ebx = ebx.wrapping_sub(esi);
            edi = edi.wrapping_add(0x15);
            stream[edx as usize] = esi;
            esi = ebx;
            ebx = stream[edx as usize];
        }

        let mut crypter = PCCrypter {
            stream,
            offset: PC_STREAM_LENGTH - 1,
        };

        crypter.update_stream();
        crypter.update_stream();
        crypter.update_stream();
        crypter.update_stream();

        crypter
    }

    fn update_stream(&mut self) {
        let mut edi: u32 = 1;
        let mut edx: u32 = 0x18;
        let mut eax = edi;
        while edx > 0 {
            let esi = self.stream[eax.wrapping_add(0x1f) as usize];
            let ebp = self.stream[eax as usize].wrapping_sub(esi);
            self.stream[eax as usize] = ebp;
            eax = eax.wrapping_add(1);
            edx = edx.wrapping_sub(1);
        }

        edi = 0x19;
        edx = 0x1f;
        eax = edi;
        while edx > 0 {
            let esi = self.stream[eax.wrapping_sub(0x18) as usize];
            let ebp = self.stream[eax as usize].wrapping_sub(esi);
            self.stream[eax as usize] = ebp;
            eax = eax.wrapping_add(1);
            edx = edx.wrapping_sub(1);
        }
    }

    fn next(&mut self) -> u32 {
        if self.offset == PC_STREAM_LENGTH - 1 {
            self.update_stream();
            self.offset = 1;
        }
        let next = self.stream[self.offset];
        self.offset += 1;
        next
    }
}

impl Crypter for PCCrypter {
    fn crypt(&mut self, data: &mut [u8]) -> Result<(), EncryptionError> {
        let data = data.as_mut_slice_of::<u32>()?;

        for dword in data.iter_mut() {
            *dword ^= self.next().to_le();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use claim::*;

    use super::*;

    #[test]
    fn pc_encrypt_decrypt() {
        let seed: u32 = 0x12345678;

        let decrypted = [
            0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x2c, 0x20, 0x77, 0x6f, 0x72, 0x6c, 0x64, 0x21, 0x00,
            0x00, 0x00,
        ];
        let encrypted = [
            0xde, 0xee, 0x84, 0xb6, 0xd6, 0x4c, 0x10, 0xbc, 0x07, 0x3c, 0x20, 0xca, 0x08, 0x20,
            0xee, 0xf0,
        ];

        let mut buffer = decrypted.clone();

        // encrypt data
        let mut encrypter = PCCrypter::new(seed);
        assert_ok!(encrypter.crypt(&mut buffer));
        assert_eq!(buffer, encrypted);

        // crypting the same buffer again with the same Crypter instance won't decrypt it
        let mut temp_buffer = buffer.clone();
        assert_ok!(encrypter.crypt(&mut temp_buffer));
        assert_ne!(temp_buffer, decrypted);

        // crypting the previous buffer with a new Crypter using the same seed, will decrypt it
        let mut decrypter = PCCrypter::new(seed);
        assert_ok!(decrypter.crypt(&mut buffer));
        assert_eq!(buffer, decrypted);
    }

    #[test]
    fn gc_encrypt_decrypt() {
        let seed: u32 = 0x12345678;

        let decrypted = [
            0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x2c, 0x20, 0x77, 0x6f, 0x72, 0x6c, 0x64, 0x21, 0x00,
            0x00, 0x00,
        ];
        let encrypted = [
            0x8a, 0x87, 0x5e, 0x68, 0x24, 0x01, 0xee, 0xac, 0xd6, 0x82, 0x07, 0xff, 0x2b, 0xa5,
            0x92, 0x2b,
        ];

        let mut buffer = decrypted.clone();

        // encrypt data
        let mut encrypter = GCCrypter::new(seed);
        assert_ok!(encrypter.crypt(&mut buffer));
        assert_eq!(buffer, encrypted);

        // crypting the same buffer again with the same Crypter instance won't decrypt it
        let mut temp_buffer = buffer.clone();
        assert_ok!(encrypter.crypt(&mut temp_buffer));
        assert_ne!(temp_buffer, decrypted);

        // crypting the previous buffer with a new Crypter using the same seed, will decrypt it
        let mut decrypter = GCCrypter::new(seed);
        assert_ok!(decrypter.crypt(&mut buffer));
        assert_eq!(buffer, decrypted);
    }

    #[test]
    fn pc_crypt_non_dword_sized_data_returns_error() {
        let mut crypter = PCCrypter::new(0x12345678);

        // too small. 3 bytes, not dword-sized
        let mut bad_data = [0x01, 0x02, 0x03];
        assert_matches!(
            crypter.crypt(&mut bad_data),
            Err(EncryptionError::InputDataCastingError(_))
        );

        // too big. 5 bytes, also not dword-sized
        let mut bad_data = [0x01, 0x02, 0x03, 0x04, 0x05];
        assert_matches!(
            crypter.crypt(&mut bad_data),
            Err(EncryptionError::InputDataCastingError(_))
        );

        // good. dword-sized
        let mut good_data = [0x01, 0x02, 0x03, 0x04];
        assert_ok!(crypter.crypt(&mut good_data));
    }

    #[test]
    fn gc_crypt_non_dword_sized_data_returns_error() {
        let mut crypter = GCCrypter::new(0x12345678);

        // too small. 3 bytes, not dword-sized
        let mut bad_data = [0x01, 0x02, 0x03];
        assert_matches!(
            crypter.crypt(&mut bad_data),
            Err(EncryptionError::InputDataCastingError(_))
        );

        // too big. 5 bytes, also not dword-sized
        let mut bad_data = [0x01, 0x02, 0x03, 0x04, 0x05];
        assert_matches!(
            crypter.crypt(&mut bad_data),
            Err(EncryptionError::InputDataCastingError(_))
        );

        // good. dword-sized
        let mut good_data = [0x01, 0x02, 0x03, 0x04];
        assert_ok!(crypter.crypt(&mut good_data));
    }

    #[test]
    fn pc_encrypt_multiple_things_and_decrypt_multiple_things() {
        let seed: u32 = 0x42424242;

        let first_decrypted = [0x46, 0x69, 0x72, 0x73, 0x74, 0x21, 0x21, 0x00];
        let second_decrypted = [
            0x53, 0x65, 0x63, 0x6f, 0x6e, 0x64, 0x20, 0x62, 0x69, 0x74, 0x20, 0x6f, 0x66, 0x20,
            0x64, 0x61, 0x74, 0x61, 0x00, 0x00,
        ];

        let first_encrypted = [0xf4, 0x41, 0x19, 0x58, 0xa3, 0x2d, 0xbc, 0x67];
        let second_encrypted = [
            0x9d, 0x08, 0xee, 0xec, 0x89, 0x7f, 0xac, 0x66, 0xef, 0x18, 0x9c, 0xc4, 0xa9, 0x84,
            0x34, 0xa1, 0x90, 0x76, 0x71, 0xea,
        ];

        let mut encrypter = PCCrypter::new(seed);

        let mut first_buffer = first_decrypted.clone();
        assert_ok!(encrypter.crypt(&mut first_buffer));
        assert_eq!(first_encrypted, first_buffer);

        let mut second_buffer = second_decrypted.clone();
        assert_ok!(encrypter.crypt(&mut second_buffer));
        assert_eq!(second_encrypted, second_buffer);

        let mut decrypter = PCCrypter::new(seed);

        assert_ok!(decrypter.crypt(&mut first_buffer));
        assert_eq!(first_decrypted, first_buffer);

        assert_ok!(decrypter.crypt(&mut second_buffer));
        assert_eq!(second_decrypted, second_buffer);
    }

    #[test]
    fn gc_encrypt_multiple_things_and_decrypt_multiple_things() {
        let seed: u32 = 0x42424242;

        let first_decrypted = [0x46, 0x69, 0x72, 0x73, 0x74, 0x21, 0x21, 0x00];
        let second_decrypted = [
            0x53, 0x65, 0x63, 0x6f, 0x6e, 0x64, 0x20, 0x62, 0x69, 0x74, 0x20, 0x6f, 0x66, 0x20,
            0x64, 0x61, 0x74, 0x61, 0x00, 0x00,
        ];

        let first_encrypted = [0xda, 0x5a, 0x14, 0xab, 0x2c, 0x0a, 0x50, 0x07];
        let second_encrypted = [
            0xc4, 0x17, 0x16, 0xa3, 0x48, 0xf1, 0x9c, 0x8d, 0x8e, 0x71, 0xdd, 0x46, 0xe2, 0x09,
            0xce, 0x38, 0xf9, 0xd3, 0xdb, 0x7c,
        ];

        let mut encrypter = GCCrypter::new(seed);

        let mut first_buffer = first_decrypted.clone();
        assert_ok!(encrypter.crypt(&mut first_buffer));
        assert_eq!(first_encrypted, first_buffer);

        let mut second_buffer = second_decrypted.clone();
        assert_ok!(encrypter.crypt(&mut second_buffer));
        assert_eq!(second_encrypted, second_buffer);

        let mut decrypter = GCCrypter::new(seed);

        assert_ok!(decrypter.crypt(&mut first_buffer));
        assert_eq!(first_decrypted, first_buffer);

        assert_ok!(decrypter.crypt(&mut second_buffer));
        assert_eq!(second_decrypted, second_buffer);
    }

    #[test]
    fn pc_encrypt_and_decrypt_bigger_data() {
        let seed: u32 = 0xabcdef;

        // these blocks of data are specifically intended to be larger than the PC encryption
        // algorithm's "stream length", so we can test the wrap-around logic works too

        let decrypted = [
            0x4c, 0x6f, 0x72, 0x65, 0x6d, 0x20, 0x69, 0x70, 0x73, 0x75, 0x6d, 0x20, 0x64, 0x6f,
            0x6c, 0x6f, 0x72, 0x20, 0x73, 0x69, 0x74, 0x20, 0x61, 0x6d, 0x65, 0x74, 0x2c, 0x20,
            0x63, 0x6f, 0x6e, 0x73, 0x65, 0x63, 0x74, 0x65, 0x74, 0x75, 0x72, 0x20, 0x61, 0x64,
            0x69, 0x70, 0x69, 0x73, 0x63, 0x69, 0x6e, 0x67, 0x20, 0x65, 0x6c, 0x69, 0x74, 0x2e,
            0x20, 0x4e, 0x61, 0x6d, 0x20, 0x65, 0x67, 0x65, 0x73, 0x74, 0x61, 0x73, 0x20, 0x64,
            0x69, 0x63, 0x74, 0x75, 0x6d, 0x20, 0x65, 0x72, 0x6f, 0x73, 0x20, 0x6e, 0x6f, 0x6e,
            0x20, 0x6c, 0x75, 0x63, 0x74, 0x75, 0x73, 0x2e, 0x20, 0x50, 0x65, 0x6c, 0x6c, 0x65,
            0x6e, 0x74, 0x65, 0x73, 0x71, 0x75, 0x65, 0x20, 0x6e, 0x75, 0x6e, 0x63, 0x20, 0x70,
            0x75, 0x72, 0x75, 0x73, 0x2c, 0x20, 0x73, 0x75, 0x73, 0x63, 0x69, 0x70, 0x69, 0x74,
            0x20, 0x76, 0x65, 0x6c, 0x20, 0x65, 0x78, 0x20, 0x69, 0x6e, 0x2c, 0x20, 0x73, 0x6f,
            0x6c, 0x6c, 0x69, 0x63, 0x69, 0x74, 0x75, 0x64, 0x69, 0x6e, 0x20, 0x66, 0x69, 0x6e,
            0x69, 0x62, 0x75, 0x73, 0x20, 0x64, 0x6f, 0x6c, 0x6f, 0x72, 0x2e, 0x20, 0x41, 0x6c,
            0x69, 0x71, 0x75, 0x61, 0x6d, 0x20, 0x61, 0x6c, 0x69, 0x71, 0x75, 0x61, 0x6d, 0x20,
            0x73, 0x65, 0x6d, 0x20, 0x6a, 0x75, 0x73, 0x74, 0x6f, 0x2c, 0x20, 0x76, 0x69, 0x74,
            0x61, 0x65, 0x20, 0x70, 0x6f, 0x73, 0x75, 0x65, 0x72, 0x65, 0x20, 0x65, 0x72, 0x61,
            0x74, 0x20, 0x69, 0x6e, 0x74, 0x65, 0x72, 0x64, 0x75, 0x6d, 0x20, 0x6e, 0x65, 0x63,
            0x2e, 0x20, 0x4e, 0x75, 0x6e, 0x63, 0x20, 0x73, 0x69, 0x74, 0x20, 0x61, 0x6d, 0x65,
            0x74, 0x20, 0x65, 0x6c, 0x65, 0x69, 0x66, 0x65, 0x6e, 0x64, 0x20, 0x65, 0x6e, 0x69,
            0x6d, 0x2e, 0x20, 0x4d, 0x6f, 0x72, 0x62, 0x69, 0x20, 0x71, 0x75, 0x69, 0x73, 0x20,
            0x75, 0x6c, 0x6c, 0x61, 0x6d, 0x63, 0x6f, 0x72, 0x70, 0x65, 0x72, 0x20, 0x6d, 0x61,
            0x75, 0x72, 0x69, 0x73, 0x2e, 0x20, 0x50, 0x72, 0x6f, 0x69, 0x6e, 0x20, 0x6c, 0x61,
            0x63, 0x75, 0x73, 0x20, 0x74, 0x65, 0x6c, 0x6c, 0x75, 0x73, 0x2c, 0x20, 0x61, 0x75,
            0x63, 0x74, 0x6f, 0x72, 0x20, 0x71, 0x75, 0x69, 0x73, 0x20, 0x6f, 0x64, 0x69, 0x6f,
            0x20, 0x6e, 0x6f, 0x6e, 0x2c, 0x20, 0x6d, 0x6f, 0x6c, 0x6c, 0x69, 0x73, 0x20, 0x74,
            0x65, 0x6d, 0x70, 0x6f, 0x72, 0x20, 0x6d, 0x61, 0x73, 0x73, 0x61, 0x2e, 0x20, 0x50,
            0x68, 0x61, 0x73, 0x65, 0x6c, 0x6c, 0x75, 0x73, 0x20, 0x66, 0x65, 0x75, 0x67, 0x69,
            0x61, 0x74, 0x20, 0x69, 0x70, 0x73, 0x75, 0x6d, 0x20, 0x61, 0x74, 0x20, 0x69, 0x6d,
            0x70, 0x65, 0x72, 0x64, 0x69, 0x65, 0x74, 0x20, 0x66, 0x61, 0x63, 0x69, 0x6c, 0x69,
            0x73, 0x69, 0x73, 0x2e, 0x20, 0x50, 0x72, 0x61, 0x65, 0x73, 0x65, 0x6e, 0x74, 0x20,
            0x70, 0x68, 0x61, 0x72, 0x65, 0x74, 0x72, 0x61, 0x20, 0x61, 0x75, 0x67, 0x75, 0x65,
            0x20, 0x6e, 0x6f, 0x6e, 0x20, 0x6f, 0x64, 0x69, 0x6f, 0x20, 0x63, 0x6f, 0x6e, 0x67,
            0x75, 0x65, 0x20, 0x74, 0x72, 0x69, 0x73, 0x74, 0x69, 0x71, 0x75, 0x65, 0x2e, 0x20,
            0x50, 0x72, 0x6f, 0x69, 0x6e, 0x20, 0x73, 0x61, 0x67, 0x69, 0x74, 0x74, 0x69, 0x73,
            0x20, 0x66, 0x65, 0x72, 0x6d, 0x65, 0x6e, 0x74, 0x75, 0x6d, 0x20, 0x6c, 0x61, 0x63,
            0x75, 0x73, 0x2c, 0x20, 0x73, 0x69, 0x74, 0x20, 0x61, 0x6d, 0x65, 0x74, 0x20, 0x76,
            0x69, 0x76, 0x65, 0x72, 0x72, 0x61, 0x20, 0x61, 0x72, 0x63, 0x75, 0x20, 0x63, 0x6f,
            0x6e, 0x73, 0x65, 0x63, 0x74, 0x65, 0x74, 0x75, 0x72, 0x20, 0x61, 0x2e, 0x20, 0x43,
            0x75, 0x72, 0x61, 0x62, 0x69, 0x74, 0x75, 0x72, 0x20, 0x74, 0x69, 0x6e, 0x63, 0x69,
            0x64, 0x75, 0x6e, 0x74, 0x20, 0x6e, 0x6f, 0x6e, 0x20, 0x6c, 0x6f, 0x72, 0x65, 0x6d,
            0x20, 0x76, 0x69, 0x74, 0x61, 0x65, 0x20, 0x6c, 0x61, 0x6f, 0x72, 0x65, 0x65, 0x74,
            0x2e, 0x20, 0x49, 0x6e, 0x20, 0x64, 0x69, 0x63, 0x74, 0x75, 0x6d, 0x20, 0x74, 0x65,
            0x6d, 0x70, 0x75, 0x73, 0x20, 0x74, 0x69, 0x6e, 0x63, 0x69, 0x64, 0x75, 0x6e, 0x74,
            0x2e, 0x20, 0x46, 0x75, 0x73, 0x63, 0x65, 0x20, 0x71, 0x75, 0x69, 0x73, 0x20, 0x6d,
            0x69, 0x20, 0x73, 0x65, 0x64, 0x20, 0x65, 0x72, 0x6f, 0x73, 0x20, 0x63, 0x6f, 0x6d,
            0x6d, 0x6f, 0x64, 0x6f, 0x20, 0x76, 0x65, 0x6e, 0x65, 0x6e, 0x61, 0x74, 0x69, 0x73,
            0x2e, 0x20, 0x51, 0x75, 0x69, 0x73, 0x71, 0x75, 0x65, 0x20, 0x65, 0x67, 0x65, 0x73,
            0x74, 0x61, 0x73, 0x20, 0x64, 0x6f, 0x6c, 0x6f, 0x72, 0x20, 0x65, 0x74, 0x20, 0x6e,
            0x75, 0x6e, 0x63, 0x20, 0x64, 0x69, 0x63, 0x74, 0x75, 0x6d, 0x20, 0x62, 0x6c, 0x61,
            0x6e, 0x64, 0x69, 0x74, 0x2e, 0x20, 0x56, 0x65, 0x73, 0x74, 0x69, 0x62, 0x75, 0x6c,
            0x75, 0x6d, 0x20, 0x65, 0x75, 0x20, 0x6c, 0x69, 0x62, 0x65, 0x72, 0x6f, 0x20, 0x65,
            0x67, 0x65, 0x74, 0x20, 0x61, 0x6e, 0x74, 0x65, 0x20, 0x76, 0x61, 0x72, 0x69, 0x75,
            0x73, 0x20, 0x70, 0x6c, 0x61, 0x63, 0x65, 0x72, 0x61, 0x74, 0x20, 0x65, 0x67, 0x65,
            0x74, 0x20, 0x75, 0x74, 0x20, 0x6e, 0x69, 0x62, 0x68, 0x2e, 0x00, 0x00,
        ];

        let encrypted = [
            0x3c, 0x76, 0x78, 0x22, 0x53, 0x33, 0x9b, 0x87, 0x0a, 0x02, 0x45, 0xf6, 0xfa, 0xcd,
            0x95, 0x84, 0xc6, 0xc9, 0x3e, 0x89, 0x23, 0x51, 0x08, 0x77, 0x30, 0xaf, 0x34, 0xd3,
            0xb0, 0x44, 0xe1, 0x17, 0x29, 0x23, 0x51, 0x0d, 0x0e, 0x3d, 0xff, 0xe1, 0x0c, 0xd2,
            0xe0, 0xa1, 0xce, 0xd3, 0x2c, 0x6d, 0xc1, 0x03, 0x86, 0x85, 0x0c, 0x10, 0xce, 0x02,
            0x15, 0xb3, 0x0a, 0x3c, 0x6a, 0x43, 0x76, 0x49, 0xd7, 0x11, 0xe9, 0x4e, 0x5b, 0x8f,
            0x43, 0x1b, 0x0f, 0xfa, 0x3a, 0xd2, 0x62, 0xc5, 0x51, 0x2b, 0x0f, 0xf8, 0x18, 0xbc,
            0xa3, 0x4a, 0xc8, 0xe0, 0x7c, 0xb8, 0xc1, 0x06, 0x36, 0xa1, 0xa4, 0xbe, 0x75, 0x4f,
            0xc0, 0xe2, 0xe6, 0xd4, 0x7d, 0x3c, 0x4e, 0x1d, 0x72, 0xc1, 0x38, 0xc2, 0xf0, 0x3e,
            0x8d, 0x28, 0x11, 0xae, 0x4b, 0x2d, 0xf2, 0x89, 0x32, 0xd8, 0x2d, 0x89, 0xb6, 0x33,
            0xe7, 0x2d, 0xa1, 0xd9, 0x46, 0x8e, 0xf0, 0x0d, 0x9f, 0xf3, 0xa1, 0xe0, 0x7a, 0xe9,
            0x50, 0xce, 0x34, 0x0f, 0xff, 0xd2, 0x4d, 0x0b, 0x30, 0xc5, 0xb5, 0x8c, 0x58, 0x75,
            0x84, 0x3b, 0x7e, 0xa5, 0x95, 0x99, 0xac, 0x7c, 0x22, 0x9b, 0xfe, 0x26, 0xd2, 0x3c,
            0xf3, 0xa7, 0xbd, 0x5f, 0x02, 0xcb, 0xa5, 0xcc, 0xa7, 0xc9, 0x78, 0xc2, 0x39, 0x7e,
            0xf2, 0x76, 0xf4, 0x38, 0x67, 0xbf, 0x8e, 0xad, 0x6f, 0x02, 0xdb, 0x4b, 0x6a, 0x5b,
            0x59, 0xd9, 0xbb, 0x0b, 0xe9, 0xf0, 0xb3, 0x44, 0x52, 0x53, 0x0d, 0x20, 0xb6, 0x4b,
            0x32, 0x0f, 0x7c, 0x5c, 0x67, 0x2f, 0xd9, 0x1a, 0x75, 0xde, 0xb1, 0xbf, 0x27, 0x88,
            0x54, 0x7d, 0xc5, 0x79, 0x9f, 0x2a, 0x12, 0x4b, 0x78, 0x96, 0xcf, 0x04, 0x15, 0x22,
            0x84, 0x53, 0xa4, 0xa6, 0x55, 0xc2, 0x9a, 0x4a, 0xed, 0x6c, 0x82, 0x75, 0xcc, 0x63,
            0x2c, 0x44, 0x4f, 0x27, 0xd8, 0x45, 0x22, 0xb1, 0xbd, 0xde, 0x83, 0xe9, 0x7e, 0xea,
            0xf3, 0xa9, 0x2c, 0x18, 0x8c, 0x5c, 0xfd, 0xb2, 0xdc, 0xec, 0x93, 0xbe, 0x87, 0x5c,
            0xc4, 0x7f, 0x6d, 0x11, 0x89, 0xab, 0xd7, 0x7d, 0xef, 0xc4, 0x49, 0x69, 0x2f, 0xb2,
            0xd8, 0x03, 0xf2, 0x13, 0x0c, 0x53, 0x63, 0x0c, 0x3f, 0xfe, 0x93, 0xdb, 0x17, 0x21,
            0x90, 0xee, 0xf0, 0xac, 0x4b, 0x03, 0xb4, 0x76, 0xfb, 0x78, 0x04, 0xcf, 0x60, 0x25,
            0xa1, 0x52, 0x55, 0x9d, 0xc5, 0x5b, 0x28, 0xd0, 0x8c, 0x84, 0xe9, 0x60, 0x54, 0x1d,
            0xc3, 0x2f, 0x20, 0x3e, 0x37, 0xab, 0xac, 0x91, 0x4e, 0x44, 0x44, 0x7f, 0xa3, 0x1b,
            0x9f, 0xe1, 0xa2, 0x90, 0xd9, 0xa9, 0x85, 0x63, 0x33, 0x63, 0x4a, 0xad, 0xb1, 0xcf,
            0x37, 0x59, 0x77, 0x46, 0xb7, 0x99, 0x9d, 0x0d, 0x70, 0x1d, 0x76, 0x3c, 0x33, 0xa5,
            0xc1, 0xfe, 0x6e, 0xe1, 0xac, 0xbc, 0x24, 0x79, 0x0d, 0x66, 0x34, 0x6a, 0x61, 0xa1,
            0x9d, 0xde, 0x3f, 0x44, 0x9f, 0x08, 0xb1, 0x74, 0xf0, 0x11, 0x6f, 0xd1, 0xd2, 0x5d,
            0x1d, 0x83, 0xf3, 0x15, 0x5a, 0x7a, 0x01, 0x84, 0xb7, 0xe2, 0x5a, 0x15, 0x6f, 0x5a,
            0x6c, 0xfe, 0xb3, 0xcb, 0xfb, 0x19, 0x28, 0x35, 0x2b, 0x37, 0xb1, 0xaa, 0x01, 0x88,
            0xb7, 0x9d, 0x46, 0x87, 0x4c, 0xab, 0x27, 0xee, 0x74, 0xeb, 0x82, 0x74, 0xba, 0xab,
            0x70, 0x26, 0x13, 0x1b, 0x4f, 0xf1, 0xaf, 0x01, 0x2e, 0x06, 0x6d, 0xb9, 0x02, 0xee,
            0xf9, 0x1d, 0x50, 0x37, 0xf7, 0xc2, 0x3c, 0xe0, 0xea, 0x83, 0xc7, 0xcd, 0xdc, 0xad,
            0xee, 0xc1, 0x56, 0xde, 0x3e, 0x3f, 0xff, 0x59, 0xd7, 0xab, 0x1c, 0x89, 0x72, 0xb7,
            0xfd, 0xa3, 0xb6, 0x15, 0x9b, 0x12, 0x6c, 0x5d, 0x92, 0x1d, 0x7e, 0xb0, 0xf5, 0x19,
            0x7b, 0x57, 0x2d, 0x62, 0x79, 0xad, 0xfb, 0xb0, 0x66, 0x41, 0xc0, 0x19, 0x15, 0xe0,
            0xee, 0xe2, 0x55, 0x8b, 0x94, 0x44, 0x0e, 0x96, 0x84, 0xfa, 0xed, 0xc5, 0xbf, 0x8c,
            0x61, 0x0a, 0xec, 0x29, 0x14, 0xd0, 0x22, 0x7f, 0x32, 0x54, 0x82, 0xc2, 0x7f, 0xf2,
            0x4d, 0x7f, 0x4d, 0x9a, 0x62, 0xed, 0x17, 0xc8, 0x3b, 0xf3, 0x49, 0xc0, 0x13, 0xa1,
            0x3e, 0x66, 0x6e, 0x27, 0xcb, 0xc6, 0xec, 0x01, 0xe8, 0xdc, 0x54, 0x92, 0x42, 0x26,
            0x56, 0xb7, 0xd6, 0xc9, 0xa7, 0xff, 0x10, 0x7f, 0x3e, 0xc0, 0x60, 0x19, 0xac, 0x2d,
            0xda, 0xa2, 0xb9, 0x99, 0x77, 0x23, 0x47, 0xbd, 0x3e, 0x4d, 0x72, 0x56, 0x27, 0x0c,
            0x14, 0xf8, 0x30, 0xf4, 0xbf, 0x61, 0x26, 0xd0, 0x04, 0xe3, 0x99, 0x77, 0xde, 0xb4,
            0xe6, 0x00, 0xa1, 0x8b, 0x3a, 0x08, 0x00, 0x5e, 0x47, 0xbc, 0xf1, 0x71, 0xe4, 0x9b,
            0x92, 0x90, 0x6e, 0x52, 0x23, 0x01, 0x6c, 0x4f, 0x48, 0xae, 0x57, 0x96, 0x0b, 0xef,
            0xc3, 0xe9, 0x3b, 0xf4, 0x69, 0x1c, 0x1b, 0x46, 0x46, 0x6a, 0x29, 0x57, 0x76, 0xc3,
            0x62, 0x17, 0x0a, 0xd7, 0xf3, 0x5e, 0x38, 0x1c, 0x2f, 0xb4, 0xca, 0x72, 0x2d, 0xca,
            0x10, 0x72, 0x3c, 0xa1, 0xfe, 0x7d, 0xea, 0x46, 0x14, 0x45, 0x7e, 0x40, 0x34, 0xae,
            0xef, 0xd7, 0x6e, 0x31, 0x08, 0x71, 0xf4, 0x00, 0xc0, 0xcc, 0xe6, 0x3e, 0xdd, 0x40,
            0x6d, 0xa0, 0xdb, 0x17, 0x12, 0x4a, 0x7a, 0x08, 0xb9, 0xda, 0x82, 0x89, 0x21, 0x8d,
            0x50, 0xaf, 0x42, 0xd2, 0x1b, 0x2d, 0x8c, 0xcf, 0x64, 0x05, 0xa8, 0x5e, 0xec, 0x35,
            0xba, 0x80, 0x30, 0x27, 0xd7, 0x48, 0x1d, 0xcb, 0x6b, 0x9c, 0x2c, 0xf4,
        ];

        let mut buffer = decrypted.clone();

        let mut encrypter = PCCrypter::new(seed);
        assert_ok!(encrypter.crypt(&mut buffer));
        assert_eq!(encrypted, buffer);

        let mut decrypter = PCCrypter::new(seed);
        assert_ok!(decrypter.crypt(&mut buffer));
        assert_eq!(decrypted, buffer);
    }

    #[test]
    fn gc_encrypt_and_decrypt_bigger_data() {
        let seed: u32 = 0xabcdef;

        // these blocks of data are specifically intended to be larger than the Gamecube encryption
        // algorithm's "stream length", so we can test the wrap-around logic works too

        let decrypted = [
            0x4c, 0x6f, 0x72, 0x65, 0x6d, 0x20, 0x69, 0x70, 0x73, 0x75, 0x6d, 0x20, 0x64, 0x6f,
            0x6c, 0x6f, 0x72, 0x20, 0x73, 0x69, 0x74, 0x20, 0x61, 0x6d, 0x65, 0x74, 0x2c, 0x20,
            0x63, 0x6f, 0x6e, 0x73, 0x65, 0x63, 0x74, 0x65, 0x74, 0x75, 0x72, 0x20, 0x61, 0x64,
            0x69, 0x70, 0x69, 0x73, 0x63, 0x69, 0x6e, 0x67, 0x20, 0x65, 0x6c, 0x69, 0x74, 0x2e,
            0x20, 0x4e, 0x61, 0x6d, 0x20, 0x65, 0x67, 0x65, 0x73, 0x74, 0x61, 0x73, 0x20, 0x64,
            0x69, 0x63, 0x74, 0x75, 0x6d, 0x20, 0x65, 0x72, 0x6f, 0x73, 0x20, 0x6e, 0x6f, 0x6e,
            0x20, 0x6c, 0x75, 0x63, 0x74, 0x75, 0x73, 0x2e, 0x20, 0x50, 0x65, 0x6c, 0x6c, 0x65,
            0x6e, 0x74, 0x65, 0x73, 0x71, 0x75, 0x65, 0x20, 0x6e, 0x75, 0x6e, 0x63, 0x20, 0x70,
            0x75, 0x72, 0x75, 0x73, 0x2c, 0x20, 0x73, 0x75, 0x73, 0x63, 0x69, 0x70, 0x69, 0x74,
            0x20, 0x76, 0x65, 0x6c, 0x20, 0x65, 0x78, 0x20, 0x69, 0x6e, 0x2c, 0x20, 0x73, 0x6f,
            0x6c, 0x6c, 0x69, 0x63, 0x69, 0x74, 0x75, 0x64, 0x69, 0x6e, 0x20, 0x66, 0x69, 0x6e,
            0x69, 0x62, 0x75, 0x73, 0x20, 0x64, 0x6f, 0x6c, 0x6f, 0x72, 0x2e, 0x20, 0x41, 0x6c,
            0x69, 0x71, 0x75, 0x61, 0x6d, 0x20, 0x61, 0x6c, 0x69, 0x71, 0x75, 0x61, 0x6d, 0x20,
            0x73, 0x65, 0x6d, 0x20, 0x6a, 0x75, 0x73, 0x74, 0x6f, 0x2c, 0x20, 0x76, 0x69, 0x74,
            0x61, 0x65, 0x20, 0x70, 0x6f, 0x73, 0x75, 0x65, 0x72, 0x65, 0x20, 0x65, 0x72, 0x61,
            0x74, 0x20, 0x69, 0x6e, 0x74, 0x65, 0x72, 0x64, 0x75, 0x6d, 0x20, 0x6e, 0x65, 0x63,
            0x2e, 0x20, 0x4e, 0x75, 0x6e, 0x63, 0x20, 0x73, 0x69, 0x74, 0x20, 0x61, 0x6d, 0x65,
            0x74, 0x20, 0x65, 0x6c, 0x65, 0x69, 0x66, 0x65, 0x6e, 0x64, 0x20, 0x65, 0x6e, 0x69,
            0x6d, 0x2e, 0x20, 0x4d, 0x6f, 0x72, 0x62, 0x69, 0x20, 0x71, 0x75, 0x69, 0x73, 0x20,
            0x75, 0x6c, 0x6c, 0x61, 0x6d, 0x63, 0x6f, 0x72, 0x70, 0x65, 0x72, 0x20, 0x6d, 0x61,
            0x75, 0x72, 0x69, 0x73, 0x2e, 0x20, 0x50, 0x72, 0x6f, 0x69, 0x6e, 0x20, 0x6c, 0x61,
            0x63, 0x75, 0x73, 0x20, 0x74, 0x65, 0x6c, 0x6c, 0x75, 0x73, 0x2c, 0x20, 0x61, 0x75,
            0x63, 0x74, 0x6f, 0x72, 0x20, 0x71, 0x75, 0x69, 0x73, 0x20, 0x6f, 0x64, 0x69, 0x6f,
            0x20, 0x6e, 0x6f, 0x6e, 0x2c, 0x20, 0x6d, 0x6f, 0x6c, 0x6c, 0x69, 0x73, 0x20, 0x74,
            0x65, 0x6d, 0x70, 0x6f, 0x72, 0x20, 0x6d, 0x61, 0x73, 0x73, 0x61, 0x2e, 0x20, 0x50,
            0x68, 0x61, 0x73, 0x65, 0x6c, 0x6c, 0x75, 0x73, 0x20, 0x66, 0x65, 0x75, 0x67, 0x69,
            0x61, 0x74, 0x20, 0x69, 0x70, 0x73, 0x75, 0x6d, 0x20, 0x61, 0x74, 0x20, 0x69, 0x6d,
            0x70, 0x65, 0x72, 0x64, 0x69, 0x65, 0x74, 0x20, 0x66, 0x61, 0x63, 0x69, 0x6c, 0x69,
            0x73, 0x69, 0x73, 0x2e, 0x20, 0x50, 0x72, 0x61, 0x65, 0x73, 0x65, 0x6e, 0x74, 0x20,
            0x70, 0x68, 0x61, 0x72, 0x65, 0x74, 0x72, 0x61, 0x20, 0x61, 0x75, 0x67, 0x75, 0x65,
            0x20, 0x6e, 0x6f, 0x6e, 0x20, 0x6f, 0x64, 0x69, 0x6f, 0x20, 0x63, 0x6f, 0x6e, 0x67,
            0x75, 0x65, 0x20, 0x74, 0x72, 0x69, 0x73, 0x74, 0x69, 0x71, 0x75, 0x65, 0x2e, 0x20,
            0x50, 0x72, 0x6f, 0x69, 0x6e, 0x20, 0x73, 0x61, 0x67, 0x69, 0x74, 0x74, 0x69, 0x73,
            0x20, 0x66, 0x65, 0x72, 0x6d, 0x65, 0x6e, 0x74, 0x75, 0x6d, 0x20, 0x6c, 0x61, 0x63,
            0x75, 0x73, 0x2c, 0x20, 0x73, 0x69, 0x74, 0x20, 0x61, 0x6d, 0x65, 0x74, 0x20, 0x76,
            0x69, 0x76, 0x65, 0x72, 0x72, 0x61, 0x20, 0x61, 0x72, 0x63, 0x75, 0x20, 0x63, 0x6f,
            0x6e, 0x73, 0x65, 0x63, 0x74, 0x65, 0x74, 0x75, 0x72, 0x20, 0x61, 0x2e, 0x20, 0x43,
            0x75, 0x72, 0x61, 0x62, 0x69, 0x74, 0x75, 0x72, 0x20, 0x74, 0x69, 0x6e, 0x63, 0x69,
            0x64, 0x75, 0x6e, 0x74, 0x20, 0x6e, 0x6f, 0x6e, 0x20, 0x6c, 0x6f, 0x72, 0x65, 0x6d,
            0x20, 0x76, 0x69, 0x74, 0x61, 0x65, 0x20, 0x6c, 0x61, 0x6f, 0x72, 0x65, 0x65, 0x74,
            0x2e, 0x20, 0x49, 0x6e, 0x20, 0x64, 0x69, 0x63, 0x74, 0x75, 0x6d, 0x20, 0x74, 0x65,
            0x6d, 0x70, 0x75, 0x73, 0x20, 0x74, 0x69, 0x6e, 0x63, 0x69, 0x64, 0x75, 0x6e, 0x74,
            0x2e, 0x20, 0x46, 0x75, 0x73, 0x63, 0x65, 0x20, 0x71, 0x75, 0x69, 0x73, 0x20, 0x6d,
            0x69, 0x20, 0x73, 0x65, 0x64, 0x20, 0x65, 0x72, 0x6f, 0x73, 0x20, 0x63, 0x6f, 0x6d,
            0x6d, 0x6f, 0x64, 0x6f, 0x20, 0x76, 0x65, 0x6e, 0x65, 0x6e, 0x61, 0x74, 0x69, 0x73,
            0x2e, 0x20, 0x51, 0x75, 0x69, 0x73, 0x71, 0x75, 0x65, 0x20, 0x65, 0x67, 0x65, 0x73,
            0x74, 0x61, 0x73, 0x20, 0x64, 0x6f, 0x6c, 0x6f, 0x72, 0x20, 0x65, 0x74, 0x20, 0x6e,
            0x75, 0x6e, 0x63, 0x20, 0x64, 0x69, 0x63, 0x74, 0x75, 0x6d, 0x20, 0x62, 0x6c, 0x61,
            0x6e, 0x64, 0x69, 0x74, 0x2e, 0x20, 0x56, 0x65, 0x73, 0x74, 0x69, 0x62, 0x75, 0x6c,
            0x75, 0x6d, 0x20, 0x65, 0x75, 0x20, 0x6c, 0x69, 0x62, 0x65, 0x72, 0x6f, 0x20, 0x65,
            0x67, 0x65, 0x74, 0x20, 0x61, 0x6e, 0x74, 0x65, 0x20, 0x76, 0x61, 0x72, 0x69, 0x75,
            0x73, 0x20, 0x70, 0x6c, 0x61, 0x63, 0x65, 0x72, 0x61, 0x74, 0x20, 0x65, 0x67, 0x65,
            0x74, 0x20, 0x75, 0x74, 0x20, 0x6e, 0x69, 0x62, 0x68, 0x2e, 0x00, 0x00,
        ];

        let encrypted = [
            0x3f, 0x41, 0xde, 0x72, 0x6a, 0x7b, 0x71, 0x63, 0x59, 0x9e, 0x0f, 0x81, 0x31, 0x16,
            0x6d, 0xe7, 0x73, 0x5f, 0x1a, 0xe1, 0xa1, 0xec, 0x78, 0x1d, 0xde, 0x0a, 0xf7, 0xcf,
            0x1d, 0xbd, 0x21, 0xe9, 0xcd, 0xd3, 0xb6, 0xc1, 0xf0, 0xb7, 0x65, 0x43, 0xd4, 0xcb,
            0xa0, 0xf5, 0x38, 0x0f, 0x19, 0xcc, 0x09, 0x4f, 0x60, 0x29, 0x37, 0x05, 0x6d, 0x9e,
            0x05, 0xd8, 0xdc, 0x8b, 0x51, 0x20, 0x94, 0x5b, 0x15, 0xe7, 0x99, 0x14, 0x6b, 0x7b,
            0x18, 0x3d, 0x4a, 0xdb, 0xcc, 0xfd, 0xe7, 0xdc, 0x8a, 0x3b, 0x1d, 0xf4, 0x4e, 0x8c,
            0xfb, 0xfa, 0xec, 0x01, 0x10, 0x0e, 0x1d, 0x26, 0xad, 0x0c, 0xc6, 0x39, 0xb9, 0x58,
            0x9f, 0x1e, 0x51, 0xc5, 0xb1, 0x4b, 0x86, 0x72, 0xec, 0x09, 0xb3, 0x8c, 0x70, 0xd6,
            0xbc, 0xbd, 0x83, 0xfd, 0x53, 0xe3, 0x5d, 0xc8, 0xc7, 0xfe, 0xb4, 0xe0, 0x7f, 0x90,
            0x62, 0xf4, 0x3a, 0x74, 0xd1, 0x5f, 0xa8, 0x00, 0xb7, 0xe7, 0x2b, 0xd0, 0x43, 0x43,
            0xbc, 0xee, 0x7d, 0x50, 0x74, 0xe6, 0x10, 0x16, 0xbf, 0xb0, 0xf4, 0x0a, 0x82, 0x87,
            0x39, 0x63, 0x30, 0x4b, 0x5a, 0xd7, 0xb4, 0x2c, 0xa0, 0x87, 0xae, 0x00, 0x11, 0x73,
            0xc0, 0x10, 0x1b, 0x64, 0xb3, 0x69, 0xdc, 0x18, 0xd0, 0x43, 0x0f, 0xfb, 0xb5, 0x85,
            0xd7, 0x1b, 0x7b, 0xeb, 0x06, 0xb4, 0x17, 0x9e, 0x42, 0x37, 0xc8, 0xe4, 0x59, 0x64,
            0x99, 0x18, 0xd4, 0x4f, 0xef, 0x16, 0x97, 0x32, 0xb3, 0x8a, 0x5b, 0x31, 0x7a, 0xbc,
            0x36, 0x1c, 0x0e, 0xd9, 0x80, 0x57, 0x61, 0x7b, 0x81, 0x34, 0x2f, 0xcc, 0x48, 0xcf,
            0x81, 0x65, 0x99, 0xfb, 0xd1, 0x8e, 0x78, 0x90, 0xe6, 0x2e, 0x7a, 0xc2, 0x46, 0x61,
            0x94, 0x57, 0x57, 0x55, 0xc8, 0xf1, 0x06, 0x0e, 0x7c, 0xa0, 0x25, 0xb8, 0x1c, 0x41,
            0x9d, 0x65, 0x5f, 0xee, 0xd6, 0x21, 0x15, 0xf8, 0xa7, 0xd8, 0x1d, 0x8c, 0xc6, 0x78,
            0x36, 0x75, 0x2e, 0x04, 0x04, 0xba, 0x43, 0x36, 0x87, 0x5c, 0x05, 0x1e, 0x83, 0xdc,
            0x0d, 0xb6, 0x2b, 0x7d, 0x87, 0xf2, 0xc4, 0xf5, 0x64, 0xd0, 0x3d, 0xa0, 0x11, 0x74,
            0xc1, 0x22, 0x23, 0x98, 0x07, 0xed, 0x3e, 0x24, 0xbd, 0xbc, 0xe2, 0x3e, 0x65, 0xaf,
            0x98, 0x63, 0x09, 0x31, 0xb5, 0x5f, 0x07, 0x9a, 0x43, 0xb1, 0xcc, 0x15, 0xf0, 0x45,
            0xcb, 0xe7, 0xa2, 0xf0, 0x35, 0x75, 0x0c, 0x50, 0xf9, 0xfa, 0xba, 0xf8, 0x59, 0xb8,
            0x14, 0x1d, 0x15, 0x02, 0x1c, 0xa7, 0x56, 0x1a, 0x7f, 0xd5, 0xdd, 0x6e, 0x45, 0x3c,
            0x97, 0x1d, 0xca, 0x20, 0x53, 0x44, 0xc6, 0xe7, 0xb4, 0xcb, 0x0a, 0xd8, 0x37, 0x7a,
            0x2a, 0x3d, 0x17, 0x52, 0x34, 0x38, 0x2f, 0x7f, 0x6f, 0x99, 0x0f, 0x55, 0x31, 0x2c,
            0xf2, 0xed, 0xe2, 0xf4, 0x53, 0xa7, 0x71, 0x45, 0x18, 0x3b, 0xa9, 0x80, 0xb8, 0x7f,
            0x26, 0xca, 0x6c, 0x5e, 0xb0, 0xcf, 0x8b, 0xa1, 0x4d, 0x5e, 0x2a, 0x47, 0x9f, 0x90,
            0x82, 0x79, 0xd9, 0x9c, 0xb7, 0xd2, 0x7c, 0xf0, 0xcc, 0xc1, 0xd9, 0xe9, 0x0d, 0xf7,
            0xc6, 0x2a, 0xc1, 0x20, 0x90, 0x83, 0x43, 0x7e, 0x7b, 0x57, 0xd9, 0xcf, 0x5f, 0x3d,
            0x46, 0x60, 0xed, 0x93, 0x7b, 0xce, 0x9e, 0xd2, 0xa4, 0xf1, 0xd0, 0xde, 0x82, 0x0c,
            0xc7, 0xf6, 0xee, 0x0d, 0x8d, 0xce, 0x79, 0x87, 0x55, 0x88, 0x46, 0x77, 0x94, 0xee,
            0xf2, 0xcf, 0x0e, 0x48, 0xab, 0x04, 0x5d, 0xb7, 0xbb, 0x98, 0x64, 0xc6, 0x7c, 0xe3,
            0x79, 0xaf, 0x61, 0xfc, 0x38, 0x89, 0x70, 0x7b, 0x08, 0xda, 0x46, 0x79, 0xbf, 0x52,
            0x41, 0x08, 0xc1, 0x0b, 0x0a, 0x47, 0x29, 0x81, 0x08, 0xcf, 0x8f, 0x44, 0xca, 0xdb,
            0x47, 0x71, 0x0c, 0x31, 0x00, 0x31, 0x8b, 0x51, 0xc4, 0x34, 0x2c, 0xd8, 0xe1, 0x52,
            0xcb, 0xcf, 0xd0, 0xea, 0x56, 0xb8, 0x8a, 0x13, 0x4b, 0x21, 0x8b, 0xec, 0xb8, 0xc2,
            0xe7, 0x10, 0x4f, 0xea, 0x76, 0x05, 0x8e, 0x69, 0x98, 0x21, 0xe0, 0x57, 0xad, 0xa2,
            0x82, 0x55, 0x7a, 0x2a, 0x45, 0x63, 0x21, 0x26, 0xfa, 0xdd, 0x4f, 0x13, 0x6d, 0x89,
            0x63, 0x6b, 0x17, 0x41, 0xdf, 0x7b, 0xed, 0x48, 0x0a, 0x5e, 0xc0, 0x76, 0xa0, 0x31,
            0x52, 0x32, 0x2b, 0x01, 0x64, 0x0c, 0x3f, 0xc3, 0x79, 0x4a, 0xf6, 0xc7, 0x92, 0xb7,
            0x0c, 0x27, 0x10, 0xee, 0x90, 0x89, 0xe6, 0x44, 0x20, 0x8f, 0xa5, 0x66, 0xc4, 0x67,
            0x26, 0xf4, 0x31, 0xa8, 0x39, 0x5f, 0x40, 0x6e, 0x23, 0xd0, 0x2a, 0x6d, 0x20, 0x2a,
            0xc2, 0x2c, 0xf2, 0x21, 0xb4, 0xb1, 0x0d, 0x20, 0xd4, 0x06, 0x9e, 0x24, 0xd7, 0xd9,
            0x44, 0xd6, 0x7a, 0x89, 0xa8, 0x7a, 0xf0, 0x96, 0x85, 0x96, 0xdd, 0xa6, 0xb9, 0xaf,
            0x9a, 0x2d, 0xbe, 0xd3, 0xd3, 0xdd, 0xc0, 0x37, 0xc6, 0x39, 0x84, 0x65, 0x61, 0x36,
            0xa6, 0xee, 0x5f, 0x9e, 0x3d, 0x98, 0xda, 0xed, 0xc6, 0xb4, 0x7f, 0x55, 0xe0, 0xca,
            0x3f, 0xf5, 0xb4, 0x7e, 0xf8, 0x16, 0x28, 0x7d, 0x84, 0x09, 0x30, 0x7f, 0xe1, 0x25,
            0x8b, 0xa7, 0x00, 0x53, 0xa3, 0x20, 0x19, 0x6a, 0x4f, 0x3d, 0xf9, 0x8c, 0x09, 0x62,
            0x9d, 0xf6, 0x86, 0x32, 0xfb, 0x93, 0x68, 0xb7, 0x1c, 0x6d, 0x04, 0x7c, 0x0c, 0x38,
            0x3a, 0x59, 0x99, 0xd1, 0xa0, 0x41, 0x20, 0xa6, 0xe0, 0x8c, 0xba, 0x1d, 0x1e, 0xbd,
            0x22, 0x81, 0x60, 0xb2, 0x6d, 0x09, 0xa9, 0x78, 0xed, 0x27, 0x45, 0xb5,
        ];

        let mut buffer = decrypted.clone();

        let mut encrypter = GCCrypter::new(seed);
        assert_ok!(encrypter.crypt(&mut buffer));
        assert_eq!(encrypted, buffer);

        let mut decrypter = GCCrypter::new(seed);
        assert_ok!(decrypter.crypt(&mut buffer));
        assert_eq!(decrypted, buffer);
    }
}
