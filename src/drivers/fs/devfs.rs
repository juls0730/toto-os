use alloc::{boxed::Box, string::String, sync::Arc};

#[repr(u8)]
pub enum DeviceType {
    CharacterDevice = 0,
    BlockDevice = 1,
}

#[allow(unused)]
pub struct Device {
    typ: DeviceType,
    block_size: usize,
    name: String,
    ops: Box<dyn DeviceOperations>,
}

pub trait DeviceOperations {
    fn read(&self, sector: u64, sector_count: usize) -> Result<Arc<[u8]>, ()>;
    fn write(&self, sector: u64, data: &[u8]) -> Result<(), ()>;
}
