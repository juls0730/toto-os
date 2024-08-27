use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
};

use crate::libs::sync::Mutex;

use super::{align_up, pmm::pmm_alloc, VirtualPtr, PAGE_SIZE};

#[derive(Debug)]
struct MemNode {
    next: Option<NonNull<Self>>,
    size: usize,
}

impl MemNode {
    const fn new(size: usize) -> Self {
        Self { next: None, size }
    }

    pub fn addr(&self) -> usize {
        self as *const Self as usize
    }

    pub fn end_addr(&self) -> usize {
        self.addr() + self.len()
    }

    pub fn len(&self) -> usize {
        self.size
    }
}

pub struct LinkedListAllocator {
    head: MemNode,
}

unsafe impl Sync for LinkedListAllocator {}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        Self {
            head: MemNode::new(0),
        }
    }

    pub fn init(&mut self, pages: usize) {
        unsafe {
            // fucking kill me
            self.add_free_region(pmm_alloc(pages).to_higher_half(), PAGE_SIZE * pages);
        }
    }

    unsafe fn add_free_region(&mut self, ptr: VirtualPtr<u8>, size: usize) {
        assert_eq!(
            align_up(ptr.addr(), core::mem::align_of::<MemNode>()),
            ptr.addr()
        );
        assert!(size >= core::mem::size_of::<MemNode>());

        let mut target_node = &mut self.head;

        while let Some(mut next_node) = target_node.next {
            if next_node.as_ref().addr() > ptr.addr() {
                break;
            }

            target_node = next_node.as_mut()
        }

        let mut node = MemNode::new(size);
        node.next = target_node.next.take();

        ptr.cast::<MemNode>().write(node);
        target_node.next = Some(NonNull::new_unchecked(ptr.cast::<MemNode>().as_raw_ptr()));
    }

    unsafe fn coalesce_memory(&mut self) {
        let mut current_node = &mut self.head;

        while let Some(mut next) = current_node.next {
            let next = next.as_mut();

            if current_node.end_addr() == next.addr() {
                let new_size = current_node.size + next.size;

                current_node.size = new_size;
                current_node.next = next.next.take();
            } else {
                current_node = next;
            }
        }
    }

    fn alloc_from_node(node: &MemNode, layout: Layout) -> VirtualPtr<u8> {
        let start = align_up(node.addr(), layout.align());
        let end = start + layout.size();

        if end > node.end_addr() {
            // aligned address goes outside the bounds of the node
            return VirtualPtr::null_mut();
        }

        let extra = node.end_addr() - end;
        if extra > 0 && extra < core::mem::size_of::<MemNode>() {
            // Node size minus allocation size is less than the minimum size needed for a node,
            // thus, if we let the allocation to happen in this node, we lose track of the extra memory
            // lost by this allocation
            return VirtualPtr::null_mut();
        }

        return VirtualPtr::from(start);
    }

    unsafe fn find_region(&mut self, layout: Layout) -> Option<NonNull<MemNode>> {
        let mut current_node = &mut self.head;

        while let Some(node) = current_node.next.as_mut() {
            let node = node.as_mut();

            if Self::alloc_from_node(node, layout).is_null() {
                current_node = current_node.next.as_mut().unwrap().as_mut();
                continue;
            }

            // `node` is suitable for this allocation
            let next = node.next.take();
            let ret = Some(current_node.next.take().unwrap());
            current_node.next = next;
            return ret;
        }

        return None;
    }

    pub fn count_reginos(&self) -> usize {
        let mut region_count = 0;
        let mut cur_region = &self.head;

        while let Some(next) = cur_region.next {
            cur_region = unsafe { next.as_ref() };
            region_count += 1;
        }

        region_count
    }

    pub fn debug_regions(&self, buf: &mut [(usize, usize)]) {
        let mut i = 0;
        let mut cur_region = &self.head;
        buf[i] = (cur_region.addr(), cur_region.end_addr());
        i += 1;

        while let Some(next) = cur_region.next {
            cur_region = unsafe { next.as_ref() };
            buf[i] = (cur_region.addr(), cur_region.end_addr());
            i += 1;
        }
    }

    fn size_align(layout: Layout) -> Layout {
        let layout = layout
            .align_to(core::mem::align_of::<MemNode>())
            .expect("Failed to align allocation")
            .pad_to_align();

        let size = layout.size().max(core::mem::size_of::<MemNode>());
        return Layout::from_size_align(size, layout.align()).expect("Failed to create layout");
    }

    unsafe fn inner_alloc(&mut self, layout: Layout) -> VirtualPtr<u8> {
        let layout = Self::size_align(layout);

        if let Some(region) = self.find_region(layout) {
            // immutable pointers are a government conspiracy anyways
            let end = VirtualPtr::from(region.as_ref().addr() + layout.size());
            let extra = region.as_ref().end_addr() - end.addr();

            if extra > 0 {
                self.add_free_region(end, extra)
            }

            return VirtualPtr::from(region.as_ref().addr());
        }

        return VirtualPtr::null_mut();
    }

    unsafe fn inner_dealloc(&mut self, ptr: VirtualPtr<u8>, layout: Layout) {
        let layout = Self::size_align(layout);

        self.add_free_region(ptr, layout.size());
        self.coalesce_memory();
    }
}

unsafe impl GlobalAlloc for Mutex<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut allocator = self.lock();

        allocator.inner_alloc(layout).as_raw_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.lock();

        allocator.inner_dealloc(VirtualPtr::new(ptr), layout);
    }
}
