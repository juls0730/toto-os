pub mod allocator;
pub mod pmm;

use crate::libs::{cell::OnceCell, sync::Mutex};

use self::{allocator::LinkedListAllocator, pmm::PhysicalMemoryManager};

#[used]
#[link_section = ".requests"]
static mut MEMMAP_REQUEST: limine::request::MemoryMapRequest =
    limine::request::MemoryMapRequest::new();

#[used]
#[link_section = ".requests"]
static HHDM_REQUEST: limine::request::HhdmRequest = limine::request::HhdmRequest::new();
pub static HHDM_OFFSET: OnceCell<usize> = OnceCell::new();

pub static PHYSICAL_MEMORY_MANAGER: OnceCell<PhysicalMemoryManager> = OnceCell::new();

pub fn align_up(addr: usize, align: usize) -> usize {
    let offset = (addr as *const u8).align_offset(align);
    addr + offset
}

const HEAP_PAGES: usize = 1024; // 4 MiB heap

#[global_allocator]
pub static ALLOCATOR: Mutex<LinkedListAllocator> = Mutex::new(LinkedListAllocator::new());

// TODO: Limine-rs 0.2.0 does NOT have debug implemented for a lot of it's types, so until that is fixed, either go without Type, or hack limine-rs locally
// pub fn log_memory_map() {
//     let memmap_request = unsafe { MEMMAP_REQUEST.get_response_mut() };
//     if memmap_request.is_none() {
//         panic!("Memory map was None!");
//     }

//     let memmap = memmap_request.unwrap().entries();

//     crate::log_serial!("====== MEMORY MAP ======\n");
//     for entry in memmap.iter() {
//         let label = (entry.length as usize).label_bytes();

//         crate::log_serial!(
//             "[ {:#018X?} ] Type: {:?} Size: {}\n",
//             entry.base..entry.base + entry.length,
//             entry.entry_type,
//             label
//         )
//     }
// }

pub fn init_allocator() {
    let mut allocator_lock = ALLOCATOR.lock();
    allocator_lock.init(HEAP_PAGES);

    drop(allocator_lock);

    crate::println!(
        "{} of memory available",
        PHYSICAL_MEMORY_MANAGER.total_memory().label_bytes()
    );

    // log_memory_map();
}

pub enum Label {
    BYTE(usize),
    KIB(usize),
    MIB(usize),
    GIB(usize),
}

impl core::fmt::Display for Label {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Label::BYTE(count) => {
                write!(f, "{count} Byte(s)")
            }
            Label::KIB(count) => {
                write!(f, "{count} KiB(s)")
            }
            Label::MIB(count) => {
                write!(f, "{count} MiB(s)")
            }
            Label::GIB(count) => {
                write!(f, "{count} GiB(s)")
            }
        }
    }
}
pub trait LabelBytes {
    fn label_bytes(&self) -> Label;
}

impl LabelBytes for usize {
    fn label_bytes(&self) -> Label {
        let bytes = *self;

        if bytes >> 30 > 0 {
            return Label::GIB(bytes >> 30);
        } else if bytes >> 20 > 0 {
            return Label::MIB(bytes >> 20);
        } else if bytes >> 10 > 0 {
            return Label::KIB(bytes >> 10);
        } else {
            return Label::BYTE(bytes);
        }
    }
}

/// # Safety
/// This will produce undefined behavior if dst is not valid for count writes
pub unsafe fn memset32(dst: *mut u32, val: u32, count: usize) {
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        let mut buf = dst;
        unsafe {
            while buf < dst.add(count) {
                core::ptr::write_volatile(buf, val);
                buf = buf.offset(1);
            }
        }
        return;
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        core::arch::asm!(
            "rep stosd",
            inout("ecx") count => _,
            inout("edi") dst => _,
            inout("eax") val => _
        );
    }
}
