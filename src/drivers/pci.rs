use alloc::vec::Vec;

use crate::{
    arch::io::{inl, outl},
    libs::sync::Mutex,
    LogLevel,
};

const PCI_CONFIG_PORT: u16 = 0xCF8; // The base I/O port for PCI configuration access
const PCI_DATA_PORT: u16 = 0xCFC; // The data port for reading/writing configuration data

fn read_pci_config(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    let mut address: u32 = 0;
    address |= 1 << 31; // Enable bit
    address |= (bus as u32) << 16; // Set Bus Number
    address |= (device as u32) << 11; // Set Device Number
    address |= (func as u32) << 8; // Set Function number
    address |= (offset & 0xFC) as u32; // Set Register offset

    // Write the address to the PCI_CONFIG_PORT
    outl(PCI_CONFIG_PORT, address);

    // Read the data from the PCI_DATA_PORT
    let data = inl(PCI_DATA_PORT) >> ((offset & 2) * 8);

    return data;
}

#[inline]
fn read_pci_vendor_id(bus: u8, device: u8, func: u8) -> u16 {
    return (read_pci_config(bus, device, func, 0x00) & 0xFFFF) as u16;
}

#[inline]
fn read_pci_device_id(bus: u8, device: u8, func: u8) -> u16 {
    return ((read_pci_config(bus, device, func, 0x00) >> 16) & 0xFFFF) as u16;
}

#[inline]
fn read_pci_class_code(bus: u8, device: u8, func: u8) -> u8 {
    return ((read_pci_config(bus, device, func, 0x08) >> 24) & 0xFF) as u8;
}

#[inline]
fn read_pci_subclass_code(bus: u8, device: u8, func: u8) -> u8 {
    return ((read_pci_config(bus, device, func, 0x08) >> 16) & 0xFF) as u8;
}

#[inline]
fn read_pci_prog_if(bus: u8, device: u8, func: u8) -> u8 {
    // Read the Prog IF (Programming Interface) from the PCI configuration space
    return ((read_pci_config(bus, device, func, 0x08) >> 8) & 0xFF) as u8;
}

#[inline]
fn read_pci_revision_id(bus: u8, device: u8, func: u8) -> u8 {
    return (read_pci_config(bus, device, func, 0x08) & 0xFF) as u8;
}

#[inline]
fn read_pci_header_type(bus: u8, device: u8, func: u8) -> u8 {
    return ((read_pci_config(bus, device, func, 0x0C) >> 16) & 0xFF) as u8;
}

#[inline]
fn read_pci_to_pci_secondary_bus(bus: u8, device: u8, func: u8) -> u8 {
    return (read_pci_config(bus, device, func, 0x10) & 0xFF) as u8;
}

pub fn _get_pci_bar_addresses(bus: u8, device: u8, func: u8) -> (u32, u32, u32, u32, u32, u32) {
    let bar0 = read_pci_config(bus, device, func, 0x10);
    let bar1 = read_pci_config(bus, device, func, 0x14);
    let bar2 = read_pci_config(bus, device, func, 0x18);
    let bar3 = read_pci_config(bus, device, func, 0x1C);
    let bar4 = read_pci_config(bus, device, func, 0x20);
    let bar5 = read_pci_config(bus, device, func, 0x24);

    (bar0, bar1, bar2, bar3, bar4, bar5)
}

#[derive(Debug)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub func: u8,
    // __reserved: u8
    pub device_id: u16,
    pub vendor_id: u16,
    pub class_code: u8,
    pub subclass_code: u8,
    pub prog_if: u8,
    pub revision_id: u8,
}

impl PciDevice {
    fn new(bus: u8, device: u8, func: u8) -> Self {
        // Read the Vendor ID and Device ID registers for each func
        let vendor_id = read_pci_vendor_id(bus, device, func);

        let device_id = read_pci_device_id(bus, device, func);
        let class_code = read_pci_class_code(bus, device, func);
        let subclass_code = read_pci_subclass_code(bus, device, func);
        let prog_if = read_pci_prog_if(bus, device, func);
        let revision_id = read_pci_revision_id(bus, device, func);

        return Self {
            bus,
            device,
            func,
            device_id,
            vendor_id,
            class_code,
            subclass_code,
            prog_if,
            revision_id,
        };
    }
}

impl core::fmt::Display for PciDevice {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        return write!(f, "Bus: {} Device: {} Function: {} VendorID: {:#X} DeviceID: {:#X} ClassCode: {:#04X} SubclassCode: {:#04X} ProgIF: {:#04X}",
        self.bus, self.device, self.func, self.vendor_id, self.device_id, self.class_code, self.subclass_code, self.prog_if);
    }
}

pub static PCI_DEVICES: Mutex<Vec<PciDevice>> = Mutex::new(Vec::new());

pub fn enumerate_pci_bus() {
    for bus in 0..=255 {
        check_bus(bus);
    }

    crate::log!(LogLevel::Debug, "====== PCI DEVICES ======");
    for (i, pci_device) in PCI_DEVICES.lock().iter().enumerate() {
        crate::log!(LogLevel::Debug, "Entry {i:2}: {pci_device}")
    }
}

fn check_bus(bus: u8) {
    for device in 0..32 {
        check_device(bus, device);
    }
}

fn check_device(bus: u8, device: u8) {
    let func: u8 = 0;

    let vendor_id = read_pci_vendor_id(bus, device, func);

    if vendor_id == 0xFFFF {
        return;
    }

    check_function(bus, device, func);
    let header_type = read_pci_header_type(bus, device, func);

    if header_type & 0x80 != 0 {
        for func in 1..8 {
            let vendor_id = read_pci_vendor_id(bus, device, func);

            if vendor_id != 0xFFFF {
                check_function(bus, device, func);
            }
        }
    }
}

fn check_function(bus: u8, device: u8, func: u8) {
    PCI_DEVICES.lock().push(PciDevice::new(bus, device, func));

    let _secondary_bus: u8;

    let class_code = read_pci_class_code(bus, device, func);
    let subclass_code = read_pci_subclass_code(bus, device, func);

    if class_code == 0x06 && subclass_code == 0x04 {
        _secondary_bus = read_pci_to_pci_secondary_bus(bus, device, func);
        // TODO: This causes an infinite loop on baremetal
        // check_bus(secondary_bus);
    }
}
