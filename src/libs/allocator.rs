use core::{
    sync::atomic::{
        AtomicUsize,
        Ordering::{Acquire, SeqCst},
    },
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    ptr
};

// ! Using a basic bump allocator, switch to something like a buddy allocator soon :tm:
const ARENA_SIZE: usize = 128 * 1024;
const MAX_SUPPORTED_ALIGN: usize = 4096;

#[repr(C, align(4096))]
pub struct SimpleAllocator {
    arena: UnsafeCell<[u8; ARENA_SIZE]>,
    pub remaining: AtomicUsize,
}

#[global_allocator]
pub static ALLOCATOR: SimpleAllocator = SimpleAllocator {
    arena: UnsafeCell::new([0x55; ARENA_SIZE]),
    remaining: AtomicUsize::new(ARENA_SIZE),
};

impl SimpleAllocator {
    pub fn get_used(&self) -> usize {
        let currently = self.remaining.load(Acquire);
        return ARENA_SIZE - currently;
    }

    pub fn get_free(&self) -> usize {
        return self.remaining.load(Acquire);
    }
}

unsafe impl Sync for SimpleAllocator {}

unsafe impl GlobalAlloc for SimpleAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        // `Layout` contract forbids making a `Layout` with align=0, or align not power of 2.
        // So we can safely use a mask to ensure alignment without worrying about UB.
        let align_mask_to_round_down = !(align - 1);

        if align > MAX_SUPPORTED_ALIGN {
            return ptr::null_mut();
        }

        let mut allocated = 0;
        if self
            .remaining
            .fetch_update(SeqCst, SeqCst, |mut remaining| {
                if size > remaining {
                    return None;
                }
                remaining -= size;
                remaining &= align_mask_to_round_down;
                allocated = remaining;
                Some(remaining)
            })
            .is_err()
        {
            return ptr::null_mut();
        };
        self.arena.get().cast::<u8>().add(allocated)
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}
