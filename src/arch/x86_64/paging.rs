use core::arch::x86_64::__cpuid;

use crate::{
    libs::{
        cell::OnceCell,
        limine::{get_hhdm_offset, get_kernel_address, get_paging_level},
    },
    mem::vmm::get_next_level,
    LogLevel,
};

const PT_FLAG_VALID: u64 = 1 << 0;
const PT_FLAG_WRITE: u64 = 1 << 1;
const PT_FLAG_USER: u64 = 1 << 2;
const PT_FLAG_LARGE: u64 = 1 << 7;
const PT_FLAG_NX: u64 = 1 << 63;
const PT_PADDR_MASK: u64 = 0x0000_FFFF_FFFF_F000;

pub const PT_TABLE_FLAGS: u64 = PT_FLAG_VALID | PT_FLAG_WRITE | PT_FLAG_USER;

// I know it's literally 8 bytes, but... fight me
#[derive(Clone)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub fn new(addr: u64, flags: u64) -> Self {
        Self(addr | flags)
    }

    pub fn addr(&self) -> u64 {
        self.0 & PT_PADDR_MASK
    }

    // TODO: probably a more elegant way to do this
    pub fn get_field(&self, field: Field) -> u64 {
        match field {
            Field::Present => (self.0 >> 0) & 1,
            Field::ReadWrite => (self.0 >> 1) & 1,
            Field::UserSupervisor => (self.0 >> 2) & 1,
            Field::WriteThrough => (self.0 >> 3) & 1,
            Field::CacheDisable => (self.0 >> 4) & 1,
            Field::Accessed => (self.0 >> 5) & 1,
            Field::Avl0 => (self.0 >> 6) & 1,
            Field::PageSize => (self.0 >> 7) & 1,
            Field::Avl1 => (self.0 >> 8) & 0xF,
            Field::Addr => (self.0 >> 12) & 0x000F_FFFF_FFFF_FFFF,
            Field::Nx => (self.0 >> 63) & 1,
        }
    }

    pub fn set_field(&mut self, field: Field, value: u64) {
        let mask = match field {
            Field::Present => 1 << 0,
            Field::ReadWrite => 1 << 1,
            Field::UserSupervisor => 1 << 2,
            Field::WriteThrough => 1 << 3,
            Field::CacheDisable => 1 << 4,
            Field::Accessed => 1 << 5,
            Field::Avl0 => 1 << 6,
            Field::PageSize => 1 << 7,
            Field::Avl1 => 0xF << 8,
            Field::Addr => 0x000F_FFFF_FFFF_FFFF << 12,
            Field::Nx => 1 << 63,
        };
        let shift = match field {
            Field::Present => 0,
            Field::ReadWrite => 1,
            Field::UserSupervisor => 2,
            Field::WriteThrough => 3,
            Field::CacheDisable => 4,
            Field::Accessed => 5,
            Field::Avl0 => 6,
            Field::PageSize => 7,
            Field::Avl1 => 8,
            Field::Addr => 12,
            Field::Nx => 63,
        };

        self.0 = (self.0 & !mask) | ((value << shift) & mask);
    }

    pub fn is_table(&self) -> bool {
        (self.0 & (PT_FLAG_VALID | PT_FLAG_LARGE)) == PT_FLAG_VALID
    }

    pub fn is_large(&self) -> bool {
        (self.0 & (PT_FLAG_VALID | PT_FLAG_LARGE)) == (PT_FLAG_VALID | PT_FLAG_LARGE)
    }

    pub fn vmm_flags(&self) -> u64 {
        self.0 & (PT_FLAG_WRITE | PT_FLAG_NX)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Field {
    Present,
    ReadWrite,
    UserSupervisor,
    WriteThrough,
    CacheDisable,
    Accessed,
    Avl0,
    PageSize,
    Avl1,
    Addr,
    Nx,
}

#[repr(align(0x1000))]
#[repr(C)]
pub struct PageDirectory {
    pub entries: [PageTableEntry; 512],
}

impl PageDirectory {
    pub fn get_mut_ptr(&mut self) -> *mut Self {
        return core::ptr::addr_of_mut!(*self);
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum PageSize {
    Size4KiB = 0,
    Size2MiB,
    Size1GiB,
}

pub fn vmm_map(
    page_directory: &mut PageDirectory,
    virtual_addr: usize,
    physical_addr: usize,
    mut flags: u64,
    page_size: PageSize,
) {
    let pml5_entry: usize = (virtual_addr & ((0x1ff as u64) << 48) as usize) >> 48;
    let pml4_entry: usize = (virtual_addr & ((0x1ff as u64) << 39) as usize) >> 39;
    let pml3_entry: usize = (virtual_addr & ((0x1ff as u64) << 30) as usize) >> 30;
    let pml2_entry: usize = (virtual_addr & ((0x1ff as u64) << 21) as usize) >> 21;
    let pml1_entry: usize = (virtual_addr & ((0x1ff as u64) << 12) as usize) >> 12;

    let (pml5, pml4, pml3, pml2, pml1): (
        &mut PageDirectory,
        &mut PageDirectory,
        &mut PageDirectory,
        &mut PageDirectory,
        &mut PageDirectory,
    );

    flags |= 0x01;

    match get_paging_level() {
        limine::paging::Mode::FIVE_LEVEL => {
            pml5 = page_directory;
            pml4 = unsafe {
                let ptr = get_next_level(pml5, virtual_addr, page_size, 4, pml5_entry);
                &mut *ptr.to_higher_half().as_raw_ptr()
            };
        }
        limine::paging::Mode::FOUR_LEVEL => {
            pml4 = page_directory;
        }
        _ => unreachable!(),
    }

    pml3 = unsafe {
        let ptr = get_next_level(pml4, virtual_addr, page_size, 3, pml4_entry);
        &mut *ptr.to_higher_half().as_raw_ptr()
    };

    if page_size == PageSize::Size1GiB {
        if is_1gib_page_supported() {
            pml3.entries[pml3_entry] = PageTableEntry(physical_addr as u64 | flags | PT_FLAG_LARGE);
        } else {
            let mut i = 0;
            while i < 0x40000000 {
                vmm_map(
                    page_directory,
                    virtual_addr + i,
                    physical_addr + i,
                    flags,
                    PageSize::Size2MiB,
                );

                i += 0x200000;
            }
        }

        return;
    }

    pml2 = unsafe {
        let ptr = get_next_level(pml3, virtual_addr, page_size, 2, pml3_entry);
        &mut *ptr.to_higher_half().as_raw_ptr()
    };

    if page_size == PageSize::Size2MiB {
        pml2.entries[pml2_entry] = PageTableEntry(physical_addr as u64 | flags | PT_FLAG_LARGE);
        return;
    }

    pml1 = unsafe {
        let ptr = get_next_level(pml2, virtual_addr, page_size, 1, pml2_entry);
        &mut *ptr.to_higher_half().as_raw_ptr()
    };

    if (flags & (1 << 12)) != 0 {
        flags &= !(1 << 12);
        flags |= 1 << 7;
    }

    pml1.entries[pml1_entry] = PageTableEntry(physical_addr as u64 | flags);
}

static IS_1GIB_SUPPORTED: OnceCell<bool> = OnceCell::new();

fn is_1gib_page_supported() -> bool {
    if let Err(()) = IS_1GIB_SUPPORTED.get() {
        let cpuid = unsafe { __cpuid(0x80000001) };

        if (cpuid.edx & (1 << 26)) == (1 << 26) {
            IS_1GIB_SUPPORTED.set(true);
            crate::log!(LogLevel::Debug, "1GiB pages are supported!");
        } else {
            IS_1GIB_SUPPORTED.set(false);
            crate::log!(LogLevel::Debug, "1GiB pages are not supported!");
        }
    }

    return *IS_1GIB_SUPPORTED.get_unchecked();
}

/// Loads a new page directory and switched the Virtual Address Space
///
/// # Safety
///
/// If the memory space has not been remapped to the HHDM before switching, this will cause Undefined Behavior.
pub unsafe fn va_space_switch(page_directory: &mut PageDirectory) {
    let hhdm_offset = get_hhdm_offset();
    let kernel_virtual_base = get_kernel_address().virtual_base();

    // cast so we can do easy math
    let mut pd_ptr = page_directory.get_mut_ptr().cast::<u8>();

    if pd_ptr as usize > kernel_virtual_base as usize {
        pd_ptr = pd_ptr.sub(kernel_virtual_base as usize);
    } else if pd_ptr as usize > hhdm_offset {
        pd_ptr = pd_ptr.sub(hhdm_offset);
    }

    unsafe { core::arch::asm!("mov cr3, {0:r}", in(reg) pd_ptr) };
}
