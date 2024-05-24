pub mod ide;

use alloc::{sync::Arc, vec::Vec};

use crate::libs::uuid::Uuid;

pub trait BlockDevice {
    fn sector_count(&self) -> u64;
    fn read(&self, sector: u64, sector_count: usize) -> Result<Arc<[u8]>, ()>;
    fn write(&self, sector: u64, data: &[u8]) -> Result<(), ()>;
}

#[derive(Clone, Copy, Debug)]
pub struct MBR {
    pub disk_id: [u8; 4],
    _reserved: [u8; 2],
    pub first_partition: [u8; 16],
    pub second_partition: [u8; 16],
    pub third_partition: [u8; 16],
    pub fourth_partition: [u8; 16],
    pub signature: [u8; 2],
}

impl From<&[u8]> for MBR {
    fn from(value: &[u8]) -> Self {
        let mut offset = 0;

        if value.len() >= 512 {
            offset = 440;
        }

        return Self {
            disk_id: value[offset..offset + 4].try_into().unwrap(),
            _reserved: value[offset + 4..offset + 6].try_into().unwrap(),
            first_partition: value[offset + 6..offset + 22].try_into().unwrap(),
            second_partition: value[offset + 22..offset + 38].try_into().unwrap(),
            third_partition: value[offset + 38..offset + 54].try_into().unwrap(),
            fourth_partition: value[offset + 54..offset + 70].try_into().unwrap(),
            signature: value[offset + 70..offset + 72].try_into().unwrap(),
        };
    }
}

impl MBR {
    pub fn partitions(&self) -> Arc<[MBRPartition]> {
        let mut partitions = Vec::new();

        let raw_partitions = [
            self.first_partition,
            self.second_partition,
            self.third_partition,
            self.fourth_partition,
        ];

        for partition in raw_partitions.iter() {
            // if partition bytes are empty
            if partition.iter().filter(|&&x| x > 0).count() == 0 {
                break;
            }

            let partition = MBRPartition {
                boot_indicator: partition[0],
                partition_start_chs: partition[1..4].try_into().unwrap(),
                partition_type: partition[4],
                partition_end_chs: partition[4..7].try_into().unwrap(),
                partition_start_lba: u32::from_le_bytes(partition[8..12].try_into().unwrap()),
                partition_sectors: u32::from_le_bytes(partition[12..16].try_into().unwrap()),
            };

            partitions.push(partition)
        }

        return Arc::from(partitions);
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Partition {
    MBRPartition((MBRPartition, *const dyn BlockDevice)),
    GPTPartition((GPTPartitionEntry, *const dyn BlockDevice)),
}

impl Partition {
    pub fn read(&self, sector: u64, sector_count: usize) -> Result<Arc<[u8]>, ()> {
        match self {
            Partition::GPTPartition((partition, block_device)) => {
                if partition.start_sector + sector + sector_count as u64 > partition.end_sector {
                    return Err(());
                }

                return unsafe {
                    (**block_device).read(partition.start_sector + sector, sector_count)
                };
            }
            Partition::MBRPartition((partition, block_device)) => {
                if partition.partition_start_lba as u64 + sector + sector_count as u64
                    > partition.partition_start_lba as u64 + partition.partition_sectors as u64
                {
                    return Err(());
                }

                return unsafe {
                    (**block_device)
                        .read(partition.partition_start_lba as u64 + sector, sector_count)
                };
            }
        }
    }

    pub fn write(&self, _sector: u64, _data: &[u8]) -> Result<(), ()> {
        todo!();
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MBRPartition {
    pub boot_indicator: u8,
    pub partition_start_chs: [u8; 3],
    pub partition_type: u8,
    pub partition_end_chs: [u8; 3],
    pub partition_start_lba: u32,
    pub partition_sectors: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct GPTPartitionEntry {
    pub partition_type_guid: Uuid,
    pub unique_partition_guid: Uuid,
    pub start_sector: u64,
    pub end_sector: u64,
    pub attributes: u64,
    pub partition_name: [u8; 72],
}

#[derive(Debug)]
pub struct GPTHeader {
    pub header: [u8; 8], // 0x45 0x46 0x49 0x20 0x50 0x41 0x52 0x54
    pub revision: u32,
    pub header_size: u32,
    pub header_checksum: u32, // CRC32
    _reserved: [u8; 4],
    pub header_lba: u64,
    pub header_lba_alt: u64,
    pub first_usable_block: u64,
    pub last_usable_block: u64,
    pub guid: Uuid,
    pub guid_lba: u64,
    pub partition_entry_count: u32,
    pub partition_entry_size: u32,
    pub partition_table_crc: u32,
}

impl GPTHeader {
    pub fn new(data: &[u8]) -> Self {
        assert!(data.len() >= 0x5C);

        let header = data[0x00..0x08].try_into().unwrap();
        let revision = u32::from_le_bytes(data[0x08..0x0C].try_into().unwrap());
        let header_size = u32::from_le_bytes(data[0x0C..0x10].try_into().unwrap());
        let header_checksum = u32::from_le_bytes(data[0x10..0x14].try_into().unwrap());
        let _reserved = data[0x14..0x18].try_into().unwrap();
        let header_lba = u64::from_le_bytes(data[0x18..0x20].try_into().unwrap());
        let header_lba_alt = u64::from_le_bytes(data[0x20..0x28].try_into().unwrap());
        let first_usable_block = u64::from_le_bytes(data[0x28..0x30].try_into().unwrap());
        let last_usable_block = u64::from_le_bytes(data[0x30..0x38].try_into().unwrap());
        let guid_bytes: [u8; 16] = data[0x38..0x48].try_into().unwrap();
        let guid = guid_bytes.into();
        let guid_lba = u64::from_le_bytes(data[0x48..0x50].try_into().unwrap());
        let partition_entry_count = u32::from_le_bytes(data[0x50..0x54].try_into().unwrap());
        let partition_entry_size = u32::from_le_bytes(data[0x54..0x58].try_into().unwrap());
        let partition_table_crc = u32::from_le_bytes(data[0x58..0x5C].try_into().unwrap());

        Self {
            header,
            revision,
            header_size,
            header_checksum,
            _reserved,
            header_lba,
            header_lba_alt,
            first_usable_block,
            last_usable_block,
            guid,
            guid_lba,
            partition_entry_count,
            partition_entry_size,
            partition_table_crc,
        }
    }
}
