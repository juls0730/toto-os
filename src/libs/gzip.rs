use alloc::vec::Vec;

use crate::libs::sync::Mutex;

#[derive(Debug)]
#[repr(u8)]
enum ZlibCompressionLevel {
    Fastest = 0,
    Fast,
    Default,
    Best,
}

impl From<u8> for ZlibCompressionLevel {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Fastest,
            1 => Self::Fast,
            2 => Self::Default,
            3 => Self::Best,
            _ => panic!("Unexpected compression level {value}"),
        }
    }
}

#[derive(Debug)]
#[repr(u8)]
pub enum CompressionErrors {
    NotDeflate = 0,
    UnsupportedWindowSize,
    FCheckFailed,
    UnsupportedDictionary,
    FailedChecksum,
    FailedCompression,
}

// RFC 1950: "ZLIB Compressed Data Format Specification"
// RFC 1951: "DEFLATE Compressed Data Format Specification"
pub fn uncompress_data(bytes: &[u8]) -> Result<Vec<u8>, ()> {
    assert!(bytes.len() > 2);

    // Compression Method and flags
    let cmf = bytes[0];

    if (cmf & 0x0F) != 0x08 {
        return Err(());
        // return Err(CompressionErrors::NotDeflate);
    }

    let window_log2 = cmf >> 4 & 0x0F;

    if window_log2 > 0x07 {
        return Err(());
        // return Err(CompressionErrors::UnsupportedWindowSize);
    }

    let flags = bytes[1];
    if (cmf as u32 * 256 + flags as u32) % 31 != 0 {
        return Err(());
        // return Err(CompressionErrors::FCheckFailed);
    }

    let present_dictionary = flags >> 5 & 0x01 != 0;
    let _compression_level: ZlibCompressionLevel = (flags >> 6 & 0x03).into();

    if present_dictionary {
        // cry
        return Err(());
        // return Err(CompressionErrors::UnsupportedDictionary);
    }

    let mut inflate_context = InflateContext::new(&bytes[2..bytes.len() - 4]);

    let data = inflate_context.decompress();

    if data.is_err() {
        return Err(());
        // return Err(CompressionErrors::FailedCompression);
    }

    let data = data.unwrap();

    // last 4 bytes of zlib data
    let checksum = u32::from_le_bytes(bytes[bytes.len() - 4..].try_into().unwrap());

    if adler32(&data) != checksum {
        return Err(());
        // return Err(CompressionErrors::FailedChecksum);
    }

    return Ok(data);
}

fn adler32(bytes: &[u8]) -> u32 {
    let mut a = 1_u32;
    let mut b = 0_u32;

    for &byte in bytes {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }

    return u32::from_be((b << 16) | a);
}

#[derive(Debug)]
struct Huff {
    counts: [u16; 16],
    symbols: [u16; 288],
}

static FIXED_LENGTHS: Mutex<Huff> = Mutex::new(Huff {
    counts: [0_u16; 16],
    symbols: [0_u16; 288],
});
static FIXED_DISTS: Mutex<Huff> = Mutex::new(Huff {
    counts: [0_u16; 16],
    symbols: [0_u16; 288],
});

struct HuffRing {
    pointer: usize,
    data: Vec<u8>,
}

impl HuffRing {
    fn new() -> Self {
        let mut data = Vec::with_capacity(32 * 1024);
        unsafe {
            data.set_len(32 * 1024);
        };

        return Self { pointer: 0, data };
    }
}

struct InflateContext {
    input_buf: Vec<u8>,
    bit_index: usize,
    output_buf: alloc::vec::Vec<u8>,
    ring: HuffRing,
}

impl InflateContext {
    fn new(bytes: &[u8]) -> Self {
        return Self {
            input_buf: bytes.to_vec(),
            bit_index: 0,
            output_buf: Vec::new(),
            ring: HuffRing::new(),
        };
    }

    // read from right-to-left NOT, and I cannot stress this enough, left-to-right
    // probably because it's way simpler computationally to get the right-most bit,
    // but still, wasted weeks on this because I read it from left-to-right ;~;
    pub fn get_bit(&mut self) -> bool {
        if self.bit_index == 8 {
            self.input_buf.remove(0);
            if self.input_buf.is_empty() {
                panic!("Not enough data! {:X?}", self.output_buf);
            }

            self.bit_index = 0;
        }

        let byte = self.input_buf[0] & (1 << self.bit_index) != 0;
        self.bit_index += 1;

        return byte;
    }

    pub fn get_bits(&mut self, num_bits: usize) -> u32 {
        let mut byte = 0_u32;

        for bit in 0..num_bits {
            byte |= (self.get_bit() as u32) << bit;
        }

        return byte;
    }

    fn get_bits_base(&mut self, num: usize, base: usize) -> u32 {
        return (base + if num != 0 { self.get_bits(num) } else { 0 } as usize) as u32;
    }

    pub fn decompress(&mut self) -> Result<Vec<u8>, ()> {
        build_fixed();

        loop {
            let is_final = self.get_bit();
            let block_type = self.get_bits(2);

            match block_type {
                0x00 => {
                    self.uncompressed()?;
                }
                0x01 => {
                    self.inflate(&mut FIXED_LENGTHS.lock(), &mut FIXED_DISTS.lock())?;
                }
                0x02 => {
                    self.decode_huffman()?;
                }
                _ => {
                    return Err(());
                }
            }

            if is_final {
                break;
            }
        }

        return Ok(self.output_buf.clone());
    }

    fn decode(&mut self, huff: &mut Huff) -> u32 {
        let mut base: i32 = 0;
        let mut offs: i32 = 0;

        let mut i = 1;
        loop {
            offs = 2 * offs + self.get_bit() as i32;

            assert!(i <= 15);

            if offs < huff.counts[i] as i32 {
                break;
            }

            base += huff.counts[i] as i32;
            offs -= huff.counts[i] as i32;
            i += 1;
        }

        assert!(base + offs >= 0 && base + offs < 288);

        return huff.symbols[(base + offs) as usize] as u32;
    }

    fn emit(&mut self, byte: u8) {
        if self.ring.pointer == 32768 {
            self.ring.pointer = 0;
        }

        self.ring.data[self.ring.pointer] = byte;
        self.ring.pointer += 1;
        self.output_buf.push(byte);
    }

    fn peek(&mut self, offset: usize) -> u8 {
        let index = (self.ring.pointer).wrapping_sub(offset) % 32768;
        self.ring.data[index]
    }

    fn uncompressed(&mut self) -> Result<(), ()> {
        let len = u16::from_le(self.get_bits(16).try_into().unwrap());
        let nlen = u16::from_le(self.get_bits(16).try_into().unwrap());

        if nlen != !len {
            return Err(());
        }

        for _ in 0..len {
            // TODO: is this right?
            let byte = self.get_bits(8) as u8;
            self.emit(byte);
        }

        return Ok(());
    }

    fn inflate(&mut self, huff_len: &mut Huff, huff_dist: &mut Huff) -> Result<(), ()> {
        let length_bits = [
            0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
            127,
        ];
        let length_base = [
            3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99,
            115, 131, 163, 195, 227, 258, 0,
        ];

        let dist_bits = [
            0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12,
            12, 13, 13,
        ];
        let dist_base = [
            1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025,
            1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
        ];

        loop {
            let mut symbol = self.decode(huff_len);

            if symbol < 256 {
                self.emit(symbol as u8);
            } else {
                if symbol == 256 {
                    break;
                }

                symbol -= 257;

                let length =
                    self.get_bits_base(length_bits[symbol as usize], length_base[symbol as usize]);
                let distance = self.decode(huff_dist);
                let offset =
                    self.get_bits_base(dist_bits[distance as usize], dist_base[distance as usize]);

                for _ in 0..length {
                    let b = self.peek(offset as usize);
                    self.emit(b);
                }
            }
        }

        return Ok(());
    }

    fn decode_huffman(&mut self) -> Result<(), ()> {
        let clens = [
            16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
        ];

        let mut lengths = [0_u8; 320];

        let literals = self.get_bits_base(5, 257);
        let distances = self.get_bits_base(5, 1);
        let clengths = self.get_bits_base(4, 4);

        for i in 0..clengths {
            lengths[clens[i as usize] as usize] = self.get_bits(3) as u8;
        }

        let mut codes = Huff {
            counts: [0_u16; 16],
            symbols: [0_u16; 288],
        };
        build_huffman(&lengths, 19, &mut codes);

        let mut count = 0_u32;
        while count < literals + distances {
            let symbol = self.decode(&mut codes);

            if symbol < 16 {
                lengths[count as usize] = symbol as u8;
                count += 1;
            } else if symbol < 19 {
                let mut rep = 0_u32;
                let mut length;

                if symbol == 16 {
                    rep = lengths[count as usize - 1] as u32;
                    length = self.get_bits_base(2, 3);
                } else if symbol == 17 {
                    length = self.get_bits_base(3, 3);
                } else {
                    length = self.get_bits_base(7, 11);
                }

                while length != 0 {
                    lengths[count as usize] = rep as u8;
                    count += 1;
                    length -= 1;
                }
            } else {
                break;
            }
        }

        let mut huff_len = Huff {
            counts: [0_u16; 16],
            symbols: [0_u16; 288],
        };
        build_huffman(&lengths, literals as usize, &mut huff_len);
        let mut huff_dist = Huff {
            counts: [0_u16; 16],
            symbols: [0_u16; 288],
        };
        build_huffman(
            &lengths[literals as usize..],
            distances as usize,
            &mut huff_dist,
        );

        self.inflate(&mut huff_len, &mut huff_dist)?;

        return Ok(());
    }
}

fn build_huffman(lengths: &[u8], size: usize, out: &mut Huff) {
    let mut offsets = [0_u32; 16];
    let mut count: u32 = 0;

    assert!(size <= 288);

    for i in 0..16 {
        out.counts[i] = 0;
    }

    for i in 0..size {
        assert!(lengths[i] <= 15);

        out.counts[lengths[i] as usize] += 1;
    }

    out.counts[0] = 0;

    for i in 0..16 {
        offsets[i] = count;
        count += out.counts[i] as u32;
    }

    for i in 0..size {
        if lengths[i] != 0 {
            out.symbols[offsets[lengths[i] as usize] as usize] = i.try_into().unwrap();
            offsets[lengths[i] as usize] += 1;
        }
    }
}

fn build_fixed() {
    let mut lengths = [0_u8; 288];

    lengths[0..144].fill(8);
    lengths[144..256].fill(9);
    lengths[256..280].fill(7);
    lengths[280..288].fill(8);

    build_huffman(&lengths, 288, &mut FIXED_LENGTHS.lock());

    lengths[0..30].fill(5);

    build_huffman(&lengths, 30, &mut FIXED_DISTS.lock());
}
