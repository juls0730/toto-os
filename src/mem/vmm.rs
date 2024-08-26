use core::arch::x86_64::__cpuid;

use limine::memory_map::EntryType;

use crate::{
    hcf,
    libs::{
        cell::OnceCell,
        limine::{get_hhdm_offset, get_kernel_address, get_memmap, get_paging_level},
    },
};

use super::{align_down, align_up, pmm::pmm_alloc, PhysicalPtr};

const PT_FLAG_VALID: u64 = 1 << 0;
const PT_FLAG_WRITE: u64 = 1 << 1;
const PT_FLAG_USER: u64 = 1 << 2;
const PT_FLAG_LARGE: u64 = 1 << 7;
const PT_FLAG_NX: u64 = 1 << 63;
const PT_PADDR_MASK: u64 = 0x000F_FFFF_FFFF_FFFF;

const PT_TABLE_FLAGS: u64 = PT_FLAG_VALID | PT_FLAG_WRITE | PT_FLAG_USER;

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

    fn is_table(&self) -> bool {
        (self.0 & (PT_FLAG_VALID | PT_FLAG_LARGE)) == PT_FLAG_VALID
    }

    fn is_large(&self) -> bool {
        (self.0 & (PT_FLAG_VALID | PT_FLAG_LARGE)) == (PT_FLAG_VALID | PT_FLAG_LARGE)
    }

    fn vmm_flags(&self) -> u64 {
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
    entries: [PageTableEntry; 512],
}

impl PageDirectory {
    pub fn get_mut_ptr(&mut self) -> *mut Self {
        return core::ptr::addr_of_mut!(*self);
    }
}

pub static PAGE_SIZES: [u64; 5] = [0x1000, 0x200000, 0x40000000, 0x8000000000, 0x1000000000000];

pub static mut KENREL_PAGE_DIRECTORY: *mut PageDirectory = core::ptr::null_mut();

pub fn vmm_init() {
    let page_directory = unsafe {
        &mut *(pmm_alloc(1)
            .to_higher_half()
            .cast::<PageDirectory>()
            .as_raw_ptr())
    };

    unsafe { KENREL_PAGE_DIRECTORY = page_directory as *mut PageDirectory };

    let mut i = 0;
    for entry in get_memmap() {
        if entry.entry_type != EntryType::KERNEL_AND_MODULES {
            continue;
        }

        let kernel_addr = get_kernel_address();

        let base = kernel_addr.physical_base();
        let length = entry.length;
        let top = base + length;

        let aligned_base = align_down(base as usize, 0x40000000);
        let aligned_top = align_up(top as usize, 0x40000000);
        let aligned_length = aligned_top - aligned_base;

        while i <= aligned_length {
            let page = aligned_base + i;

            crate::println!(
                "Mapping the kernel from {:X} to {:X}",
                page,
                kernel_addr.virtual_base()
            );

            vmm_map(
                page_directory,
                page + kernel_addr.virtual_base() as usize,
                page as usize,
                0x02,
                PageSize::Size1GiB,
            );
            i += 0x40000000
        }
    }

    while i <= 0x100000000 {
        // vmm_map(page_directory, i, i, 0x03, PageSize::Size4KiB);
        vmm_map(
            page_directory,
            i + get_hhdm_offset(),
            i,
            0x02,
            PageSize::Size1GiB,
        );

        i += 0x40000000;
    }

    for entry in get_memmap() {
        if entry.entry_type == EntryType::RESERVED || entry.entry_type == EntryType::BAD_MEMORY {
            continue;
        }

        let mut base = entry.base;
        let length = entry.length;
        let top = base + length;

        if base < 0x100000000 {
            base = 0x100000000;
        }

        if base >= top {
            continue;
        }

        let aligned_base = align_down(base as usize, 0x40000000);
        let aligned_top = align_up(top as usize, 0x40000000);
        let aligned_length = aligned_top - aligned_base;

        i = 0;
        while i < aligned_length {
            let page = aligned_base + i;

            // vmm_map(
            //     page_directory,
            //     i + entry.base as usize,
            //     i + entry.base as usize,
            //     0x02,
            //     PageSize::Size4KiB,
            // );
            vmm_map(
                page_directory,
                page + get_hhdm_offset(),
                page,
                0x02,
                PageSize::Size1GiB,
            );

            i += 0x40000000;
        }
    }

    for entry in get_memmap() {
        if entry.entry_type != EntryType::FRAMEBUFFER {
            continue;
        }

        let base = entry.base;
        let length = entry.length;
        let top = base + length;

        let aligned_base = align_down(base as usize, 0x1000);
        let aligned_top = align_up(top as usize, 0x1000);
        let aligned_length = aligned_top - aligned_base;

        while i < aligned_length {
            let page = aligned_base + i;
            vmm_map(
                page_directory,
                page + get_hhdm_offset(),
                page,
                0x02 | 1 << 3,
                PageSize::Size4KiB,
            );

            i += 0x1000;
        }
    }

    unsafe { va_space_switch(page_directory) };
}

pub fn get_kernel_pdpt() -> &'static mut PageDirectory {
    return unsafe { &mut *KENREL_PAGE_DIRECTORY };
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

fn get_next_level(
    page_directory: &mut PageDirectory,
    virtual_addr: usize,
    desired_size: PageSize,
    level: usize,
    entry: usize,
) -> PhysicalPtr<PageDirectory> {
    let ret: PhysicalPtr<PageDirectory>;

    if page_directory.entries[entry].is_table() {
        ret = PhysicalPtr::from(page_directory.entries[entry].addr() as usize);
    } else {
        if page_directory.entries[entry].is_large() {
            // We are replacing an existing large page with a smaller page

            if (level >= 3) || (level == 0) {
                panic!("Unexpected level!");
            }
            if desired_size as usize >= 3 {
                panic!("Unexpected page size!");
            }

            let old_page_size = PAGE_SIZES[level];
            let new_page_size = PAGE_SIZES[desired_size as usize];

            crate::println!("OLD {old_page_size:X} NEW {new_page_size:X}");

            // ((x) & (PT_FLAG_WRITE | PT_FLAG_NX))
            let old_flags = page_directory.entries[entry].vmm_flags();
            let old_phys = page_directory.entries[entry].addr();
            let old_virt = virtual_addr as u64 & !(old_page_size - 1);

            if (old_phys & (old_page_size - 1)) != 0 {
                panic!(
                    "Unexpected page table entry address! {:X} {:X}",
                    old_phys, old_page_size
                );
            }

            ret = pmm_alloc(1).cast::<PageDirectory>();
            page_directory.entries[entry] = PageTableEntry::new(ret.addr() as u64, PT_TABLE_FLAGS);

            let mut i: usize = 0;
            while i < old_page_size as usize {
                vmm_map(
                    page_directory,
                    (old_virt as usize) + i,
                    (old_phys as usize) + i,
                    old_flags,
                    desired_size,
                );

                i += new_page_size as usize;
            }
        } else {
            ret = pmm_alloc(1).cast::<PageDirectory>();
            page_directory.entries[entry] = PageTableEntry::new(ret.addr() as u64, PT_TABLE_FLAGS);
        }
    }

    return ret;
}

static IS_1GIB_SUPPORTED: OnceCell<bool> = OnceCell::new();

fn is_1gib_page_supported() -> bool {
    if let Err(()) = IS_1GIB_SUPPORTED.get() {
        let cpuid = unsafe { __cpuid(0x80000001) };

        if (cpuid.edx & (1 << 26)) == (1 << 26) {
            IS_1GIB_SUPPORTED.set(true);
            crate::println!("1GiB is supported!");
        } else {
            IS_1GIB_SUPPORTED.set(false);
            crate::println!("1GiB is not supported!");
        }
    }

    return *IS_1GIB_SUPPORTED.get_unchecked();
}

/// Loads a new page directory and switched the Virtual Address Space
///
/// # Safety
///
/// If the memory space has not been remapped to the HHDM before switching, this will cause Undefined Behavior.
unsafe fn va_space_switch(page_directory: &mut PageDirectory) {
    let hhdm_offset = get_hhdm_offset();
    let kernel_virtual_base = get_kernel_address().virtual_base();

    // cast so we can do easy math
    let mut pd_ptr = page_directory.get_mut_ptr().cast::<u8>();

    if pd_ptr as usize > kernel_virtual_base as usize {
        pd_ptr = pd_ptr.sub(kernel_virtual_base as usize);
    } else if pd_ptr as usize > hhdm_offset {
        pd_ptr = pd_ptr.sub(hhdm_offset);
    }

    crate::println!("SWITCHING VA SPACE {pd_ptr:p}");
    crate::println!("HHDM_OFFSET: {hhdm_offset:#x}");
    crate::println!("KERNEL_VIRTUAL_BASE: {kernel_virtual_base:#x}");
    crate::println!("Page directory virtual address: {pd_ptr:p}");

    assert_eq!(
        pd_ptr as usize % 0x1000,
        0,
        "Page directory pointer is not aligned"
    );

    let mut cr3 = 0;
    unsafe { core::arch::asm!("mov rax, cr3", out("rax") cr3) };

    crate::println!("{cr3:X}");

    // hcf();

    unsafe { core::arch::asm!("mov cr3, {0:r}", in(reg) pd_ptr) };
    // test(pd_ptr);

    crate::println!("waa");
}

#[naked]
pub extern "C" fn test(ptr: *mut u8) {
    unsafe {
        core::arch::asm!("mov cr3, rdi", "ret", options(noreturn));
    }
}
