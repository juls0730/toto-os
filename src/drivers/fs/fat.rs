use core::ptr::NonNull;

use alloc::{
    boxed::Box,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};

use crate::{drivers::storage::Partition, LogLevel};

use super::vfs::{FsOps, VNode, VNodeOperations};

// The first Cluster (perhaps 0xF0FFFF0F) is the FAT ID
// The second cluster stores the end-of-cluster-chain marker
// The third entry and further holds the directory table
//
// Fat Clusters are either one of these types:
//
// |     fat12     |      fat16      |          fat32          |          Description          |
// |---------------|-----------------|-------------------------|-------------------------------|
// |  0xFF8-0xFFF  |  0xFFF8-0xFFFF  |  0x0FFFFFF8-0x0FFFFFFF  | End Of cluster Chain          |
// |  0xFF7        |  0xFFF7         |  0x0FFFFFF7             | Bad Cluster                   |
// |  0x002-0xFEF  |  0x0002-0xFFEF  |  0x00000002-0x0FFFFFEF  | In use Cluster                |
// |  0x000        |  0x0000         |  0x00000000             | Free Cluster                  |

// End Of Chain
const EOC_12: u32 = 0x0FF8;
const EOC_16: u32 = 0xFFF8;
const EOC_32: u32 = 0x0FFFFFF8;

#[derive(Clone, Copy, Debug)]
enum FatType {
    Fat12(Fat16EBPB),
    Fat16(Fat16EBPB),
    Fat32(Fat32EBPB),
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct BIOSParameterBlock {
    _jmp_instruction: [u8; 3],
    pub oem_identifier: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub fat_count: u8,
    pub root_directory_count: u16,
    pub total_sectors: u16,
    pub media_descriptor_type: u8,
    pub sectors_per_fat: u16,
    pub sectors_per_track: u16,
    pub head_count: u16,
    pub hidden_sectors: u32,
    pub large_sector_count: u32,
    pub ebpb_bytes: [u8; 54],
}

// Fat 12 and Fat 16 EBPB
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct Fat16EBPB {
    pub drive_number: u8,
    _reserved: u8,
    pub signature: u8,
    pub volume_id: u32,
    pub volume_label: [u8; 11],
    pub system_identifier_string: [u8; 8],
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct Fat32EBPB {
    pub sectors_per_fat_ext: u32,
    pub flags: [u8; 2],
    pub fat_version: u16,
    pub root_dir_cluster: u32,
    pub fsinfo_sector: u16,
    pub backup_bootsector: u16,
    _reserved: [u8; 12],
    pub drive_number: u8,
    _reserved2: u8,
    pub signature: u8,
    pub volume_id: u32,
    pub volume_label: [u8; 11],
    pub system_identifier_string: [u8; 8],
}

#[repr(C, packed)]
#[derive(Debug)]
pub struct FSInfo {
    pub lead_signature: u32,
    pub mid_signature: u32,
    pub last_known_free_cluster: u32,
    pub look_for_free_clusters: u32,
    _reserved2: [u8; 12],
    pub trail_signature: u32,
}

impl FSInfo {
    pub fn from_bytes(bytes: Arc<[u8]>) -> Self {
        assert!(bytes.len() >= 512);

        let lead_signature = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let mid_signature = u32::from_le_bytes(bytes[484..488].try_into().unwrap());
        let last_known_free_cluster = u32::from_le_bytes(bytes[488..492].try_into().unwrap());
        let look_for_free_clusters = u32::from_le_bytes(bytes[492..496].try_into().unwrap());
        let _reserved2 = bytes[496..508].try_into().unwrap();
        let trail_signature = u32::from_le_bytes(bytes[508..].try_into().unwrap());

        return Self {
            lead_signature,
            mid_signature,
            last_known_free_cluster,
            look_for_free_clusters,
            _reserved2,
            trail_signature,
        };
    }
}

#[repr(u8)]
#[derive(Debug, PartialEq)]
#[allow(dead_code)]
enum FileEntryAttributes {
    ReadOnly = 0x01,
    Hidden = 0x02,
    System = 0x04,
    VolumeId = 0x08,
    Directory = 0x10,
    Archive = 0x20, // basically any file
    LongFileName = 0x0F,
}

#[repr(C, packed)]
#[derive(Debug)]
struct LongFileName {
    entry_order: u8,
    first_characters: [u16; 5],
    attribute: u8,       // always 0x0F
    long_entry_type: u8, // zero for name entries
    checksum: u8,
    second_characters: [u16; 6],
    _always_zero: [u8; 2],
    final_characters: [u16; 2],
}

#[repr(C, packed)]
#[derive(Debug)]
pub struct FileEntry {
    file_name: [u8; 8],
    extension: [u8; 3],
    attributes: u8,
    _reserved: u8,
    creation_tenths: u8,
    creation_time: u16,
    creation_date: u16,
    accessed_date: u16,
    high_first_cluster_number: u16, // The high 16 bits of this entry's first cluster number. For FAT 12 and FAT 16 this is always zero.
    modified_time: u16,
    modified_date: u16,
    low_first_cluster_number: u16,
    file_size: u32,
}

impl FileEntry {
    pub fn cluster(&self) -> u32 {
        let mut cluster = self.low_first_cluster_number as u32;
        cluster |= (self.high_first_cluster_number as u32) << 16;
        return cluster;
    }
}

pub struct FatFs {
    partition: Partition,
    // FAT info
    #[allow(dead_code)]
    fs_info: Option<FSInfo>,
    fat: Option<Arc<[u32]>>,
    bpb: BIOSParameterBlock,
    fat_start: u64,
    fat_type: FatType,
    cluster_size: usize,
    sectors_per_fat: usize,
}

impl FatFs {
    pub fn new(partition: Partition) -> Result<Self, ()> {
        let bpb_bytes = partition
            .read(0, 1)
            .expect("Failed to read FAT32 BIOS Parameter Block!");

        let bpb = unsafe { *(bpb_bytes.as_ptr().cast::<BIOSParameterBlock>()) };

        let (total_sectors, fat_size) = if bpb.total_sectors == 0 {
            (bpb.large_sector_count, unsafe {
                (*bpb.ebpb_bytes.as_ptr().cast::<Fat32EBPB>()).sectors_per_fat_ext
            })
        } else {
            (bpb.total_sectors as u32, bpb.sectors_per_fat as u32)
        };

        let root_dir_sectors =
            ((bpb.root_directory_count * 32) + (bpb.bytes_per_sector - 1)) / bpb.bytes_per_sector;
        let total_data_sectors = total_sectors
            - (bpb.reserved_sectors as u32
                + (bpb.fat_count as u32 * fat_size)
                + root_dir_sectors as u32);

        let total_clusters = total_data_sectors / bpb.sectors_per_cluster as u32;

        let fat_type = if total_clusters < 4085 {
            FatType::Fat12(unsafe { *bpb.ebpb_bytes.as_ptr().cast::<Fat16EBPB>() })
        } else if total_clusters < 65525 {
            FatType::Fat16(unsafe { *bpb.ebpb_bytes.as_ptr().cast::<Fat16EBPB>() })
        } else {
            FatType::Fat32(unsafe { *bpb.ebpb_bytes.as_ptr().cast::<Fat32EBPB>() })
        };

        let system_ident = match fat_type {
            FatType::Fat12(ebpb) => ebpb.system_identifier_string,
            FatType::Fat16(ebpb) => ebpb.system_identifier_string,
            FatType::Fat32(ebpb) => ebpb.system_identifier_string,
        };

        let system_identifier = core::str::from_utf8(&system_ident);

        if system_identifier.is_err() {
            return Err(());
        }

        if let Ok(system_identifier_string) = system_identifier {
            match fat_type {
                FatType::Fat12(_) => {
                    if !system_identifier_string.contains("FAT12") {
                        return Err(());
                    }
                }
                FatType::Fat16(_) => {
                    if !system_identifier_string.contains("FAT16") {
                        return Err(());
                    }
                }
                FatType::Fat32(_) => {
                    if !system_identifier_string.contains("FAT32") {
                        return Err(());
                    }
                }
            }
        }

        let fs_info = match fat_type {
            FatType::Fat32(ebpb) => {
                let fsinfo_bytes = partition
                    .read(ebpb.fsinfo_sector as u64, 1)
                    .expect("Failed to read FSInfo sector!");

                Some(FSInfo::from_bytes(fsinfo_bytes))
            }
            _ => None,
        };

        let fat_start = bpb.reserved_sectors as u64;

        let sectors_per_fat = match fat_type {
            FatType::Fat32(ebpb) => ebpb.sectors_per_fat_ext as usize,
            _ => bpb.sectors_per_fat as usize,
        };

        // crate::println!("Found {fat_type:?} FS");

        let cluster_size = bpb.sectors_per_cluster as usize * 512;

        return Ok(Self {
            partition,
            fs_info,
            fat: None,
            bpb,
            fat_start,
            fat_type,
            cluster_size,
            sectors_per_fat,
        });
    }

    fn find_entry_in_directory(&self, cluster: usize, name: &str) -> Result<FileEntry, ()> {
        let mut i: usize = 0;
        // Long file name is stored outsize because long filename and the real entry on separate entries
        let mut long_filename: Vec<LongFileName> = Vec::new();
        let mut long_filename_string: Option<String> = None;

        let data_sector = self.read_cluster(cluster)?;

        loop {
            let bytes: [u8; core::mem::size_of::<FileEntry>()] =
                data_sector[(i * 32)..((i + 1) * 32)].try_into().unwrap();
            let first_byte = bytes[0];

            let file_entry: FileEntry;

            i += 1;

            // Step 1
            if first_byte == 0x00 {
                break; // End of directory listing
            }

            // Step 2
            if first_byte == 0xE5 {
                continue; // Directory is unused, ignore it
            } else if bytes[11] == FileEntryAttributes::LongFileName as u8 {
                // Entry is LFN (step 3)
                // read long filename (step 4)
                let long_filename_part: LongFileName;

                unsafe {
                    long_filename_part = core::mem::transmute(bytes);
                }
                long_filename.push(long_filename_part);
                continue;
            } else {
                // step 5
                unsafe {
                    file_entry = core::mem::transmute(bytes);
                }

                // step 6
                if !long_filename.is_empty() {
                    // Make fileEntry with LFN (step 7)
                    let mut string: Vec<u16> = Vec::with_capacity(long_filename.len() * 13);

                    for i in 0..long_filename.len() {
                        let i = (long_filename.len() - 1) - i;
                        let long_filename = &long_filename[i];

                        let mut character_bytes = Vec::new();
                        let characters = long_filename.first_characters;

                        character_bytes.extend_from_slice(&characters);
                        let characters = long_filename.second_characters;

                        character_bytes.extend_from_slice(&characters);
                        let characters = long_filename.final_characters;

                        character_bytes.extend_from_slice(&characters);

                        // remove 0x0000 characters and 0xFFFF characters
                        character_bytes.retain(|&x| x != 0xFFFF && x != 0x0000);

                        for &le_character in character_bytes.iter() {
                            // Convert little-endian u16 to native-endian u16
                            let native_endian_value = u16::from_le(le_character);
                            string.push(native_endian_value);
                        }
                    }
                    long_filename_string = Some(String::from_utf16(&string).unwrap());
                    long_filename.clear();
                }
            }

            let raw_short_filename = core::str::from_utf8(&file_entry.file_name)
                .unwrap()
                .trim_end();
            let raw_short_extension = core::str::from_utf8(&file_entry.extension)
                .unwrap()
                .trim_end();
            let formatted_short_filename = match raw_short_extension.is_empty() {
                true => raw_short_filename.to_string(),
                false => alloc::format!("{}.{}", raw_short_filename, raw_short_extension),
            };

            if let Some(ref filename) = long_filename_string {
                if filename != name {
                    continue;
                }
            } else if name.to_uppercase() != formatted_short_filename {
                continue;
            }

            return Ok(file_entry);
        }

        return Err(());
    }

    pub fn read_cluster(&self, cluster: usize) -> Result<Arc<[u8]>, ()> {
        return self.partition.read(
            self.cluster_to_sector(cluster) as u64,
            self.bpb.sectors_per_cluster as usize,
        );
    }

    fn cluster_to_sector(&self, cluster: usize) -> usize {
        let fat_size = self.sectors_per_fat;
        let root_dir_sectors = ((self.bpb.root_directory_count * 32)
            + (self.bpb.bytes_per_sector - 1))
            / self.bpb.bytes_per_sector;

        let first_data_sector = self.bpb.reserved_sectors as usize
            + (self.bpb.fat_count as usize * fat_size)
            + root_dir_sectors as usize;

        return ((((cluster.wrapping_sub(2)) as isize)
            .wrapping_mul(self.bpb.sectors_per_cluster as isize)) as usize)
            .wrapping_add(first_data_sector);
    }

    fn sector_to_cluster(&self, sector: usize) -> usize {
        let fat_size = self.sectors_per_fat;
        let root_dir_sectors = ((self.bpb.root_directory_count * 32)
            + (self.bpb.bytes_per_sector - 1))
            / self.bpb.bytes_per_sector;

        let first_data_sector = self.bpb.reserved_sectors as usize
            + (self.bpb.fat_count as usize * fat_size)
            + root_dir_sectors as usize;

        return (((sector).wrapping_sub(first_data_sector))
            .wrapping_div(self.bpb.sectors_per_cluster as usize))
        .wrapping_add(2);
    }

    fn get_next_cluster(&self, cluster: usize) -> u32 {
        if crate::KERNEL_FEATURES.fat_in_mem {
            return match self.fat_type {
                FatType::Fat12(_) => self.fat.as_ref().unwrap()[cluster] & 0x0FFF,
                FatType::Fat16(_) => self.fat.as_ref().unwrap()[cluster] & 0xFFFF,
                FatType::Fat32(_) => self.fat.as_ref().unwrap()[cluster] & 0x0FFFFFFF,
            };
        } else {
            let fat_entry_size = match self.fat_type {
                FatType::Fat12(_) => 2, // 12 bits per entry
                FatType::Fat16(_) => 2, // 16 bits per entry
                FatType::Fat32(_) => 4, // 28 bits per entry
            };
            let entry_offset = cluster * fat_entry_size;
            let entry_offset_in_sector = entry_offset % 512;

            // needs two incase we "straddle a sector"
            let sector_data = self
                .partition
                .read(self.fat_start + entry_offset as u64 / 512, 2)
                .expect("Failed to read from FAT!");

            match self.fat_type {
                FatType::Fat12(_) => {
                    let cluster_entry_bytes: [u8; 2] = sector_data
                        [entry_offset_in_sector..entry_offset_in_sector + 2]
                        .try_into()
                        .unwrap();
                    return (u16::from_le_bytes(cluster_entry_bytes) & 0x0FFF) as u32;
                }
                FatType::Fat16(_) => {
                    let cluster_entry_bytes: [u8; 2] = sector_data
                        [entry_offset_in_sector..entry_offset_in_sector + 2]
                        .try_into()
                        .unwrap();
                    return (u16::from_le_bytes(cluster_entry_bytes)) as u32;
                }
                FatType::Fat32(_) => {
                    let cluster_entry_bytes: [u8; 4] = sector_data
                        [entry_offset_in_sector..entry_offset_in_sector + 4]
                        .try_into()
                        .unwrap();
                    return u32::from_le_bytes(cluster_entry_bytes) & 0x0FFFFFFF;
                }
            }
        }
    }
}

impl FsOps for FatFs {
    fn mount(&mut self, _path: &str, data: &mut *mut u8, _vfsp: NonNull<super::vfs::Vfs>) {
        let bytes_per_fat = 512 * self.sectors_per_fat;

        let mut fat: Option<Arc<[u32]>> = None;

        if crate::KERNEL_FEATURES.fat_in_mem {
            let cluster_bytes = match self.fat_type {
                FatType::Fat32(_) => 4,
                _ => 2,
            };

            let mut fat_vec: Vec<u32> = Vec::with_capacity(bytes_per_fat / cluster_bytes);

            for i in 0..self.sectors_per_fat {
                let sector = self
                    .partition
                    .read(self.fat_start + i as u64, 1)
                    .expect("Failed to read FAT");
                for j in 0..(512 / cluster_bytes) {
                    match self.fat_type {
                        FatType::Fat32(_) => fat_vec.push(u32::from_le_bytes(
                            sector[j * cluster_bytes..(j * cluster_bytes + cluster_bytes)]
                                .try_into()
                                .unwrap(),
                        )),
                        _ => fat_vec.push(u16::from_le_bytes(
                            sector[j * cluster_bytes..(j * cluster_bytes + cluster_bytes)]
                                .try_into()
                                .unwrap(),
                        ) as u32),
                    }
                }
            }

            fat = Some(Arc::from(fat_vec));
        } else {
            crate::log!(
                LogLevel::Warn,
                "FAT is not being stored in memory, this feature is experimental and file reads are expected to be slower."
            )
        }

        self.fat = fat;

        *data = core::ptr::addr_of!(*self) as *mut u8;
    }

    fn unmount(&mut self, _vfsp: NonNull<super::vfs::Vfs>) {
        self.fat = None;
    }

    fn root(&mut self, vfsp: NonNull<super::vfs::Vfs>) -> super::vfs::VNode {
        let root_cluster = match self.fat_type {
            FatType::Fat32(ebpb) => ebpb.root_dir_cluster as usize,
            _ => self.sector_to_cluster(
                self.bpb.reserved_sectors as usize
                    + (self.bpb.fat_count as usize * self.sectors_per_fat),
            ),
        };

        let file = File::Dir(root_cluster);

        return VNode::new(Box::new(file), super::vfs::VNodeType::Directory, vfsp);
    }

    fn fid(&mut self, _path: &str, _vfsp: NonNull<super::vfs::Vfs>) -> Option<super::vfs::FileId> {
        todo!("FAT FID");
    }

    fn statfs(&mut self, _vfsp: NonNull<super::vfs::Vfs>) -> super::vfs::StatFs {
        todo!("FAT STATFS");
    }

    fn sync(&mut self, _vfsp: NonNull<super::vfs::Vfs>) {
        todo!("FAT SYNC");
    }

    fn vget(
        &mut self,
        _fid: super::vfs::FileId,
        _vfsp: NonNull<super::vfs::Vfs>,
    ) -> super::vfs::VNode {
        todo!("FAT VGET");
    }
}

enum File {
    Archive(FileEntry),
    // directory cluster
    Dir(usize),
}

impl VNodeOperations for File {
    fn open(&mut self, _f: u32, _c: super::vfs::UserCred, _vp: NonNull<VNode>) {}

    fn close(&mut self, _f: u32, _c: super::vfs::UserCred, _vp: NonNull<VNode>) {}

    fn read(
        &mut self,
        count: usize,
        mut offset: usize,
        _f: u32,
        _c: super::vfs::UserCred,
        vp: NonNull<VNode>,
    ) -> Result<Arc<[u8]>, ()> {
        match self {
            File::Archive(archive) => {
                let fat_fs = unsafe { (*vp.as_ptr()).parent_vfs.as_mut().data.cast::<FatFs>() };

                let mut file: Vec<u8> = Vec::with_capacity(count);

                let mut cluster = ((archive.high_first_cluster_number as u32) << 16)
                    | archive.low_first_cluster_number as u32;

                let cluster_size = unsafe { (*fat_fs).cluster_size };

                let mut cluster_offset = offset / cluster_size;
                while cluster_offset > 0 {
                    cluster = unsafe { (*fat_fs).get_next_cluster(cluster as usize) };
                    cluster_offset -= 1;
                }

                let mut copied_bytes = 0;

                loop {
                    let cluster_data = unsafe { (*fat_fs).read_cluster(cluster as usize)? };

                    let remaining = count - copied_bytes;
                    let to_copy = if remaining > cluster_size {
                        cluster_size - offset
                    } else {
                        remaining
                    };

                    file.extend(cluster_data[offset..offset + to_copy].iter());

                    offset = 0;

                    copied_bytes += to_copy;

                    cluster = unsafe { (*fat_fs).get_next_cluster(cluster as usize) };

                    match unsafe { (*fat_fs).fat_type } {
                        FatType::Fat12(_) => {
                            if cluster >= EOC_12 {
                                break;
                            }
                        }
                        FatType::Fat16(_) => {
                            if cluster >= EOC_16 {
                                break;
                            }
                        }
                        FatType::Fat32(_) => {
                            if cluster >= EOC_32 {
                                break;
                            }
                        }
                    }
                }

                return Ok(Arc::from(file));
            }
            _ => panic!("Cannot open non archives"),
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
        todo!("VNODE OPERATIONS");
    }

    fn getattr(&mut self, _c: super::vfs::UserCred, _vp: NonNull<VNode>) -> super::vfs::VAttr {
        todo!("VNODE OPERATIONS");
    }

    fn setattr(&mut self, _va: super::vfs::VAttr, _c: super::vfs::UserCred, _vp: NonNull<VNode>) {
        todo!("VNODE OPERATIONS");
    }

    fn access(&mut self, _m: u32, _c: super::vfs::UserCred, _vp: NonNull<VNode>) {
        todo!("VNODE OPERATIONS");
    }

    fn lookup(
        &mut self,
        nm: &str,
        _c: super::vfs::UserCred,
        vp: NonNull<VNode>,
    ) -> Result<super::vfs::VNode, ()> {
        let fat_fs = unsafe { (*vp.as_ptr()).parent_vfs.as_mut().data.cast::<FatFs>() };

        match self {
            File::Dir(directory) => unsafe {
                let file_entry = (*fat_fs).find_entry_in_directory(*directory, nm)?;

                let file_typ = if file_entry.attributes == FileEntryAttributes::Directory as u8 {
                    crate::drivers::fs::vfs::VNodeType::Directory
                } else {
                    crate::drivers::fs::vfs::VNodeType::Regular
                };

                let file = if file_entry.attributes == FileEntryAttributes::Directory as u8 {
                    File::Dir(file_entry.cluster() as usize)
                } else {
                    File::Archive(file_entry)
                };

                let vnode = VNode::new(Box::new(file), file_typ, (*vp.as_ptr()).parent_vfs);

                Ok(vnode)
            },
            _ => panic!("tried to lookup on a file"),
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
        todo!("VNODE OPERATIONS");
    }

    fn link(
        &mut self,
        _target_dir: *mut super::vfs::VNode,
        _target_name: &str,
        _c: super::vfs::UserCred,
        _vp: NonNull<VNode>,
    ) {
        todo!("VNODE OPERATIONS");
    }

    fn rename(
        &mut self,
        _nm: &str,
        _target_dir: *mut super::vfs::VNode,
        _target_name: &str,
        _c: super::vfs::UserCred,
        _vp: NonNull<VNode>,
    ) {
        todo!("VNODE OPERATIONS");
    }

    fn mkdir(
        &mut self,
        _nm: &str,
        _va: super::vfs::VAttr,
        _c: super::vfs::UserCred,
        _vp: NonNull<VNode>,
    ) -> Result<super::vfs::VNode, ()> {
        todo!("VNODE OPERATIONS");
    }

    fn readdir(
        &mut self,
        _uiop: *const super::vfs::UIO,
        _c: super::vfs::UserCred,
        _vp: NonNull<VNode>,
    ) {
        todo!("VNODE OPERATIONS");
    }

    fn symlink(
        &mut self,
        _link_name: &str,
        _va: super::vfs::VAttr,
        _target_name: &str,
        _c: super::vfs::UserCred,
        _vp: NonNull<VNode>,
    ) {
        todo!("symlink not supported in FAT");
    }

    fn readlink(
        &mut self,
        _uiop: *const super::vfs::UIO,
        _c: super::vfs::UserCred,
        _vp: NonNull<VNode>,
    ) {
        todo!("VNODE OPERATIONS");
    }

    fn fsync(&mut self, _c: super::vfs::UserCred, _vp: NonNull<VNode>) {
        todo!("VNODE OPERATIONS");
    }

    fn len(&self, _vp: NonNull<VNode>) -> usize {
        match self {
            File::Archive(archive) => archive.file_size as usize,
            _ => panic!("idk"),
        }
    }
}
