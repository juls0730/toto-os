mod chunk_reader;
mod superblock;

use core::{fmt::Debug, mem::MaybeUninit, ptr::NonNull};

use alloc::{boxed::Box, string::String, sync::Arc, vec::Vec};

use super::vfs::{FsOps, VNode, VNodeOperations, VNodeType};

pub fn init() -> Squashfs<'static> {
    let initramfs = crate::libs::limine::get_module("initramfs.img");

    if initramfs.is_none() {
        panic!("Initramfs was not found!");
    }
    let initramfs = initramfs.unwrap();

    let squashfs = Squashfs::new(initramfs.addr());

    if squashfs.is_err() {
        panic!("Initramfs in corrupt!");
    }

    let squashfs = squashfs.unwrap();

    return squashfs;
}

#[repr(u8)]
#[derive(Clone, Copy)]
enum Table {
    Inode,
    Dir,
    Frag,
    Export,
    ID,
    Xattr,
}

#[repr(C)]
// #[derive(Debug)]
pub struct Squashfs<'a> {
    pub superblock: superblock::SquashfsSuperblock,
    start: *mut u8,
    decompressor: Box<dyn Fn(&'a [u8]) -> Result<Vec<u8>, ()>>,
    data_table: &'a [u8],
    inode_table: chunk_reader::ChunkReader<'a, Box<dyn Fn(&[u8]) -> Result<Vec<u8>, ()>>>,
    directory_table: chunk_reader::ChunkReader<'a, Box<dyn Fn(&[u8]) -> Result<Vec<u8>, ()>>>,
    fragment_table: Option<&'a [u8]>,
    export_table: Option<&'a [u8]>,
    id_table: &'a [u8],
    xattr_table: Option<&'a [u8]>,
}

impl Squashfs<'_> {
    fn new(ptr: *mut u8) -> Result<Squashfs<'static>, ()> {
        // crate::log_info!("Parsing initramfs at {:p}", ptr);

        // 40 is the offset for bytes used by the archive in the superblock
        let length = unsafe { u64::from_le(*(ptr.add(40).cast::<u64>())) as usize };

        let squashfs_data: &[u8] = unsafe { core::slice::from_raw_parts(ptr, length) };

        let superblock = superblock::SquashfsSuperblock::new(squashfs_data)?;

        let decompressor = match superblock.compressor() {
            superblock::SquashfsCompressionType::Gzip => {
                Box::new(crate::libs::gzip::uncompress_data)
            }
            compressor => panic!("Unsupported SquashFS decompressor {compressor:?}"),
        };

        // The easy part with none of this metadata nonesense
        let data_table = &squashfs_data[core::mem::size_of::<superblock::SquashfsSuperblock>()
            ..superblock.inode_table as usize];

        let mut tables: Vec<(Table, u64)> = Vec::new();

        // todo: there's probably a better way to do this
        tables.push((Table::Inode, superblock.inode_table));
        tables.push((Table::Dir, superblock.dir_table));

        if superblock.frag_table != u64::MAX {
            tables.push((Table::Frag, superblock.frag_table));
        }

        if superblock.export_table != u64::MAX {
            tables.push((Table::Export, superblock.export_table));
        }

        tables.push((Table::ID, superblock.id_table));

        if superblock.xattr_table != u64::MAX {
            tables.push((Table::Xattr, superblock.xattr_table));
        }

        let mut inode_table: MaybeUninit<
            chunk_reader::ChunkReader<'static, Box<dyn Fn(&[u8]) -> Result<Vec<u8>, ()>>>,
        > = MaybeUninit::uninit();
        let mut directory_table: MaybeUninit<
            chunk_reader::ChunkReader<'static, Box<dyn Fn(&[u8]) -> Result<Vec<u8>, ()>>>,
        > = MaybeUninit::uninit();
        let mut fragment_table = None;
        let mut export_table = None;
        let mut id_table: &[u8] = &[];
        let mut xattr_table = None;

        for (i, &(table, offset)) in tables.iter().enumerate() {
            let whole_table = if i == tables.len() - 1 {
                &squashfs_data[offset as usize..]
            } else {
                &squashfs_data[offset as usize..tables[i + 1].1 as usize]
            };

            match table {
                Table::Inode => {
                    inode_table = MaybeUninit::new(chunk_reader::ChunkReader::new(
                        whole_table,
                        decompressor.clone(),
                    ));
                }
                Table::Dir => {
                    directory_table = MaybeUninit::new(chunk_reader::ChunkReader::new(
                        whole_table,
                        decompressor.clone(),
                    ));
                }
                Table::Frag => {
                    fragment_table = Some(whole_table);
                }
                Table::Export => export_table = Some(whole_table),
                Table::ID => id_table = whole_table,
                Table::Xattr => xattr_table = Some(whole_table),
            }
        }

        return Ok(Squashfs {
            superblock,
            start: ptr,
            decompressor,
            data_table,
            inode_table: unsafe { inode_table.assume_init() },
            directory_table: unsafe { directory_table.assume_init() },
            fragment_table,
            export_table,
            id_table,
            xattr_table,
        });
    }

    #[inline(always)]
    fn get_inode_block_offset(&self, inode: u64) -> (u64, u16) {
        let inode_block = (inode >> 16) & 0x0000FFFFFFFFFFFF;
        let inode_offset = (inode & 0xFFFF) as u16;

        (inode_block, inode_offset)
    }

    fn read_root_dir(&mut self) -> Inode {
        self.read_inode(self.superblock.root_inode)
    }

    fn read_inode(&mut self, inode: u64) -> Inode {
        let (inode_block, inode_offset) = self.get_inode_block_offset(inode);

        let file_type = InodeFileType::from(u16::from_le_bytes(
            self.inode_table
                .get_slice(inode_block, inode_offset, 2)
                .try_into()
                .unwrap(),
        ));

        let inode_size = match file_type {
            InodeFileType::BasicDirectory => core::mem::size_of::<BasicDirectoryInode>(),
            InodeFileType::ExtendedDirectory => core::mem::size_of::<ExtendedDirectoryInode>(),
            InodeFileType::BasicFile => core::mem::size_of::<BasicFileInode>(),
            inode_type => unimplemented!("Inode type {inode_type:?}"),
        };

        let inode_bytes: &[u8] = &self
            .inode_table
            .get_slice(inode_block, inode_offset, inode_size);

        Inode::from(inode_bytes)
    }

    fn find_entry_in_directory(&mut self, dir: Inode, name: &str) -> Result<Inode, ()> {
        let dir_inode = match dir {
            Inode::BasicDirectory(dir) => {
                (dir.block_index as usize) << 16 | dir.block_offset as usize
            }
            Inode::ExtendedDirectory(dir) => {
                (dir.block_index as usize) << 16 | dir.block_offset as usize
            }
            _ => return Err(()),
        };

        let dir_size = match dir {
            Inode::BasicDirectory(dir) => dir.file_size as usize,
            Inode::ExtendedDirectory(dir) => dir.file_size as usize,
            _ => return Err(()),
        };

        if dir_size == 0 {
            // directory has no entries
            return Err(());
        }

        let (directory_block, directory_offset) = self.get_inode_block_offset(dir_inode as u64);

        let mut directory_table_header = {
            let bytes: &[u8] = &self.directory_table.get_slice(
                directory_block,
                directory_offset,
                core::mem::size_of::<DirectoryTableHeader>(),
            );

            DirectoryTableHeader::from(bytes)
        };

        let mut offset = core::mem::size_of::<DirectoryTableHeader>();
        let mut i = 0;

        loop {
            if i == directory_table_header.entry_count && offset != dir_size {
                //read second table
                directory_table_header = {
                    let bytes: &[u8] = &self.directory_table.get_slice(
                        directory_block,
                        directory_offset + offset as u16,
                        core::mem::size_of::<DirectoryTableHeader>(),
                    );

                    DirectoryTableHeader::from(bytes)
                };

                i = 0;
                offset += core::mem::size_of::<DirectoryTableHeader>();

                continue;
            }

            if offset >= dir_size {
                break;
            }

            let name_size = u16::from_le_bytes(
                self.directory_table
                    .get_slice(
                        directory_block,
                        directory_offset + (offset as u16 + 6),
                            2
                    )
                    .try_into()
                    .unwrap(),
            ) as usize
            // the name is stored off-by-one
                + 1;

            let directory_entry = DirectoryTableEntry::from_bytes(&self.directory_table.get_slice(
                directory_block,
                directory_offset + offset as u16,
                8 + name_size,
            ));

            offset += 8 + name_size;

            if directory_entry.name == name {
                let directory_entry_inode = (directory_table_header.start as usize) << 16
                    | (directory_entry.offset as usize);

                return Ok(self.read_inode(directory_entry_inode as u64));
            }

            i += 1;
        }

        return Err(());
    }

    // metadata_block takes a tuple, the first element is whether the array is a metadata block,
    // and the second element is a is_compressed override if the array is not a metadata block.
    fn get_decompressed_table(
        &self,
        table: &[u8],
        metadata_block: (bool, Option<bool>),
    ) -> Vec<u8> {
        // the bottom 15 bits, I think the last bit indicates whether the data is uncompressed
        let header = u16::from_le_bytes(table[0..2].try_into().unwrap());
        let table_is_compressed = if !metadata_block.0 {
            metadata_block.1.unwrap()
        } else {
            header & 0x8000 == 0
        };
        // let table_size = header & 0x7FFF;

        // if table.len() >= 8192 {
        //     panic!("Inode block is not less than 8KiB!");
        // }

        let mut buffer: Vec<u8> = Vec::new();
        let bytes = if metadata_block.0 { &table[2..] } else { table };

        if table_is_compressed {
            match self.superblock.compressor() {
                superblock::SquashfsCompressionType::Gzip => {
                    buffer.extend_from_slice(
                        &crate::libs::gzip::uncompress_data(bytes).unwrap_or(bytes.to_vec()),
                    );
                }
                _ => {
                    crate::log!(
                        crate::LogLevel::Error,
                        "Unsupported squashfs compression type"
                    )
                }
            }
        } else {
            buffer.extend(bytes);
        }

        return buffer;
    }
}

impl<'a> FsOps for Squashfs<'a> {
    fn mount(&mut self, _path: &str, data: &mut *mut u8, _vfsp: NonNull<super::vfs::Vfs>) {
        // STUB

        // not recommended:tm:
        *data = core::ptr::addr_of!(*self) as *mut u8;
    }

    fn unmount(&mut self, _vfsp: NonNull<super::vfs::Vfs>) {
        // STUB
    }

    fn root(&mut self, vfsp: NonNull<super::vfs::Vfs>) -> super::vfs::VNode {
        let root_dir = self.read_root_dir();

        return VNode::new(Box::new(root_dir), VNodeType::Directory, vfsp);
    }

    fn fid(&mut self, _path: &str, _vfsp: NonNull<super::vfs::Vfs>) -> Option<super::vfs::FileId> {
        todo!();
    }

    fn statfs(&mut self, _vfsp: NonNull<super::vfs::Vfs>) -> super::vfs::StatFs {
        todo!();
    }

    fn sync(&mut self, _vfsp: NonNull<super::vfs::Vfs>) {
        todo!();
    }

    fn vget(
        &mut self,
        _fid: super::vfs::FileId,
        _vfsp: NonNull<super::vfs::Vfs>,
    ) -> super::vfs::VNode {
        todo!();
    }
}

#[derive(Clone, Copy, Debug)]
enum Inode {
    BasicFile(BasicFileInode),
    BasicDirectory(BasicDirectoryInode),
    ExtendedDirectory(ExtendedDirectoryInode),
}

impl From<&[u8]> for Inode {
    fn from(value: &[u8]) -> Self {
        let file_type = InodeFileType::from(u16::from_le_bytes(value[0..2].try_into().unwrap()));

        match file_type {
            InodeFileType::BasicDirectory => {
                Inode::BasicDirectory(BasicDirectoryInode::from_bytes(value))
            }
            InodeFileType::ExtendedDirectory => {
                Inode::ExtendedDirectory(ExtendedDirectoryInode::from_bytes(value))
            }
            InodeFileType::BasicFile => Inode::BasicFile(BasicFileInode::from_bytes(value)),
            _ => unimplemented!("Inode from bytes"),
        }
    }
}

impl VNodeOperations for Inode {
    fn open(&mut self, _f: u32, _c: super::vfs::UserCred, _vp: NonNull<VNode>) {}

    fn close(&mut self, _f: u32, _c: super::vfs::UserCred, _vp: NonNull<VNode>) {}

    fn read(
        &mut self,
        count: usize,
        offset: usize,
        _f: u32,
        _c: super::vfs::UserCred,
        vp: NonNull<VNode>,
    ) -> Result<Arc<[u8]>, ()> {
        let squashfs = unsafe { (*vp.as_ptr()).parent_vfs.as_mut().data.cast::<Squashfs>() };

        match self {
            Inode::BasicFile(file) => unsafe {
                // TODO: is this really how you're supposed to do this?
                let mut block_data: Vec<u8> = Vec::with_capacity(count);

                let data_table: Vec<u8>;

                let block_offset = if file.frag_idx == u32::MAX {
                    data_table = (*squashfs).get_decompressed_table(
                        (*squashfs).data_table,
                        (
                            false,
                            Some(!(*squashfs).superblock.features().uncompressed_data_blocks),
                        ),
                    );

                    file.block_offset as usize
                } else {
                    // Tail end packing
                    let fragment_table = (*squashfs).get_decompressed_table(
                        (*squashfs).fragment_table.unwrap(),
                        (
                            false,
                            Some(!(*squashfs).superblock.features().uncompressed_fragments),
                        ),
                    );

                    let fragment_pointer = ((*squashfs).start as u64
                        + u64::from_le_bytes(
                            fragment_table[file.frag_idx as usize..(file.frag_idx + 8) as usize]
                                .try_into()
                                .unwrap(),
                        )) as *mut u8;

                    // build array since fragment_pointer is not guaranteed to be 0x02 aligned
                    // We add two since fragment_pointer points to the beginning of the fragment block,
                    // Which is a metadata block, and we get the size, but that excludes the two header bytes,
                    // And since we are building the array due to unaligned pointer shenanigans we need to
                    // include the header bytes otherwise we are short by two bytes
                    let fragment_block_size =
                        (u16::from_le(core::ptr::read_unaligned(fragment_pointer.cast::<u16>()))
                            & 0x7FFF)
                            + 2;

                    let mut fragment_block_raw = Vec::new();
                    for i in 0..fragment_block_size as usize {
                        fragment_block_raw.push(core::ptr::read_unaligned(fragment_pointer.add(i)))
                    }

                    let fragment_block =
                        (*squashfs).get_decompressed_table(&fragment_block_raw, (true, None));

                    let fragment_start =
                        u64::from_le_bytes(fragment_block[0..8].try_into().unwrap());
                    let fragment_size =
                        u32::from_le_bytes(fragment_block[8..12].try_into().unwrap());
                    let fragment_compressed = fragment_size & 1 << 24 == 0;
                    let fragment_size = fragment_size & 0xFEFFFFFF;

                    let data_table_raw = core::slice::from_raw_parts(
                        ((*squashfs).start as u64 + fragment_start) as *mut u8,
                        fragment_size as usize,
                    )
                    .to_vec();

                    data_table = (*squashfs).get_decompressed_table(
                        &data_table_raw,
                        (false, Some(fragment_compressed)),
                    );

                    file.block_offset as usize
                } + offset;

                block_data.extend(&data_table[block_offset..(block_offset + count)]);

                return Ok(Arc::from(block_data));
            },
            _ => panic!("Tried to open non-file"),
        }
    }

    fn write(
        &mut self,
        _offset: usize,
        _buf: &[u8],
        _f: u32,
        _c: super::vfs::UserCred,
        _vp: NonNull<VNode>,
    ) {
        todo!()
    }

    fn ioctl(
        &mut self,
        _com: u32,
        _d: *mut u8,
        _f: u32,
        _c: super::vfs::UserCred,
        _vp: NonNull<VNode>,
    ) {
        todo!()
    }

    fn getattr(&mut self, _c: super::vfs::UserCred, _vp: NonNull<VNode>) -> super::vfs::VAttr {
        todo!()
    }

    fn setattr(&mut self, _va: super::vfs::VAttr, _c: super::vfs::UserCred, _vp: NonNull<VNode>) {
        todo!()
    }

    fn access(&mut self, _m: u32, _c: super::vfs::UserCred, _vp: NonNull<VNode>) {
        todo!()
    }

    fn lookup(
        &mut self,
        nm: &str,
        _c: super::vfs::UserCred,
        vp: NonNull<VNode>,
    ) -> Result<super::vfs::VNode, ()> {
        let squashfs = unsafe { (*vp.as_ptr()).parent_vfs.as_mut().data.cast::<Squashfs>() };

        match self {
            Inode::BasicDirectory(_) | Inode::ExtendedDirectory(_) => unsafe {
                let inode = (*squashfs).find_entry_in_directory(*self, nm)?;
                let vnode_type = match inode {
                    Inode::BasicDirectory(_) | Inode::ExtendedDirectory(_) => VNodeType::Directory,
                    Inode::BasicFile(_) => VNodeType::Regular,
                };

                let vnode = VNode::new(Box::new(inode), vnode_type, (*vp.as_ptr()).parent_vfs);

                return Ok(vnode);
            },
            _ => panic!("tried to lookup on non directory"),
        }
    }

    fn create(
        &mut self,
        _nm: &str,
        _va: super::vfs::VAttr,
        _e: u32,
        _m: u32,
        _c: super::vfs::UserCred,
        _vp: NonNull<VNode>,
    ) -> Result<super::vfs::VNode, ()> {
        todo!()
    }

    fn link(
        &mut self,
        _target_dir: *mut super::vfs::VNode,
        _target_name: &str,
        _c: super::vfs::UserCred,
        _vp: NonNull<VNode>,
    ) {
        todo!()
    }

    fn rename(
        &mut self,
        _nm: &str,
        _target_dir: *mut super::vfs::VNode,
        _target_name: &str,
        _c: super::vfs::UserCred,
        _vp: NonNull<VNode>,
    ) {
        todo!()
    }

    fn mkdir(
        &mut self,
        _nm: &str,
        _va: super::vfs::VAttr,
        _c: super::vfs::UserCred,
        _vp: NonNull<VNode>,
    ) -> Result<super::vfs::VNode, ()> {
        todo!()
    }

    fn readdir(
        &mut self,
        _uiop: *const super::vfs::UIO,
        _c: super::vfs::UserCred,
        _vp: NonNull<VNode>,
    ) {
        todo!()
    }

    fn symlink(
        &mut self,
        _link_name: &str,
        _va: super::vfs::VAttr,
        _target_name: &str,
        _c: super::vfs::UserCred,
        _vp: NonNull<VNode>,
    ) {
        todo!()
    }

    fn readlink(
        &mut self,
        _uiop: *const super::vfs::UIO,
        _c: super::vfs::UserCred,
        _vp: NonNull<VNode>,
    ) {
        todo!()
    }

    fn fsync(&mut self, _c: super::vfs::UserCred, _vp: NonNull<VNode>) {
        todo!()
    }

    fn len(&self, _vp: NonNull<VNode>) -> usize {
        match self {
            Inode::BasicFile(file) => file.file_size as usize,
            _ => panic!("idk"),
        }
    }
}

macro_rules! inode_enum_try_into {
    ($inode_type:ty, $inode_name:ident) => {
        impl<'a> TryInto<$inode_type> for Inode {
            type Error = ();

            fn try_into(self) -> Result<$inode_type, Self::Error> {
                match self {
                    Inode::$inode_name(inode) => {
                        return Ok(inode);
                    }
                    _ => {
                        return Err(());
                    }
                }
            }
        }
    };
}

inode_enum_try_into!(BasicFileInode, BasicFile);
inode_enum_try_into!(BasicDirectoryInode, BasicDirectory);
inode_enum_try_into!(ExtendedDirectoryInode, ExtendedDirectory);

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct InodeHeader {
    file_type: InodeFileType,
    _reserved: [u16; 3],
    mtime: u32,
    inode_num: u32,
}

impl InodeHeader {
    fn from_bytes(bytes: &[u8]) -> Self {
        let file_type = u16::from_le_bytes(bytes[0..2].try_into().unwrap()).into();
        let mtime = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let inode_num = u32::from_le_bytes(bytes[12..16].try_into().unwrap());

        return Self {
            // squashfs,
            file_type,
            _reserved: [0; 3],
            mtime,
            inode_num,
        };
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct BasicDirectoryInode {
    header: InodeHeader,
    block_index: u32,  // 4
    link_count: u32,   // 8
    file_size: u16,    // 10
    block_offset: u16, // 12
    parent_inode: u32, // 16
}

impl BasicDirectoryInode {
    fn from_bytes(bytes: &[u8]) -> Self {
        let header = InodeHeader::from_bytes(bytes);
        let block_index = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let link_count = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
        let file_size = u16::from_le_bytes(bytes[24..26].try_into().unwrap());
        let block_offset = u16::from_le_bytes(bytes[26..28].try_into().unwrap());
        let parent_inode = u32::from_le_bytes(bytes[28..32].try_into().unwrap());

        return Self {
            header,
            block_index,
            link_count,
            file_size,
            block_offset,
            parent_inode,
        };
    }

    // #[allow(dead_code)]
    // fn entries(&self) -> Arc<[Inode]> {
    //     let mut entries: Vec<Inode> = Vec::new();

    //     let directory_table = &self
    //         .header
    //         .squashfs
    //         .get_decompressed_table(self.header.squashfs.directory_table, (true, None));

    //     let directory_table_header =
    //         DirectoryTableHeader::from_bytes(&directory_table[self.block_offset as usize..]);

    //     // TODO: cheap hack, fix it when I have more hours of sleep.
    //     let mut offset = self.block_offset as usize + core::mem::size_of::<DirectoryTableHeader>();

    //     for _ in 0..directory_table_header.entry_count as usize {
    //         let directory_table_entry = DirectoryTableEntry::from_bytes(&directory_table[offset..]);

    //         offset += 8 + directory_table_entry.name.len();

    //         let file_inode = self
    //             .header
    //             .squashfs
    //             .read_inode(directory_table_entry.offset as u32);

    //         entries.push(file_inode);
    //     }

    //     return Arc::from(entries);
    // }

    // fn find(&self, name: &str) -> Option<Inode<'a>> {
    //     let directory_table = &self
    //         .header
    //         .squashfs
    //         .get_decompressed_table(self.header.squashfs.directory_table, (true, None));

    //     let directory_table_header =
    //         DirectoryTableHeader::from_bytes(&directory_table[self.block_offset as usize..]);

    //     // TODO: cheap hack, fix it when I have more hours of sleep.
    //     let mut offset = self.block_offset as usize + core::mem::size_of::<DirectoryTableHeader>();
    //     InodeHeader
    //     for _ in 0..directory_table_header.entry_count as usize {
    //         let directory_table_entry = DirectoryTableEntry::from_bytes(&directory_table[offset..]);

    //         offset += 8 + directory_table_entry.name.len();

    //         if directory_table_entry.name == name {
    //             return Some(
    //                 self.header
    //                     .squashfs
    //                     .read_inode(directory_table_entry.offset as u32),
    //             );
    //         }
    //     }

    //     return None;
    // }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct ExtendedDirectoryInode {
    header: InodeHeader,
    link_count: u32,   // 8
    file_size: u32,    // 10
    block_index: u32,  // 4
    parent_inode: u32, // 16
    index_count: u16,
    block_offset: u16, // 12
    xattr_index: u32,
}

impl ExtendedDirectoryInode {
    fn from_bytes(bytes: &[u8]) -> Self {
        let header = InodeHeader::from_bytes(bytes);
        let link_count = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let file_size = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
        let block_index = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
        let parent_inode = u32::from_le_bytes(bytes[28..32].try_into().unwrap());
        let index_count = u16::from_le_bytes(bytes[32..34].try_into().unwrap());
        let block_offset = u16::from_le_bytes(bytes[34..36].try_into().unwrap());
        let xattr_index = u32::from_le_bytes(bytes[36..40].try_into().unwrap());

        return Self {
            header,
            link_count,
            file_size,
            block_index,
            parent_inode,
            index_count,
            block_offset,
            xattr_index,
        };
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct BasicFileInode {
    header: InodeHeader,
    block_start: u32,  // 4
    frag_idx: u32,     // 8
    block_offset: u32, // 12
    file_size: u32,    // 16
                       // block_sizes: *const u32,
}

impl BasicFileInode {
    fn from_bytes(bytes: &[u8]) -> Self {
        let header = InodeHeader::from_bytes(bytes);
        let block_start = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let frag_idx = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
        let block_offset = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
        let file_size = u32::from_le_bytes(bytes[28..32].try_into().unwrap());
        // let block_sizes = bytes[32..].as_ptr() as *const u32;

        return Self {
            header,
            block_start,
            frag_idx,
            block_offset,
            file_size,
            // block_sizes,
        };
    }
}

#[repr(C)]
#[derive(Debug)]
struct DirectoryTableHeader {
    entry_count: u32,
    start: u32,
    inode_num: u32,
}

impl From<&[u8]> for DirectoryTableHeader {
    fn from(value: &[u8]) -> Self {
        // count is off by 1 entry
        let entry_count = u32::from_le_bytes(value[0..4].try_into().unwrap()) + 1;
        let start = u32::from_le_bytes(value[4..8].try_into().unwrap());
        let inode_num = u32::from_le_bytes(value[8..12].try_into().unwrap());

        return Self {
            entry_count,
            start,
            inode_num,
        };
    }
}

#[repr(C)]
#[derive(Debug)]
struct DirectoryTableEntry {
    offset: u16,
    inode_offset: i16,
    inode_type: InodeFileType,
    name_size: u16,
    name: String, // the file name length is name_size + 1 bytes
}

impl DirectoryTableEntry {
    fn from_bytes(bytes: &[u8]) -> Self {
        let offset = u16::from_le_bytes(bytes[0..2].try_into().unwrap());
        let inode_offset = i16::from_le_bytes(bytes[2..4].try_into().unwrap());
        let inode_type = u16::from_le_bytes(bytes[4..6].try_into().unwrap()).into();
        let name_size = u16::from_le_bytes(bytes[6..8].try_into().unwrap());
        let name = String::from_utf8(bytes[8..((name_size as usize) + 1) + 8].to_vec()).unwrap();
        // let name = core::str::from_utf8(&bytes[8..((name_size as usize) + 1) + 8])
        //     .expect("Failed to make DirectoryHeader name");

        return Self {
            offset,
            inode_offset,
            inode_type,
            name_size,
            name,
        };
    }
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InodeFileType {
    BasicDirectory = 1,
    BasicFile = 2,
    BasicSymlink = 3,
    BasicBlockDevice = 4,
    BasicCharDevice = 5,
    BasicPipe = 6,
    BasicSocked = 7,
    ExtendedDirectory = 8,
    ExtendedFile = 9,
    ExtendedSymlink = 10,
    ExtendedBlockDevice = 11,
    ExtendedPipe = 12,
    ExtendedSocked = 13,
}

impl From<u16> for InodeFileType {
    fn from(value: u16) -> Self {
        match value {
            1 => Self::BasicDirectory,
            2 => Self::BasicFile,
            3 => Self::BasicSymlink,
            4 => Self::BasicBlockDevice,
            5 => Self::BasicCharDevice,
            6 => Self::BasicPipe,
            7 => Self::BasicSocked,
            8 => Self::ExtendedDirectory,
            9 => Self::ExtendedFile,
            10 => Self::ExtendedSymlink,
            11 => Self::ExtendedBlockDevice,
            12 => Self::ExtendedPipe,
            13 => Self::ExtendedSocked,
            _ => panic!("Unexpected Inode file type {value}!"),
        }
    }
}
