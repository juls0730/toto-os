use limine::memory_map::EntryType;

use crate::{
    arch::paging::{
        va_space_switch, vmm_map, PageDirectory, PageSize, PageTableEntry, PT_TABLE_FLAGS,
    },
    libs::limine::{get_hhdm_offset, get_kernel_address, get_memmap},
};

use super::{align_down, align_up, pmm::pmm_alloc, PhysicalPtr};

const VMM_FLAG_WRITE: u64 = 1 << 1;
const VMM_FLAG_NOEXEC: u64 = 1 << 63;
const VMM_FLAG_FB: u64 = 1 << 3 | 1 << 12;

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

    let mut i = 0_usize;
    while i < 0x100000000 {
        vmm_map(
            page_directory,
            i + get_hhdm_offset(),
            i,
            VMM_FLAG_WRITE,
            PageSize::Size1GiB,
        );

        i += 0x40000000;
    }

    for entry in get_memmap() {
        if entry.entry_type != EntryType::KERNEL_AND_MODULES {
            continue;
        }

        let kernel_addr = get_kernel_address();

        let base = kernel_addr.physical_base() as usize;
        let length = entry.length as usize;

        crate::println!("{length:X} {base:X} {:X}", entry.base);

        i = 0;
        while i < length {
            vmm_map(
                page_directory,
                kernel_addr.virtual_base() as usize + i,
                base + i,
                0x02,
                PageSize::Size4KiB,
            );
            i += 0x1000;
        }
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

    unsafe { va_space_switch(page_directory) };
}

pub fn get_kernel_pdpt() -> &'static mut PageDirectory {
    return unsafe { &mut *KENREL_PAGE_DIRECTORY };
}

pub fn get_next_level(
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

            assert!(level <= 3 && level != 0, "Unexpected level!");

            let old_page_size = PAGE_SIZES[level];
            let new_page_size = PAGE_SIZES[desired_size as usize];

            // ((x) & (PT_FLAG_WRITE | PT_FLAG_NX))
            let old_flags = page_directory.entries[entry].vmm_flags();
            let old_phys = page_directory.entries[entry].addr();
            let old_virt = virtual_addr as u64 & !(old_page_size - 1);

            assert_eq!(
                old_phys & (old_page_size - 1),
                0,
                "Unexpected page table entry address!"
            );

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
