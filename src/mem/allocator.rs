use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
};

use crate::{libs::sync::Mutex, mem::pmm::PAGE_SIZE};

use super::{align_up, HHDM_OFFSET};

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
            self.add_free_region(
                super::PHYSICAL_MEMORY_MANAGER
                    .alloc(pages)
                    .add(*HHDM_OFFSET),
                PAGE_SIZE * pages,
            );
        }
    }

    unsafe fn add_free_region(&mut self, addr: *mut u8, size: usize) {
        assert_eq!(
            align_up(addr as usize, core::mem::align_of::<MemNode>()),
            addr as usize
        );
        assert!(size >= core::mem::size_of::<MemNode>());

        let mut node = MemNode::new(size);
        node.next = self.head.next.take();

        addr.cast::<MemNode>().write(node);
        self.head.next = Some(NonNull::new_unchecked(addr.cast::<MemNode>()));
    }

    fn alloc_from_node(node: &MemNode, layout: Layout) -> *mut u8 {
        let start = align_up(node.addr(), layout.align());
        let end = start + layout.size();

        if end > node.end_addr() {
            // aligned address goes outside the bounds of the node
            return core::ptr::null_mut();
        }

        let extra = node.end_addr() - end;
        if extra > 0 && extra < core::mem::size_of::<MemNode>() {
            // Node size minus allocation size is less than the minimum size needed for a node,
            // thus, if we let the allocation to happen in this node, we lose track of the extra memory
            // lost by this allocation
            return core::ptr::null_mut();
        }

        return start as *mut u8;
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

    fn size_align(layout: Layout) -> Layout {
        let layout = layout
            .align_to(core::mem::align_of::<MemNode>())
            .expect("Failed to align allocation")
            .pad_to_align();

        let size = layout.size().max(core::mem::size_of::<MemNode>());
        return Layout::from_size_align(size, layout.align()).expect("Failed to create layout");
    }

    unsafe fn inner_alloc(&mut self, layout: Layout) -> *mut u8 {
        let layout = Self::size_align(layout);

        if let Some(region) = self.find_region(layout) {
            // immutable pointers are a government conspiracy anyways
            let end = (region.as_ref().addr() + layout.size()) as *mut u8;
            let extra = region.as_ref().end_addr() - end as usize;

            if extra > 0 {
                self.add_free_region(end, extra)
            }

            return region.as_ref().addr() as *mut u8;
        }

        return core::ptr::null_mut();
    }

    unsafe fn inner_dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let layout = Self::size_align(layout);

        self.add_free_region(ptr, layout.size());
    }
}

unsafe impl GlobalAlloc for Mutex<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut allocator = self.lock();

        allocator.inner_alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.lock();

        allocator.inner_dealloc(ptr, layout);
    }
}
