// Physical Memory Manager (pmm)

use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use super::{HHDM_REQUEST, MEMMAP_REQUEST};

pub const PAGE_SIZE: usize = 4096;

#[derive(Debug)]
pub struct PhysicalMemoryManager {
    bitmap: AtomicPtr<u8>,
    highest_page_idx: AtomicUsize,
    last_used_page_idx: AtomicUsize,
    usable_pages: AtomicUsize,
    used_pages: AtomicUsize,
}

pub fn pmm_init() {
    super::PHYSICAL_MEMORY_MANAGER.set(PhysicalMemoryManager::new());
}

impl PhysicalMemoryManager {
    pub fn new() -> Self {
        let pmm = Self {
            bitmap: AtomicPtr::new(core::ptr::null_mut()),
            highest_page_idx: AtomicUsize::new(0),
            last_used_page_idx: AtomicUsize::new(0),
            usable_pages: AtomicUsize::new(0),
            used_pages: AtomicUsize::new(0),
        };

        let hhdm_req = HHDM_REQUEST
            .get_response()
            .get()
            .expect("Failed to get Higher Half Direct Map!");

        let hhdm_offset = hhdm_req.offset as usize;

        let memmap = MEMMAP_REQUEST
            .get_response()
            .get_mut()
            .expect("Failed to get Memory map!")
            .memmap_mut();

        let mut highest_addr: usize = 0;

        for entry in memmap.iter() {
            if entry.typ == limine::MemoryMapEntryType::Usable {
                pmm.usable_pages
                    .fetch_add(entry.len as usize / PAGE_SIZE, Ordering::SeqCst);
                if highest_addr < (entry.base + entry.len) as usize {
                    highest_addr = (entry.base + entry.len) as usize;
                }
            }
        }

        pmm.highest_page_idx
            .store(highest_addr / PAGE_SIZE, Ordering::SeqCst);
        let bitmap_size =
            ((pmm.highest_page_idx.load(Ordering::SeqCst) / 8) + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);

        for entry in memmap.iter_mut() {
            if entry.typ != limine::MemoryMapEntryType::Usable {
                continue;
            }

            if entry.len as usize >= bitmap_size {
                let ptr = (entry.base as usize + hhdm_offset) as *mut u8;
                pmm.bitmap.store(ptr, Ordering::SeqCst);

                unsafe {
                    // Set the bit map to non-free
                    core::ptr::write_bytes(ptr, 0xFF, bitmap_size);
                };

                entry.len -= bitmap_size as u64;
                entry.base += bitmap_size as u64;

                break;
            }
        }

        for entry in memmap.iter() {
            if entry.typ != limine::MemoryMapEntryType::Usable {
                continue;
            }

            for i in 0..(entry.len as usize / PAGE_SIZE) {
                pmm.bitmap_reset((entry.base as usize + (i * PAGE_SIZE)) / PAGE_SIZE);
            }
        }

        return pmm;
    }

    fn inner_alloc(&self, pages: usize, limit: usize) -> *mut u8 {
        let mut p: usize = 0;

        while self.last_used_page_idx.load(Ordering::SeqCst) < limit {
            if self.bitmap_test(self.last_used_page_idx.fetch_add(1, Ordering::SeqCst)) {
                p = 0;
                continue;
            }

            p += 1;
            if p == pages {
                let page = self.last_used_page_idx.load(Ordering::SeqCst) - pages;
                for i in page..self.last_used_page_idx.load(Ordering::SeqCst) {
                    self.bitmap_set(i);
                }
                return (page * PAGE_SIZE) as *mut u8;
            }
        }

        // We have hit the search limit, but did not find any suitable memory regions starting from last_used_page_idx
        return core::ptr::null_mut();
    }

    pub fn alloc_nozero(&self, pages: usize) -> *mut u8 {
        // Attempt to allocate n pages with a search limit of the amount of usable pages
        let mut page_addr = self.inner_alloc(pages, self.highest_page_idx.load(Ordering::SeqCst));

        if page_addr.is_null() {
            // If page_addr is null, then attempt to allocate n pages, but starting from
            // The beginning of the bitmap and with a limit of the old last_used_page_idx
            let last = self.last_used_page_idx.swap(0, Ordering::SeqCst);
            page_addr = self.inner_alloc(pages, last);

            // If page_addr is still null, we have ran out of usable memory
            if page_addr.is_null() {
                return core::ptr::null_mut();
            }
        }

        self.used_pages.fetch_add(pages, Ordering::SeqCst);

        return page_addr;
    }

    pub fn alloc(&self, pages: usize) -> *mut u8 {
        let ret = self.alloc_nozero(pages);

        if ret.is_null() {
            return ret;
        }

        unsafe {
            core::ptr::write_bytes(ret, 0x00, pages * PAGE_SIZE);
        };

        return ret;
    }

    pub fn dealloc(&self, addr: *mut u8, pages: usize) {
        let page = addr as usize / PAGE_SIZE;

        for i in page..(page + pages) {
            self.bitmap_reset(i);
        }

        self.used_pages.fetch_sub(pages, Ordering::SeqCst);
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

    pub fn total_memory(&self) -> usize {
        return self.usable_pages.load(Ordering::SeqCst) * 4096;
    }

    pub fn usable_memory(&self) -> usize {
        return (self.usable_pages.load(Ordering::SeqCst) * 4096)
            - (self.used_pages.load(Ordering::SeqCst) * 4096);
    }

    pub fn used_memory(&self) -> usize {
        return self.used_pages.load(Ordering::SeqCst) * 4096;
    }
}
