#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SquashfsCompressionType {
    Gzip = 1,
    Lzma = 2,
    Lzo = 3,
    Xz = 4,
    Lz4 = 5,
    Zstd = 6,
}

impl From<u16> for SquashfsCompressionType {
    fn from(value: u16) -> Self {
        match value {
            1 => Self::Gzip,
            2 => Self::Lzma,
            3 => Self::Lzo,
            4 => Self::Xz,
            5 => Self::Lz4,
            6 => Self::Zstd,
            _ => panic!("Unexpected Squashfs compression type!"),
        }
    }
}

#[repr(u16)]
enum SquashfsFlags {
    UncompressedInodes = 0x0001,
    UncompressedDataBlocks = 0x0002,
    Reserved = 0x0004,
    UncompressedFragments = 0x0008,
    UnusedFragments = 0x0010,
    FragmentsAlwaysPresent = 0x0020,
    DeduplicatedData = 0x0040,
    PresentNFSTable = 0x0080,
    UncompressedXattrs = 0x0100,
    NoXattrs = 0x0200,
    PresentCompressorOptions = 0x0400,
    UncompressedIDTable = 0x0800,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SquashfsFeatures {
    pub uncompressed_inodes: bool,
    pub uncompressed_data_blocks: bool,
    _reserved: bool,
    pub uncompressed_fragments: bool,
    pub unused_fragments: bool,
    pub fragments_always_present: bool,
    pub deduplicated_data: bool,
    pub nfs_table_present: bool,
    pub uncompressed_xattrs: bool,
    pub no_xattrs: bool,
    pub compressor_options_present: bool,
    pub uncompressed_id_table: bool,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct SquashfsSuperblock {
    magic: u32,                          // 0x73717368
    inode_count: u32,                    // 0x02
    mod_time: u32,                       // varies
    pub block_size: u32,                 // 0x20000
    frag_count: u32,                     // 0x01
    compressor: SquashfsCompressionType, // GZIP
    block_log: u16,                      // 0x11
    flags: u16,                          // 0xC0
    id_count: u16,                       // 0x01
    ver_major: u16,                      // 0x04
    ver_minor: u16,                      // 0x00
    pub root_inode: u64,                 //
    bytes_used: u64,                     // 0x0103
    pub id_table: u64,                   // 0x00FB
    pub xattr_table: u64,                // 0xFFFFFFFFFFFFFFFF
    pub inode_table: u64,                // 0x7B
    pub dir_table: u64,                  // 0xA4
    pub frag_table: u64,                 // 0xD5
    pub export_table: u64,               // 0xED
}

impl SquashfsSuperblock {
    pub fn new(bytes: &[u8]) -> Result<Self, ()> {
        let superblock = Self {
            magic: u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            inode_count: u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            mod_time: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            block_size: u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            frag_count: u32::from_le_bytes(bytes[16..20].try_into().unwrap()),
            compressor: u16::from_le_bytes(bytes[20..22].try_into().unwrap()).into(),
            block_log: u16::from_le_bytes(bytes[22..24].try_into().unwrap()),
            flags: u16::from_le_bytes(bytes[24..26].try_into().unwrap()),
            id_count: u16::from_le_bytes(bytes[26..28].try_into().unwrap()),
            ver_major: u16::from_le_bytes(bytes[28..30].try_into().unwrap()),
            ver_minor: u16::from_le_bytes(bytes[30..32].try_into().unwrap()),
            root_inode: u64::from_le_bytes(bytes[32..40].try_into().unwrap()),
            bytes_used: u64::from_le_bytes(bytes[40..48].try_into().unwrap()),
            id_table: u64::from_le_bytes(bytes[48..56].try_into().unwrap()),
            xattr_table: u64::from_le_bytes(bytes[56..64].try_into().unwrap()),
            inode_table: u64::from_le_bytes(bytes[64..72].try_into().unwrap()),
            dir_table: u64::from_le_bytes(bytes[72..80].try_into().unwrap()),
            frag_table: u64::from_le_bytes(bytes[80..88].try_into().unwrap()),
            export_table: u64::from_le_bytes(bytes[88..96].try_into().unwrap()),
        };

        if superblock.magic != 0x73717368 {
            return Err(());
        }

        if superblock.ver_major != 4 || superblock.ver_minor != 0 {
            return Err(());
        }

        if superblock.block_size > 1048576 {
            return Err(());
        }

        if superblock.block_log > 20 {
            return Err(());
        }

        if superblock.block_size != (1 << superblock.block_log) {
            return Err(());
        }

        if superblock.block_size == 0 {
            return Err(());
        }

        if ((superblock.block_size - 1) & superblock.block_size) != 0 {
            return Err(());
        }

        return Ok(superblock);
    }

    pub fn compressor(&self) -> SquashfsCompressionType {
        self.compressor
    }

    pub fn features(&self) -> SquashfsFeatures {
        let uncompressed_inodes = (self.flags & SquashfsFlags::UncompressedInodes as u16) != 0;
        let uncompressed_data_blocks =
            (self.flags & SquashfsFlags::UncompressedDataBlocks as u16) != 0;
        let _reserved = (self.flags & SquashfsFlags::Reserved as u16) != 0;
        let uncompressed_fragments =
            (self.flags & SquashfsFlags::UncompressedFragments as u16) != 0;
        let unused_fragments = (self.flags & SquashfsFlags::UnusedFragments as u16) != 0;
        let fragments_always_present =
            (self.flags & SquashfsFlags::FragmentsAlwaysPresent as u16) != 0;
        let deduplicated_data = (self.flags & SquashfsFlags::DeduplicatedData as u16) != 0;
        let nfs_table_present = (self.flags & SquashfsFlags::PresentNFSTable as u16) != 0;
        let uncompressed_xattrs = (self.flags & SquashfsFlags::UncompressedXattrs as u16) != 0;
        let no_xattrs = (self.flags & SquashfsFlags::NoXattrs as u16) != 0;
        let compressor_options_present =
            (self.flags & SquashfsFlags::PresentCompressorOptions as u16) != 0;
        let uncompressed_id_table = (self.flags & SquashfsFlags::UncompressedIDTable as u16) != 0;

        return SquashfsFeatures {
            uncompressed_inodes,
            uncompressed_data_blocks,
            _reserved,
            uncompressed_fragments,
            unused_fragments,
            fragments_always_present,
            deduplicated_data,
            nfs_table_present,
            uncompressed_xattrs,
            no_xattrs,
            compressor_options_present,
            uncompressed_id_table,
        };
    }
}
