use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::ops::Index;
use core::ops::{Range, RangeFrom};

const HEADER_SIZE: usize = 2;

struct Chunk<'a> {
    data: Cow<'a, [u8]>,
}

impl Chunk<'_> {
    fn header(&self) -> u16 {
        u16::from_le_bytes(self.data[0..HEADER_SIZE].try_into().unwrap())
    }

    fn len(&self) -> usize {
        self.header() as usize & 0x7FFF
    }

    fn is_compressed(&self) -> bool {
        self.header() & 0x8000 == 0
    }

    fn decompress(&mut self, decompressor: &dyn Fn(&[u8]) -> Result<Vec<u8>, ()>) {
        if self.is_compressed() {
            let decompressed_data = decompressor(&self.data[HEADER_SIZE..]).unwrap();

            let header = decompressed_data.len() as u16 | 0x8000;

            let data = [header.to_le_bytes().to_vec(), decompressed_data].concat();

            self.data = Cow::Owned(data);
        }
    }
}

impl Index<usize> for Chunk<'_> {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        &self.data[index]
    }
}

impl Index<Range<usize>> for Chunk<'_> {
    type Output = [u8];

    fn index(&self, index: Range<usize>) -> &Self::Output {
        &self.data[index]
    }
}

impl Index<RangeFrom<usize>> for Chunk<'_> {
    type Output = [u8];

    fn index(&self, index: RangeFrom<usize>) -> &Self::Output {
        &self.data[index]
    }
}

pub struct ChunkReader<'a, F> {
    chunks: Vec<Chunk<'a>>,
    decompressor: F,
}

impl<'a, F: Fn(&[u8]) -> Result<Vec<u8>, ()>> ChunkReader<'a, F> {
    pub fn new(data: &'a [u8], decompressor: F) -> Self {
        let mut chunks: Vec<Chunk<'_>> = Vec::new();

        let mut offset = 0;
        loop {
            if offset == data.len() {
                break;
            }

            let length =
                (u16::from_le_bytes(data[offset..offset + HEADER_SIZE].try_into().unwrap())
                    & 0x7FFF) as usize
                    + HEADER_SIZE;

            chunks.push(Chunk {
                data: Cow::Borrowed(&data[offset..offset + length]),
            });

            offset += length;
        }

        Self {
            chunks,
            decompressor,
        }
    }

    pub fn get_slice(&mut self, mut chunk: u64, mut offset: u16, size: usize) -> Vec<u8> {
        // handle cases where the chunks arent aligned to CHUNK_SIZE (they're compressed and are doing stupid things)
        {
            let mut chunk_idx = 0;
            let mut total_length = 0;

            while total_length != chunk {
                chunk_idx += 1;
                total_length += (self.chunks[0].len() + HEADER_SIZE) as u64;
            }

            chunk = chunk_idx;
        }

        let mut chunks_to_read = 1;
        {
            let mut available_bytes = {
                self.chunks[chunk as usize].decompress(&self.decompressor);
                self.chunks[chunk as usize][offset as usize..].len()
            };

            while available_bytes < size {
                self.chunks[chunk as usize + chunks_to_read].decompress(&self.decompressor);
                available_bytes += self.chunks[chunk as usize + chunks_to_read].len();
                chunks_to_read += 1;
            }
        }

        let mut data = Vec::new();

        for i in chunk as usize..chunk as usize + chunks_to_read {
            self.chunks[i].decompress(&self.decompressor);

            let block_start = offset as usize + HEADER_SIZE;
            let mut block_end = self.chunks[i].len() + HEADER_SIZE;

            if (block_end - block_start) > size {
                block_end = block_start + size;
            }

            data.extend(self.chunks[i][block_start..block_end].iter());

            offset = 0;
        }

        data
    }
}
