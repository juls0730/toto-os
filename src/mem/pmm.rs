// Physical Memory Manager (pmm)

use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use crate::{
    libs::limine::{get_hhdm_offset, get_memmap},
    LogLevel,
};

use super::{PhysicalPtr, VirtualPtr, PAGE_SIZE};

#[derive(Debug)]
struct PhysicalMemoryManager {
    pub bitmap: AtomicPtr<u8>,
    pub highest_page_idx: AtomicUsize,
    pub last_used_page_idx: AtomicUsize,
    pub usable_pages: AtomicUsize,
    pub used_pages: AtomicUsize,
}

static mut PHYSICAL_MEMORY_MANAGER: PhysicalMemoryManager = PhysicalMemoryManager::new();

impl PhysicalMemoryManager {
    const fn new() -> Self {
        return Self {
            bitmap: AtomicPtr::new(core::ptr::null_mut()),
            highest_page_idx: AtomicUsize::new(0),
            last_used_page_idx: AtomicUsize::new(0),
            usable_pages: AtomicUsize::new(0),
            used_pages: AtomicUsize::new(0),
        };
    }

    #[inline(always)]
    fn bitmap_test(&self, bit: usize) -> bool {
        unsafe {
            let byte_index = bit / 8;
            let bit_index = bit % 8;
            return (*self.bitmap.load(Ordering::SeqCst).add(byte_index)) & (1 << bit_index) != 0;
        }
    }

    #[inline(always)]
    fn bitmap_set(&self, bit: usize) {
        unsafe {
            let byte_index = bit / 8;
            let bit_index = bit % 8;
            (*self.bitmap.load(Ordering::SeqCst).add(byte_index)) |= 1 << bit_index;
        }
    }

    #[inline(always)]
    fn bitmap_reset(&self, bit: usize) {
        unsafe {
            let byte_index = bit / 8;
            let bit_index = bit % 8;
            (*self.bitmap.load(Ordering::SeqCst).add(byte_index)) &= !(1 << bit_index);
        }
    }
}

pub fn pmm_init() {
    // we borrow the pointer because it is discouraged to make mutable reference to a mutable static and in Rust 2024 that will be a hard error
    let pmm = unsafe { &mut *core::ptr::addr_of_mut!(PHYSICAL_MEMORY_MANAGER) };

    let memmap = get_memmap();

    let mut highest_addr: usize = 0;

    for entry in memmap.iter() {
        if entry.entry_type == limine::memory_map::EntryType::USABLE {
            pmm.usable_pages
                .fetch_add(entry.length as usize / PAGE_SIZE, Ordering::SeqCst);
            if highest_addr < (entry.base + entry.length) as usize {
                highest_addr = (entry.base + entry.length) as usize;
            }
        }
    }

    pmm.highest_page_idx
        .store(highest_addr / PAGE_SIZE, Ordering::SeqCst);
    let bitmap_size =
        ((pmm.highest_page_idx.load(Ordering::SeqCst) / 8) + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

    for entry in memmap.iter_mut() {
        if entry.entry_type != limine::memory_map::EntryType::USABLE {
            continue;
        }

        if entry.length as usize >= bitmap_size {
            let ptr = VirtualPtr::from(entry.base as usize + get_hhdm_offset());
            pmm.bitmap.store(ptr.as_raw_ptr(), Ordering::SeqCst);

            unsafe {
                // Set the bit map to non-free
                ptr.write_bytes(0xFF, bitmap_size);
            };

            entry.length -= bitmap_size as u64;
            entry.base += bitmap_size as u64;

            break;
        }
    }

    for entry in memmap.iter() {
        if entry.entry_type != limine::memory_map::EntryType::USABLE {
            continue;
        }

        for i in 0..(entry.length as usize / PAGE_SIZE) {
            pmm.bitmap_reset((entry.base as usize + (i * PAGE_SIZE)) / PAGE_SIZE);
        }
    }
}

fn get_pmm<'a>() -> &'a mut PhysicalMemoryManager {
    return unsafe { &mut *core::ptr::addr_of_mut!(PHYSICAL_MEMORY_MANAGER) };
}

fn pmm_inner_alloc(pages: usize, limit: usize) -> PhysicalPtr<u8> {
    let pmm = get_pmm();
    let mut p: usize = 0;

    while pmm.last_used_page_idx.load(Ordering::SeqCst) < limit {
        if pmm.bitmap_test(pmm.last_used_page_idx.fetch_add(1, Ordering::SeqCst)) {
            p = 0;
            continue;
        }

        p += 1;
        if p == pages {
            let page = pmm.last_used_page_idx.load(Ordering::SeqCst) - pages;
            for i in page..pmm.last_used_page_idx.load(Ordering::SeqCst) {
                pmm.bitmap_set(i);
            }
            return PhysicalPtr::from(page * PAGE_SIZE);
        }
    }

    // We have hit the search limit, but did not find any suitable memory regions starting from last_used_page_idx
    crate::log!(LogLevel::Fatal, "Out Of Memory!");
    return PhysicalPtr::null_mut();
}

pub fn pmm_alloc_nozero(pages: usize) -> PhysicalPtr<u8> {
    let pmm = get_pmm();

    // Attempt to allocate n pages with a search limit of the amount of usable pages
    let mut page_addr = pmm_inner_alloc(pages, pmm.highest_page_idx.load(Ordering::SeqCst));

    if page_addr.is_null() {
        // If page_addr is null, then attempt to allocate n pages, but starting from
        // The beginning of the bitmap and with a limit of the old last_used_page_idx
        let last = pmm.last_used_page_idx.swap(0, Ordering::SeqCst);
        page_addr = pmm_inner_alloc(pages, last);

        // If page_addr is still null, we have ran out of usable memory
        if page_addr.is_null() {
            return PhysicalPtr::null_mut();
        }
    }

    pmm.used_pages.fetch_add(pages, Ordering::SeqCst);

    return page_addr;
}

pub fn pmm_alloc(pages: usize) -> PhysicalPtr<u8> {
    let ret = pmm_alloc_nozero(pages);

    if ret.is_null() {
        return ret;
    }

    unsafe {
        ret.to_higher_half().write_bytes(0x00, pages * PAGE_SIZE);
    };

    return ret;
}

pub fn pmm_dealloc(ptr: PhysicalPtr<u8>, pages: usize) {
    let pmm = get_pmm();
    let page = ptr.addr() as usize / PAGE_SIZE;

    for i in page..(page + pages) {
        pmm.bitmap_reset(i);
    }

    pmm.used_pages.fetch_sub(pages, Ordering::SeqCst);
}

pub fn total_memory() -> usize {
    let pmm = get_pmm();
    return pmm.usable_pages.load(Ordering::SeqCst) * 4096;
}

pub fn usable_memory() -> usize {
    let pmm = get_pmm();

    return (pmm.usable_pages.load(Ordering::SeqCst) * 4096)
        - (pmm.used_pages.load(Ordering::SeqCst) * 4096);
}

pub fn used_memory() -> usize {
    let pmm = get_pmm();

    return pmm.used_pages.load(Ordering::SeqCst) * 4096;
}
