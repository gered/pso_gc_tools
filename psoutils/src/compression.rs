struct Context {
    bitpos: u8,
    forward_log: Vec<u8>,
    output: Vec<u8>,
}

impl Context {
    pub fn new() -> Context {
        // tiny bug from the fuzziqer implementation? it never really initializes the forward log
        // anywhere (except, in newserv, as a zero-length std::string) and will ALWAYS start doing
        // some bit twiddling on the first byte before it ever actually explicitly adds the first
        // byte to the forward log ...
        let mut forward_log = Vec::new();
        forward_log.push(0);

        Context {
            bitpos: 0,
            forward_log,
            output: Vec::new(),
        }
    }

    pub fn put_control_bit_nosave(&mut self, bit: bool) {
        self.forward_log[0] >>= 1;
        self.forward_log[0] |= (bit as u8) << 7;
        self.bitpos += 1;
    }

    pub fn put_control_save(&mut self) {
        if self.bitpos >= 8 {
            self.bitpos = 0;
            self.output.append(&mut self.forward_log);
            self.forward_log.resize(1, 0);
            self.forward_log[0] = 0;
        }
    }

    pub fn put_static_data(&mut self, data: u8) {
        self.forward_log.push(data);
    }

    pub fn put_control_bit(&mut self, bit: bool) {
        self.put_control_bit_nosave(bit);
        self.put_control_save();
    }

    pub fn raw_byte(&mut self, value: u8) {
        self.put_control_bit_nosave(true);
        self.put_static_data(value);
        self.put_control_save();
    }

    pub fn short_copy(&mut self, offset: isize, size: u8) {
        let size = size - 2;
        self.put_control_bit(false);
        self.put_control_bit(false);
        self.put_control_bit((size >> 1) & 1 == 1);
        self.put_control_bit_nosave(size & 1 == 1);
        self.put_static_data((offset & 0xff) as u8);
        self.put_control_save();
    }

    pub fn long_copy(&mut self, offset: isize, size: u8) {
        if size <= 9 {
            self.put_control_bit(false);
            self.put_control_bit_nosave(true);
            self.put_static_data((((offset << 3) & 0xf8) as u8) | ((size - 2) & 0x07));
            self.put_static_data(((offset >> 5) & 0xff) as u8);
            self.put_control_save();
        } else {
            self.put_control_bit(false);
            self.put_control_bit_nosave(true);
            self.put_static_data(((offset << 3) & 0xf8) as u8);
            self.put_static_data(((offset >> 5) & 0xff) as u8);
            self.put_static_data(size - 1);
            self.put_control_save();
        }
    }

    pub fn copy(&mut self, offset: isize, size: u8) {
        if (offset > -0x100) && (size <= 5) {
            self.short_copy(offset, size);
        } else {
            self.long_copy(offset, size);
        }
    }

    pub fn finish(mut self) -> Box<[u8]> {
        self.put_control_bit(false);
        self.put_control_bit(true);
        if self.bitpos != 0 {
            self.forward_log[0] =
                (((self.forward_log[0] as u32) << (self.bitpos as u32)) >> 8) as u8;
        };
        self.put_static_data(0);
        self.put_static_data(0);
        self.output.append(&mut self.forward_log);
        self.output.into_boxed_slice()
    }
}

fn is_mem_equal(base: &[u8], offset1: isize, offset2: isize, length: usize) -> bool {
    // the fuzziqer prs compression implementation performs memcmp's that check memory slightly
    // outside of the buffers it is working with fairly often actually, despite the checks it
    // does in the main prs_compress loops ... ugh
    if offset1 < 0 || offset2 < 0 {
        false
    } else {
        let offset1 = offset1 as usize;
        let offset2 = offset2 as usize;
        if ((offset1 + length) > base.len()) || ((offset2 + length) > base.len()) {
            false
        } else {
            base[offset1..(offset1 + length)] == base[offset2..(offset2 + length)]
        }
    }
}

pub fn prs_compress(source: &[u8]) -> Box<[u8]> {
    let mut pc = Context::new();

    let mut x: isize = 0;
    while x < (source.len() as isize) {
        let mut lsoffset: isize = 0;
        let mut lssize: isize = 0;
        let mut xsize: usize = 0;

        let mut y: isize = x - 3;
        while (y > 0) && (y > (x - 0x1ff0)) && (xsize < 255) {
            xsize = 3;
            if is_mem_equal(source, y as isize, x as isize, xsize) {
                xsize += 1;
                while (xsize < 256)
                    && ((y + xsize as isize) < x)
                    && ((x + xsize as isize) <= (source.len() as isize))
                    && is_mem_equal(source, y as isize, x as isize, xsize)
                {
                    xsize += 1;
                }
                xsize -= 1;

                if (xsize as isize) > lssize {
                    lsoffset = -(x - y);
                    lssize = xsize as isize;
                }
            }
            y -= 1;
        }

        if lssize == 0 {
            pc.raw_byte(source[x as usize]);
        } else {
            pc.copy(lsoffset, lssize as u8);
            x += lssize - 1;
        }

        x += 1;
    }

    pc.finish()
}

enum Next {
    Byte(u8),
    Eof(),
}

struct ByteReader<'a> {
    source: &'a [u8],
    offset: usize,
}

impl<'a> ByteReader<'a> {
    pub fn new(source: &[u8]) -> ByteReader {
        ByteReader { source, offset: 0 }
    }

    pub fn next(&mut self) -> Next {
        if self.offset <= self.source.len() {
            let result = Next::Byte(self.source[self.offset]);
            self.offset += 1;
            result
        } else {
            Next::Eof()
        }
    }
}

pub fn prs_decompress(source: &[u8]) -> Box<[u8]> {
    let mut output = Vec::new();
    let mut reader = ByteReader::new(source);
    let mut r3: i32;
    let mut r5: i32;
    let mut bitpos = 9;
    let mut current_byte: u8;
    let mut flag: bool;
    let mut offset: i32;

    current_byte = match reader.next() {
        Next::Byte(byte) => byte,
        Next::Eof() => return output.into_boxed_slice(),
    };

    loop {
        bitpos -= 1;
        if bitpos == 0 {
            current_byte = match reader.next() {
                Next::Byte(byte) => byte,
                Next::Eof() => return output.into_boxed_slice(),
            };
            bitpos = 8;
        }

        flag = (current_byte & 1) == 1;
        current_byte >>= 1;
        if flag {
            output.push(match reader.next() {
                Next::Byte(byte) => byte,
                Next::Eof() => return output.into_boxed_slice(),
            });
            continue;
        }

        bitpos -= 1;
        if bitpos == 0 {
            current_byte = match reader.next() {
                Next::Byte(byte) => byte,
                Next::Eof() => return output.into_boxed_slice(),
            };
            bitpos = 8;
        }

        flag = (current_byte & 1) == 1;
        current_byte >>= 1;
        if flag {
            r3 = match reader.next() {
                Next::Byte(byte) => byte as i32,
                Next::Eof() => return output.into_boxed_slice(),
            };
            let high_byte = match reader.next() {
                Next::Byte(byte) => byte as i32,
                Next::Eof() => return output.into_boxed_slice(),
            };
            offset = ((high_byte & 0xff) << 8) | (r3 & 0xff);
            if offset == 0 {
                return output.into_boxed_slice();
            }
            r3 &= 0x00000007;
            r5 = (offset >> 3) | -8192i32; // 0xffffe000
            if r3 == 0 {
                r3 = match reader.next() {
                    Next::Byte(byte) => byte as i32,
                    Next::Eof() => return output.into_boxed_slice(),
                };
                r3 = (r3 & 0xff) + 1;
            } else {
                r3 += 2;
            }
        } else {
            r3 = 0;
            for _ in 0..2 {
                bitpos -= 1;
                if bitpos == 0 {
                    current_byte = match reader.next() {
                        Next::Byte(byte) => byte,
                        Next::Eof() => return output.into_boxed_slice(),
                    };
                    bitpos = 8;
                }
                flag = (current_byte & 1) == 1;
                current_byte >>= 1;
                offset = r3 << 1;
                r3 = offset | (flag as i32);
            }
            offset = match reader.next() {
                Next::Byte(byte) => byte as i32,
                Next::Eof() => return output.into_boxed_slice(),
            };
            r3 += 2;
            r5 = offset | -256i32; // 0xffffff00
        }
        if r3 == 0 {
            continue;
        }
        for _ in 0..r3 {
            let index = output.len() as i32 + r5;
            output.push(output[index as usize]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestData<'a> {
        uncompressed: &'a [u8],
        compressed: &'a [u8],
    }

    static TEST_DATA: &[TestData] = &[
        TestData {
            uncompressed: "Hello, world!\0".as_bytes(),
            compressed: &[
                0xff, 0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x2c, 0x20, 0x77, 0xbf, 0x6f, 0x72, 0x6c, 0x64,
                0x21, 0x00, 0x00, 0x00, 0x00,
            ],
        },
        TestData {
            uncompressed: "I am Sam

Sam I am

That Sam-I-am!
That Sam-I-am!
I do not like
that Sam-I-am!

Do you like green eggs and ham?

I do not like them, Sam-I-am.
I do not like green eggs and ham."
                .as_bytes(),
            compressed: &[
                0xff, 0x49, 0x20, 0x61, 0x6d, 0x20, 0x53, 0x61, 0x6d, 0xe3, 0x0a, 0x0a, 0xfb, 0x20,
                0x49, 0xf8, 0xf2, 0x0a, 0x0a, 0x54, 0x68, 0xd3, 0x61, 0x74, 0xec, 0x2d, 0x49, 0xef,
                0x2d, 0x61, 0x6d, 0x21, 0x88, 0xff, 0x0d, 0x21, 0x0a, 0xff, 0x49, 0x20, 0x64, 0x6f,
                0x20, 0x6e, 0x6f, 0x74, 0x7f, 0x20, 0x6c, 0x69, 0x6b, 0x65, 0x0a, 0x74, 0xff, 0x18,
                0xff, 0x0d, 0x0a, 0x44, 0x6f, 0x20, 0x79, 0x6f, 0x75, 0xfc, 0xe4, 0x20, 0x67, 0x72,
                0x65, 0xff, 0x65, 0x6e, 0x20, 0x65, 0x67, 0x67, 0x73, 0x20, 0xff, 0x61, 0x6e, 0x64,
                0x20, 0x68, 0x61, 0x6d, 0x3f, 0xfd, 0x0a, 0x08, 0xfe, 0x0d, 0x20, 0x74, 0x68, 0x65,
                0x6d, 0xad, 0x2c, 0x07, 0xfe, 0x2e, 0x10, 0xff, 0x0e, 0xf8, 0xfd, 0x11, 0x05, 0x2e,
                0x00, 0x00,
            ],
        },
        TestData {
            uncompressed: &[],
            compressed: &[0x02, 0x00, 0x00],
        },
        TestData {
            uncompressed: b"a",
            compressed: &[0x05, 0x61, 0x00, 0x00],
        },
        TestData {
            uncompressed: b"aa",
            compressed: &[0x0b, 0x61, 0x61, 0x00, 0x00],
        },
        TestData {
            uncompressed: b"aaa",
            compressed: &[0x17, 0x61, 0x61, 0x61, 0x00, 0x00],
        },
        TestData {
            uncompressed: b"aaaa",
            compressed: &[0x2f, 0x61, 0x61, 0x61, 0x61, 0x00, 0x00],
        },
        TestData {
            uncompressed: b"aaaaa",
            compressed: &[0x5f, 0x61, 0x61, 0x61, 0x61, 0x61, 0x00, 0x00],
        },
        TestData {
            uncompressed: b"aaaaaa",
            compressed: &[0xbf, 0x61, 0x61, 0x61, 0x61, 0x61, 0x61, 0x00, 0x00, 0x00],
        },
        TestData {
            uncompressed: b"aaaaaaa",
            compressed: &[0x8f, 0x61, 0x61, 0x61, 0x61, 0xfd, 0x02, 0x00, 0x00],
        },
        TestData {
            uncompressed: b"aaaaaaaa",
            compressed: &[0x8f, 0x61, 0x61, 0x61, 0x61, 0xfd, 0x05, 0x61, 0x00, 0x00],
        },
        TestData {
            uncompressed: b"aaaaaaaaa",
            compressed: &[
                0x8f, 0x61, 0x61, 0x61, 0x61, 0xfd, 0x0b, 0x61, 0x61, 0x00, 0x00,
            ],
        },
        TestData {
            uncompressed: b"aaaaaaaaaa",
            compressed: &[0x8f, 0x61, 0x61, 0x61, 0x61, 0xfd, 0x28, 0xfd, 0x00, 0x00],
        },
        TestData {
            uncompressed: b"aaaaaaaaaaa",
            compressed: &[0x8f, 0x61, 0x61, 0x61, 0x61, 0xfd, 0x24, 0xfb, 0x00, 0x00],
        },
        TestData {
            uncompressed: b"aaaaaaaaaaaa",
            compressed: &[0x8f, 0x61, 0x61, 0x61, 0x61, 0xfd, 0x2c, 0xfa, 0x00, 0x00],
        },
        TestData {
            uncompressed: b"aaaaaaaaaaaaa",
            compressed: &[
                0x8f, 0x61, 0x61, 0x61, 0x61, 0xfd, 0x5c, 0xfa, 0x61, 0x00, 0x00,
            ],
        },
        TestData {
            uncompressed: b"aaaaaaaaaaaaaa",
            compressed: &[
                0x8f, 0x61, 0x61, 0x61, 0x61, 0xfd, 0xbc, 0xfa, 0x61, 0x61, 0x00, 0x00, 0x00,
            ],
        },
        TestData {
            uncompressed: b"aaaaaaaaaaaaaaa",
            compressed: &[
                0x8f, 0x61, 0x61, 0x61, 0x61, 0xfd, 0x8c, 0xfa, 0xfd, 0x02, 0x00, 0x00,
            ],
        },
        TestData {
            uncompressed: b"aaaaaaaaaaaaaaaa",
            compressed: &[
                0x8f, 0x61, 0x61, 0x61, 0x61, 0xfd, 0x4c, 0xfa, 0xfb, 0x02, 0x00, 0x00,
            ],
        },
        TestData {
            uncompressed: b"aaaaaaaaaaaaaaaaa",
            compressed: &[
                0x8f, 0x61, 0x61, 0x61, 0x61, 0xfd, 0xcc, 0xfa, 0xfa, 0x02, 0x00, 0x00,
            ],
        },
        TestData {
            uncompressed: &[0xff],
            compressed: &[0x05, 0xff, 0x00, 0x00],
        },
        TestData {
            uncompressed: &[0xff, 0xff],
            compressed: &[0x0b, 0xff, 0xff, 0x00, 0x00],
        },
        TestData {
            uncompressed: &[0xff, 0xff, 0xff],
            compressed: &[0x17, 0xff, 0xff, 0xff, 0x00, 0x00],
        },
        TestData {
            uncompressed: &[0xff, 0xff, 0xff, 0xff],
            compressed: &[0x2f, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00],
        },
        TestData {
            uncompressed: &[0xff, 0xff, 0xff, 0xff, 0xff],
            compressed: &[0x5f, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00],
        },
        TestData {
            uncompressed: &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
            compressed: &[0xbf, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00],
        },
        TestData {
            uncompressed: &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
            compressed: &[0x8f, 0xff, 0xff, 0xff, 0xff, 0xfd, 0x02, 0x00, 0x00],
        },
        TestData {
            uncompressed: &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
            compressed: &[0x8f, 0xff, 0xff, 0xff, 0xff, 0xfd, 0x05, 0xff, 0x00, 0x00],
        },
        TestData {
            uncompressed: &[0x00],
            compressed: &[0x05, 0x00, 0x00, 0x00],
        },
        TestData {
            uncompressed: &[0x00, 0x00],
            compressed: &[0x0b, 0x00, 0x00, 0x00, 0x00],
        },
        TestData {
            uncompressed: &[0x00, 0x00, 0x00],
            compressed: &[0x17, 0x00, 0x00, 0x00, 0x00, 0x00],
        },
        TestData {
            uncompressed: &[0x00, 0x00, 0x00, 0x00],
            compressed: &[0x2f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        },
        TestData {
            uncompressed: &[0x00, 0x00, 0x00, 0x00, 0x00],
            compressed: &[0x5f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        },
        // when comparing these results to the fuzziqer implementation, these zero-byte run tests
        // start to get a little interesting from this point onward. at this point, the fuzziqer
        // implementation will sometimes start to perform memcmp() calls that check memory slightly
        // out of the bounds of its buffers, and when that memory also happens to contain zeros (as
        // seems to always be the case for me right now), you get things like the 6 and 7 length
        // zero-byte runs compressing identically (which is obviously a bug). this buggy behaviour
        // carries on for some larger run sizes too, probably indefinitely.
        TestData {
            uncompressed: &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            compressed: &[0xbf, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        },
        TestData {
            uncompressed: &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            compressed: &[0x8f, 0x00, 0x00, 0x00, 0x00, 0xfd, 0x02, 0x00, 0x00],
        },
        TestData {
            uncompressed: &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            compressed: &[0x8f, 0x00, 0x00, 0x00, 0x00, 0xfd, 0x05, 0x00, 0x00, 0x00],
        },
        TestData {
            uncompressed: &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            compressed: &[
                0x8f, 0x00, 0x00, 0x00, 0x00, 0xfd, 0x0b, 0x00, 0x00, 0x00, 0x00,
            ],
        },
        TestData {
            uncompressed: &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            compressed: &[0x8f, 0x00, 0x00, 0x00, 0x00, 0xfd, 0x28, 0xfd, 0x00, 0x00],
        },
        TestData {
            uncompressed: &[
                0x04, 0x00, 0x02, 0x01, 0x05, 0x04, 0x08, 0x00, 0x04, 0x02, 0x07, 0x0d, 0x0c, 0x11,
                0x02, 0x00, 0x03, 0x04, 0x04, 0x04, 0x03, 0x02, 0x09, 0x02, 0x03, 0x01, 0x0b, 0x0a,
                0x0d, 0x0e, 0x04, 0x03, 0x03, 0x04, 0x02, 0x00, 0x07, 0x00, 0x08, 0x00, 0x03, 0x03,
                0x0b, 0x0a, 0x0b, 0x10, 0x03, 0x03, 0x04, 0x02, 0x06, 0x04, 0x07, 0x03, 0x07, 0x04,
                0x01, 0x03, 0x0a, 0x0c, 0x0c, 0x0f, 0x02, 0x04, 0x01, 0x04, 0x04, 0x02, 0x07, 0x02,
                0x09, 0x04, 0x02, 0x03, 0x09, 0x0b, 0x0f, 0x0d, 0x02, 0x03, 0x04, 0x01, 0x02, 0x03,
                0x04, 0x02, 0x05, 0x02, 0x00, 0x00, 0x0a, 0x0b, 0x0d, 0x0f, 0x03, 0x00, 0x01, 0x02,
                0x02, 0x02, 0x07, 0x04, 0x09, 0x02, 0x00, 0x03, 0x08, 0x0a, 0x0c, 0x11, 0x04, 0x00,
                0x00, 0x04, 0x03, 0x04, 0x06, 0x01, 0x06, 0x01, 0x03, 0x01, 0x07, 0x09, 0x0e, 0x10,
                0x02, 0x01, 0x03, 0x04, 0x03, 0x02, 0x04, 0x00, 0x06, 0x01, 0x00, 0x03, 0x09, 0x0a,
                0x0d, 0x10, 0x02, 0x04, 0x03, 0x03, 0x05, 0x03, 0x04, 0x02, 0x09, 0x04, 0x03, 0x04,
                0x08, 0x0b, 0x0b, 0x0d, 0x00, 0x03, 0x00, 0x01, 0x04, 0x01, 0x06, 0x04, 0x09, 0x04,
                0x04, 0x03, 0x07, 0x0a, 0x0c, 0x0f, 0x02, 0x01, 0x02, 0x03, 0x02, 0x03, 0x05, 0x01,
                0x09, 0x00, 0x01, 0x02, 0x0b, 0x0c, 0x0e, 0x0d, 0x03, 0x00, 0x03, 0x00, 0x03, 0x02,
                0x04, 0x02, 0x06, 0x00, 0x00, 0x01, 0x0a, 0x0c, 0x0c, 0x0e, 0x02, 0x03, 0x01, 0x02,
                0x06, 0x03, 0x03, 0x00, 0x05, 0x03, 0x03, 0x02, 0x08, 0x0c, 0x0f, 0x0e, 0x03, 0x02,
                0x02, 0x01, 0x06, 0x03, 0x03, 0x02, 0x06, 0x02, 0x04, 0x04, 0x07, 0x0b, 0x0b, 0x0f,
                0x00, 0x01, 0x01, 0x01, 0x06, 0x04, 0x05, 0x02, 0x07, 0x02, 0x04, 0x04, 0x09, 0x0c,
                0x0d, 0x0d, 0x00, 0x04, 0x03, 0x02, 0x02, 0x00, 0x07, 0x01, 0x07, 0x00, 0x00, 0x04,
                0x09, 0x0c, 0x0f, 0x10, 0x04, 0x00, 0x01, 0x01, 0x06, 0x03, 0x03, 0x04, 0x07, 0x04,
                0x03, 0x04, 0x09, 0x09, 0x0c, 0x11, 0x02, 0x01, 0x03, 0x04, 0x03, 0x03, 0x03, 0x03,
                0x08, 0x02, 0x03, 0x01, 0x07, 0x0b, 0x0c, 0x0f, 0x04, 0x04, 0x00, 0x01, 0x02, 0x00,
                0x03, 0x02, 0x09, 0x00, 0x04, 0x03, 0x09, 0x09, 0x0f, 0x0e, 0x02, 0x03, 0x00, 0x00,
                0x03, 0x02, 0x04, 0x01, 0x05, 0x01, 0x04, 0x02, 0x07, 0x0b, 0x0f, 0x11, 0x02, 0x04,
                0x02, 0x02, 0x03, 0x04, 0x07, 0x00, 0x09, 0x03, 0x00, 0x04, 0x08, 0x09, 0x0b, 0x0d,
                0x03, 0x01, 0x00, 0x01, 0x02, 0x01, 0x05, 0x00, 0x07, 0x04, 0x03, 0x02, 0x08, 0x0d,
                0x0f, 0x10, 0x01, 0x03, 0x00, 0x02, 0x05, 0x02, 0x03, 0x02, 0x07, 0x00, 0x03, 0x03,
                0x09, 0x0d, 0x0b, 0x0f, 0x02, 0x01, 0x03, 0x02, 0x06, 0x03, 0x03, 0x04, 0x07, 0x00,
                0x03, 0x03, 0x0b, 0x0b, 0x0f, 0x0f, 0x03, 0x01, 0x00, 0x01, 0x05, 0x02, 0x03, 0x03,
                0x07, 0x04, 0x03, 0x02, 0x0a, 0x0d, 0x0f, 0x0d, 0x02, 0x00, 0x04, 0x01, 0x05, 0x04,
                0x05, 0x02, 0x06, 0x01, 0x00, 0x03, 0x07, 0x0a, 0x0b, 0x10, 0x03, 0x02, 0x04, 0x03,
                0x06, 0x00, 0x04, 0x04, 0x06, 0x00, 0x01, 0x04, 0x08, 0x09, 0x0c, 0x10, 0x00, 0x02,
                0x01, 0x00, 0x04, 0x04, 0x05, 0x00, 0x07, 0x00, 0x03, 0x02, 0x08, 0x0d, 0x0e, 0x0e,
                0x01, 0x04, 0x00, 0x01, 0x03, 0x01, 0x05, 0x02, 0x08, 0x03, 0x01, 0x04, 0x07, 0x0d,
                0x0f, 0x10, 0x02, 0x01, 0x00, 0x01, 0x04, 0x03, 0x04, 0x04, 0x05, 0x00, 0x03, 0x01,
                0x0b, 0x0c, 0x0b, 0x0f, 0x03, 0x00, 0x00, 0x04, 0x05, 0x02, 0x05, 0x02, 0x05, 0x00,
                0x03, 0x03, 0x09, 0x09, 0x0e, 0x11, 0x03, 0x03, 0x00, 0x00, 0x03, 0x01, 0x04, 0x01,
                0x08, 0x01, 0x00, 0x02, 0x07, 0x09, 0x0d, 0x10, 0x00, 0x02, 0x04, 0x00, 0x02, 0x01,
                0x05, 0x02, 0x09, 0x03, 0x00, 0x01, 0x0a, 0x0c, 0x0d, 0x0e, 0x02, 0x02, 0x03, 0x00,
                0x02, 0x04, 0x05, 0x01, 0x07, 0x04, 0x03, 0x02, 0x08, 0x09, 0x0b, 0x10, 0x03, 0x00,
                0x03, 0x00, 0x05, 0x03, 0x05, 0x04, 0x06, 0x03, 0x02, 0x01, 0x0a, 0x0d, 0x0f, 0x0d,
                0x01, 0x02, 0x03, 0x04, 0x05, 0x02, 0x03, 0x02, 0x06, 0x00, 0x00, 0x02, 0x0a, 0x0b,
                0x0b, 0x10, 0x04, 0x00, 0x03, 0x03, 0x05, 0x02, 0x07, 0x01, 0x05, 0x02, 0x04, 0x01,
                0x08, 0x0c, 0x0e, 0x0d, 0x02, 0x01, 0x01, 0x02, 0x05, 0x01, 0x03, 0x01, 0x08, 0x00,
                0x00, 0x03, 0x0b, 0x0b, 0x0c, 0x11, 0x03, 0x01, 0x02, 0x01, 0x06, 0x01, 0x03, 0x01,
                0x05, 0x04, 0x02, 0x02, 0x0a, 0x0c, 0x0d, 0x0f, 0x04, 0x03, 0x02, 0x00, 0x03, 0x02,
                0x04, 0x01, 0x09, 0x02, 0x00, 0x03, 0x0b, 0x0c, 0x0d, 0x0f, 0x02, 0x01, 0x01, 0x03,
                0x02, 0x01, 0x07, 0x00, 0x07, 0x04, 0x02, 0x02, 0x09, 0x0a, 0x0b, 0x10, 0x01, 0x02,
                0x03, 0x02, 0x03, 0x00, 0x07, 0x02, 0x09, 0x01, 0x00, 0x00, 0x0b, 0x09, 0x0e, 0x0e,
                0x01, 0x01, 0x04, 0x03, 0x06, 0x01, 0x07, 0x01, 0x07, 0x03, 0x04, 0x01, 0x09, 0x0a,
                0x0f, 0x10, 0x03, 0x03, 0x01, 0x01, 0x02, 0x02, 0x06, 0x01, 0x08, 0x00, 0x01, 0x04,
                0x07, 0x0a, 0x0e, 0x11, 0x02, 0x04, 0x02, 0x01, 0x02, 0x03, 0x03, 0x02, 0x07, 0x04,
                0x03, 0x01, 0x07, 0x09, 0x0f, 0x0d, 0x03, 0x02, 0x01, 0x00, 0x06, 0x01, 0x04, 0x04,
                0x06, 0x02, 0x01, 0x04, 0x08, 0x0d, 0x0e, 0x10, 0x03, 0x00, 0x02, 0x01, 0x03, 0x02,
                0x03, 0x00, 0x08, 0x04, 0x01, 0x03, 0x08, 0x09, 0x0b, 0x11, 0x03, 0x03, 0x01, 0x04,
                0x06, 0x04, 0x04, 0x02, 0x08, 0x04, 0x01, 0x04, 0x0a, 0x0d, 0x0e, 0x10, 0x02, 0x02,
                0x01, 0x03, 0x06, 0x02, 0x03, 0x04, 0x08, 0x01, 0x02, 0x04, 0x08, 0x0b, 0x0e, 0x0e,
                0x00, 0x01, 0x03, 0x01, 0x02, 0x04, 0x06, 0x00, 0x05, 0x00, 0x00, 0x00, 0x08, 0x09,
                0x0e, 0x10, 0x02, 0x00, 0x03, 0x03, 0x06, 0x03, 0x07, 0x02, 0x09, 0x01, 0x01, 0x03,
                0x07, 0x0a, 0x0f, 0x0f, 0x02, 0x04, 0x03, 0x04, 0x05, 0x03, 0x07, 0x00, 0x08, 0x01,
                0x00, 0x00, 0x0a, 0x0d, 0x0b, 0x0f, 0x01, 0x04, 0x00, 0x00, 0x06, 0x04, 0x07, 0x01,
                0x07, 0x00, 0x04, 0x04, 0x08, 0x09, 0x0c, 0x0d, 0x00, 0x01, 0x04, 0x00, 0x02, 0x00,
                0x04, 0x00, 0x09, 0x01, 0x02, 0x02, 0x09, 0x0c, 0x0b, 0x0d, 0x04, 0x02, 0x02, 0x03,
                0x06, 0x01, 0x07, 0x01, 0x06, 0x00, 0x01, 0x04, 0x08, 0x0d, 0x0f, 0x10, 0x03, 0x00,
                0x03, 0x03, 0x03, 0x01, 0x03, 0x00, 0x05, 0x03, 0x02, 0x02, 0x0a, 0x0b, 0x0b, 0x0f,
                0x00, 0x02, 0x02, 0x04, 0x03, 0x04, 0x05, 0x02, 0x09, 0x00, 0x04, 0x02, 0x09, 0x0c,
                0x0b, 0x0d, 0x04, 0x01, 0x00, 0x02, 0x04, 0x00, 0x05, 0x02, 0x05, 0x01, 0x02, 0x03,
                0x08, 0x0b, 0x0d, 0x10, 0x01, 0x00, 0x04, 0x02, 0x03, 0x01, 0x05, 0x02, 0x09, 0x01,
                0x00, 0x01, 0x08, 0x0b, 0x0c, 0x0d, 0x03, 0x03, 0x02, 0x03, 0x05, 0x01, 0x05, 0x04,
                0x05, 0x04, 0x04, 0x01, 0x0a, 0x0b, 0x0f, 0x0d, 0x04, 0x03, 0x02, 0x00, 0x03, 0x01,
                0x05, 0x02, 0x07, 0x04, 0x03, 0x04, 0x09, 0x0a, 0x0c, 0x0f, 0x04, 0x01, 0x00, 0x00,
                0x04, 0x03, 0x04, 0x04, 0x09, 0x00, 0x00, 0x03, 0x0b, 0x0a, 0x0b, 0x10, 0x01, 0x04,
                0x00, 0x00, 0x03, 0x03, 0x05, 0x00, 0x09, 0x01, 0x01, 0x01, 0x0b, 0x0c, 0x0f, 0x11,
                0x01, 0x04,
            ],
            compressed: &[
                0xff, 0x04, 0x00, 0x02, 0x01, 0x05, 0x04, 0x08, 0x00, 0xff, 0x04, 0x02, 0x07, 0x0d,
                0x0c, 0x11, 0x02, 0x00, 0xff, 0x03, 0x04, 0x04, 0x04, 0x03, 0x02, 0x09, 0x02, 0xff,
                0x03, 0x01, 0x0b, 0x0a, 0x0d, 0x0e, 0x04, 0x03, 0xff, 0x03, 0x04, 0x02, 0x00, 0x07,
                0x00, 0x08, 0x00, 0x3f, 0x03, 0x03, 0x0b, 0x0a, 0x0b, 0x10, 0xfd, 0xf1, 0x06, 0x04,
                0x07, 0x03, 0x07, 0x04, 0xff, 0x01, 0x03, 0x0a, 0x0c, 0x0c, 0x0f, 0x02, 0x04, 0xe3,
                0x01, 0x04, 0xc6, 0x02, 0x09, 0xff, 0x04, 0x02, 0x03, 0x09, 0x0b, 0x0f, 0x0d, 0x02,
                0xc7, 0x03, 0x04, 0x01, 0xfc, 0x02, 0xff, 0x05, 0x02, 0x00, 0x00, 0x0a, 0x0b, 0x0d,
                0x0f, 0xff, 0x03, 0x00, 0x01, 0x02, 0x02, 0x02, 0x07, 0x04, 0xf1, 0x09, 0xa7, 0x08,
                0x0a, 0x0c, 0xff, 0x11, 0x04, 0x00, 0x00, 0x04, 0x03, 0x04, 0x06, 0xff, 0x01, 0x06,
                0x01, 0x03, 0x01, 0x07, 0x09, 0x0e, 0x8f, 0x10, 0x02, 0x01, 0x03, 0x92, 0xff, 0x04,
                0x00, 0x06, 0x01, 0x00, 0x03, 0x09, 0x0a, 0xc7, 0x0d, 0x10, 0x02, 0x8f, 0x05, 0x18,
                0xc0, 0x09, 0x3f, 0xda, 0x08, 0x0b, 0x0b, 0x0d, 0x00, 0x7e, 0xbf, 0x04, 0x01, 0x06,
                0x04, 0x09, 0x1c, 0x6b, 0x07, 0x0a, 0xf1, 0x90, 0xa2, 0x02, 0x03, 0x05, 0xe3, 0x01,
                0x09, 0xa8, 0x0b, 0x0c, 0x47, 0x0e, 0x0d, 0x03, 0xdf, 0xfc, 0xc0, 0x02, 0x06, 0x00,
                0x00, 0x01, 0x18, 0x70, 0x0e, 0xff, 0x49, 0x02, 0x06, 0x03, 0x03, 0x00, 0x05, 0x03,
                0xff, 0x03, 0x02, 0x08, 0x0c, 0x0f, 0x0e, 0x03, 0x02, 0xe3, 0x02, 0x01, 0xf0, 0x02,
                0x06, 0xff, 0x02, 0x04, 0x04, 0x07, 0x0b, 0x0b, 0x0f, 0x00, 0x63, 0x01, 0x01, 0xb2,
                0x05, 0xfc, 0x4e, 0x04, 0x04, 0x09, 0x0c, 0x0d, 0x31, 0x0d, 0x72, 0x02, 0x8e, 0x20,
                0x01, 0x07, 0x68, 0x1f, 0x09, 0x0c, 0x0f, 0x10, 0x04, 0x71, 0xdf, 0xd0, 0x04, 0x07,
                0x5c, 0x80, 0x09, 0x09, 0x81, 0xf7, 0x7a, 0x60, 0x03, 0x03, 0x03, 0x08, 0xfc, 0xa7,
                0x07, 0x0b, 0x0c, 0x0f, 0x04, 0x88, 0xdf, 0x35, 0xe3, 0x02, 0x09, 0xc7, 0x09, 0x09,
                0x31, 0x0f, 0x90, 0x00, 0xdd, 0x80, 0x01, 0x05, 0x01, 0xd1, 0xf7, 0x0b, 0x23, 0x0f,
                0x11, 0x75, 0xfe, 0x01, 0x07, 0x00, 0x09, 0x03, 0x00, 0x04, 0x3f, 0x08, 0x09, 0x0b,
                0x0d, 0x03, 0x01, 0x1e, 0xd0, 0x01, 0x05, 0x00, 0xff, 0xb0, 0x02, 0x08, 0x0d, 0x0f,
                0x10, 0x01, 0x03, 0xbd, 0x00, 0x21, 0xf7, 0x03, 0x02, 0x07, 0x81, 0xf5, 0x47, 0x09,
                0x0d, 0x0b, 0x30, 0x24, 0x64, 0x90, 0x4f, 0x02, 0xf5, 0x0b, 0x0f, 0x0f, 0xd0, 0xe8,
                0xe0, 0x01, 0xf5, 0x03, 0x02, 0x1b, 0x0a, 0x0d, 0x81, 0xf5, 0x00, 0xd1, 0xa4, 0x50,
                0x02, 0xf7, 0x07, 0xfa, 0x02, 0xf4, 0xf9, 0xf6, 0x06, 0x00, 0x04, 0x04, 0x7d, 0x06,
                0x49, 0xf7, 0x08, 0x09, 0x0c, 0x10, 0x11, 0x19, 0xf2, 0xf2, 0xf1, 0xa0, 0x7a, 0x08,
                0x0d, 0x0e, 0x63, 0x0e, 0x01, 0x60, 0x03, 0xfc, 0xbe, 0x08, 0x03, 0x01, 0x04, 0x07,
                0x88, 0x90, 0xe1, 0x11, 0x01, 0x35, 0xdd, 0xde, 0x81, 0xf1, 0x0c, 0x0b, 0x81, 0xf3,
                0x00, 0x38, 0xb2, 0x05, 0x02, 0xe2, 0xf0, 0x40, 0x0e, 0x11, 0xe2, 0xa9, 0xf6, 0xe6,
                0x04, 0x01, 0x7f, 0x08, 0x01, 0x00, 0x02, 0x07, 0x09, 0x0d, 0x6c, 0xb0, 0x04, 0x82,
                0xef, 0x02, 0x74, 0x40, 0x81, 0xf5, 0x0d, 0x0e, 0xc4, 0x32, 0xed, 0x05, 0x39, 0x01,
                0x40, 0x09, 0xfa, 0x80, 0x81, 0xf4, 0x05, 0x03, 0x05, 0x04, 0x4f, 0x06, 0x03, 0x02,
                0x01, 0x60, 0x92, 0x9a, 0xf0, 0x30, 0x01, 0xf4, 0xaf, 0x02, 0x0a, 0x0b, 0x0b, 0x01,
                0xf6, 0x01, 0xf2, 0x23, 0x02, 0x07, 0xbe, 0xfa, 0xac, 0x01, 0xf3, 0x02, 0x01, 0x01,
                0x02, 0x31, 0x05, 0x6e, 0x08, 0xbe, 0x99, 0x0b, 0x0b, 0x0c, 0x11, 0x09, 0xf3, 0x16,
                0x23, 0xf0, 0x05, 0xc1, 0xf6, 0xa3, 0xa0, 0x0f, 0xa9, 0x03, 0xf6, 0x46, 0x02, 0xef,
                0x0b, 0xf0, 0xc4, 0xd0, 0xa6, 0x07, 0x1e, 0x81, 0xf6, 0x02, 0x02, 0x09, 0xed, 0x10,
                0x8b, 0xf0, 0x00, 0x01, 0xed, 0x01, 0x00, 0x47, 0x00, 0x0b, 0x09, 0x20, 0x5c, 0x32,
                0x06, 0x01, 0x81, 0xf2, 0x2f, 0xc1, 0xec, 0x09, 0x0a, 0x0f, 0x81, 0xeb, 0x1a, 0x9f,
                0x11, 0xf7, 0x08, 0x2f, 0x19, 0x07, 0x0a, 0x0e, 0x02, 0xf4, 0xda, 0xcd, 0x01, 0xf5,
                0x04, 0x02, 0xed, 0x0f, 0x51, 0x0d, 0xb3, 0x21, 0xed, 0x3b, 0x81, 0xf6, 0x02, 0x81,
                0xf6, 0x0d, 0x0e, 0x7a, 0x40, 0x72, 0xf4, 0x03, 0x00, 0x08, 0xd1, 0x01, 0xea, 0x30,
                0x01, 0xf8, 0x01, 0x8b, 0x04, 0x06, 0xf1, 0xe9, 0xf0, 0xed, 0x04, 0x89, 0xe8, 0x10,
                0x89, 0xee, 0x03, 0x06, 0x78, 0x2c, 0x08, 0x01, 0x02, 0xb7, 0x01, 0xec, 0x0e, 0x0e,
                0x92, 0xf5, 0x02, 0x91, 0xf4, 0x5f, 0x05, 0x00, 0x00, 0x00, 0x08, 0x82, 0xea, 0x9c,
                0x20, 0x06, 0x03, 0x5a, 0x70, 0x01, 0x81, 0xf3, 0x0f, 0x01, 0xe8, 0x8b, 0x01, 0xf8,
                0x03, 0x01, 0xe7, 0x60, 0xb5, 0x0a, 0x81, 0xf1, 0x01, 0xf4, 0x00, 0x01, 0xe7, 0xd8,
                0x3e, 0x04, 0x02, 0xf3, 0x0d, 0xd8, 0x69, 0x00, 0xe1, 0xf1, 0x00, 0xf1, 0x09, 0x5a,
                0x09, 0x0c, 0x0b, 0x25, 0x0d, 0x0a, 0xef, 0x40, 0x15, 0x03, 0xf2, 0x01, 0xf3, 0x81,
                0xf5, 0xd5, 0x7d, 0x62, 0xf5, 0x81, 0xf7, 0x02, 0xeb, 0x02, 0xac, 0xae, 0x02, 0xed,
                0x84, 0xfe, 0x2e, 0x41, 0xf3, 0x04, 0x00, 0x81, 0xf2, 0xbe, 0x2a, 0x08, 0x0b, 0x0d,
                0x10, 0x91, 0xf0, 0xa2, 0xb1, 0xeb, 0xe0, 0x41, 0xf1, 0x45, 0x08, 0x79, 0xf6, 0x15,
                0x7d, 0x91, 0xe7, 0xf1, 0xee, 0x04, 0x04, 0x01, 0x0a, 0x45, 0x01, 0xe4, 0x83, 0xf5,
                0xe0, 0x45, 0x03, 0xea, 0x81, 0xe6, 0xc0, 0x57, 0x7a, 0xe4, 0x04, 0x09, 0x02, 0xf4,
                0x82, 0xf5, 0x14, 0x60, 0xf1, 0xf2, 0xfb, 0x70, 0x01, 0x81, 0xef, 0x0f, 0x11, 0x01,
                0x04, 0x02, 0x00, 0x00,
            ],
        },
    ];

    #[test]
    pub fn compresses_things() {
        for (index, test) in TEST_DATA.iter().enumerate() {
            println!("\ntest #{}", index);
            println!("  prs_compress({:02x?})", test.uncompressed);
            assert_eq!(*test.compressed, *prs_compress(&test.uncompressed));
            println!("  prs_decompress({:02x?})", test.compressed);
            assert_eq!(*test.uncompressed, *prs_decompress(&test.compressed));
        }
    }

    #[test]
    pub fn testit() {}
}
