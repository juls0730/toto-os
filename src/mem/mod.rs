pub mod allocator;
pub mod pmm;
pub mod vmm;

use core::fmt::{write, Debug, Pointer};

use crate::libs::{limine::get_hhdm_offset, sync::Mutex};

use self::allocator::LinkedListAllocator;

/// A PhysicalPtr is a pointer that uses a physical location in memory. These pointers are not readable or mutable as we cannot gurantee we are viewing the correct section of memory or that this memory is mapped to that location at all, resulting in a Page Fault.
// TODO: make this use only a usize or something instead of a ptr
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PhysicalPtr<T> {
    inner: *mut T,
}

impl<T> PhysicalPtr<T> {
    pub const fn new(ptr: *mut T) -> Self {
        return Self { inner: ptr };
    }

    pub const fn null_mut() -> Self {
        return Self {
            inner: core::ptr::null_mut(),
        };
    }

    pub fn as_raw_ptr(&self) -> *mut T {
        return self.inner;
    }

    pub fn addr(&self) -> usize {
        return self.inner as usize;
    }

    pub fn is_null(&self) -> bool {
        return self.inner.is_null();
    }

    /// # Safety:
    /// see core::ptr::mut_ptr.add()
    pub unsafe fn add(&self, count: usize) -> Self {
        return Self::new(self.inner.add(count));
    }

    /// # Safety:
    /// see core::ptr::mut_ptr.sub()
    pub unsafe fn sub(&self, count: usize) -> Self {
        return Self::new(self.inner.sub(count));
    }

    /// # Safety:
    /// see core::ptr::mut_ptr.offset()
    pub unsafe fn offset(&self, count: isize) -> Self {
        return Self::new(self.inner.offset(count));
    }

    pub const fn cast<U>(&self) -> PhysicalPtr<U> {
        return PhysicalPtr::new(self.inner.cast::<U>());
    }

    // torn if this should be unsafe or not
    pub fn to_higher_half(&self) -> VirtualPtr<T> {
        return unsafe {
            VirtualPtr::new(self.cast::<u8>().add(get_hhdm_offset()).inner.cast::<T>())
        };
    }
}

impl<T> From<usize> for PhysicalPtr<T> {
    fn from(addr: usize) -> Self {
        PhysicalPtr {
            inner: addr as *mut T,
        }
    }
}

impl<T> From<*mut T> for PhysicalPtr<T> {
    fn from(ptr: *mut T) -> Self {
        PhysicalPtr { inner: ptr }
    }
}

// constant pointers are a lie anyways tbh
impl<T> From<*const T> for PhysicalPtr<T> {
    fn from(ptr: *const T) -> Self {
        PhysicalPtr {
            inner: ptr as *mut T,
        }
    }
}

impl<T> Debug for PhysicalPtr<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write(f, format_args!("PhysicalPtr({:p})", self.inner))
    }
}

impl<T> Pointer for PhysicalPtr<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write(f, format_args!("PhysicalPtr({:p})", self.inner))
    }
}

/// A Virtual Pointer is a pointer that uses a virtual address. These pointers are readable and mutable as they map to a physical address through paging.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct VirtualPtr<T> {
    inner: *mut T,
}

impl<T> VirtualPtr<T> {
    pub const fn new(ptr: *mut T) -> Self {
        return Self { inner: ptr };
    }

    pub const fn null_mut() -> Self {
        return Self {
            inner: core::ptr::null_mut(),
        };
    }

    pub fn addr(&self) -> usize {
        return self.inner as usize;
    }

    pub fn as_raw_ptr(&self) -> *mut T {
        return self.inner;
    }

    pub fn is_null(&self) -> bool {
        return self.inner.is_null();
    }

    /// # Safety:
    /// see core::ptr::mut_ptr.add()
    pub unsafe fn add(&self, count: usize) -> Self {
        return Self::new(self.inner.add(count));
    }

    /// # Safety:
    /// see core::ptr::mut_ptr.sub()
    pub unsafe fn sub(&self, count: usize) -> Self {
        return Self::new(self.inner.sub(count));
    }

    /// # Safety:
    /// see core::ptr::mut_ptr.offset()
    pub unsafe fn offset(&self, count: isize) -> Self {
        return Self::new(self.inner.offset(count));
    }

    pub const fn cast<U>(&self) -> VirtualPtr<U> {
        return VirtualPtr::new(self.inner.cast::<U>());
    }

    /// # Safety:
    /// see core::ptr::mut_ptr.write_bytes()
    pub unsafe fn write_bytes(&self, val: u8, count: usize) {
        self.inner.write_bytes(val, count);
    }

    /// # Safety:
    /// Ensure the pointer is in the higher half
    pub unsafe fn to_lower_half(&self) -> PhysicalPtr<T> {
        return unsafe {
            // be very careful with the math here
            PhysicalPtr::new(self.cast::<u8>().sub(get_hhdm_offset()).inner.cast::<T>())
        };
    }

    /// # Safety:
    /// Ensure that the pointer is a valid virtual pointer and follows the same rules as ptr::read
    pub const unsafe fn read(&self) -> T {
        return self.inner.read();
    }

    /// # Safety:
    /// Ensure that the pointer is a valid virtual pointer and follows the same rules as ptr::read_unaligned
    pub const unsafe fn read_unaligned(&self) -> T {
        return self.inner.read_unaligned();
    }

    /// # Safety:
    /// Ensure that the pointer is a valid virtual pointer and follows the same rules as ptr::read_unaligned
    pub unsafe fn read_volatile(&self) -> T {
        return self.inner.read_volatile();
    }

    /// # Safety:
    /// Ensure that the pointer is a valid virtual pointer and follows the same rules as ptr::write
    pub unsafe fn write(&self, val: T) {
        self.inner.write(val);
    }

    /// # Safety:
    /// Ensure that the pointer is a valid virtual pointer and follows the same rules as ptr::write_unaligned
    pub unsafe fn write_unaligned(&self, val: T) {
        self.inner.write_unaligned(val);
    }

    /// # Safety:
    /// Ensure that the pointer is a valid virtual pointer and follows the same rules as ptr::write_volatile
    pub unsafe fn write_volatile(&self, val: T) {
        self.inner.write_volatile(val);
    }

    pub unsafe fn copy_to_nonoverlapping(&self, dest: VirtualPtr<T>, count: usize) {
        self.inner.copy_to_nonoverlapping(dest.as_raw_ptr(), count)
    }

    pub unsafe fn copy_from_nonoverlapping(&self, src: VirtualPtr<T>, count: usize) {
        self.inner.copy_from_nonoverlapping(src.as_raw_ptr(), count)
    }

    pub unsafe fn as_ref(&self) -> Option<&T> {
        return self.inner.as_ref();
    }

    pub unsafe fn as_mut(&self) -> Option<&mut T> {
        return self.inner.as_mut();
    }
}

impl<T> From<usize> for VirtualPtr<T> {
    fn from(addr: usize) -> Self {
        VirtualPtr {
            inner: addr as *mut T,
        }
    }
}

impl<T> From<*mut T> for VirtualPtr<T> {
    fn from(ptr: *mut T) -> Self {
        VirtualPtr { inner: ptr }
    }
}

// constant pointers are a lie anyways tbh
impl<T> From<*const T> for VirtualPtr<T> {
    fn from(ptr: *const T) -> Self {
        VirtualPtr {
            inner: ptr as *mut T,
        }
    }
}

impl<T> Debug for VirtualPtr<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write(f, format_args!("VirtualPtr({:p})", self.inner))
    }
}

impl<T> Pointer for VirtualPtr<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write(f, format_args!("VirtualPtr({:p})", self.inner))
    }
}

pub const PAGE_SIZE: usize = 4096;

pub fn align_up(val: usize, align: usize) -> usize {
    assert!(align.is_power_of_two());
    (val + align - 1) & !(align - 1)
}

pub fn align_down(val: usize, align: usize) -> usize {
    assert!(align.is_power_of_two());
    val & !(align - 1)
}

const HEAP_PAGES: usize = 1024; // 4 MiB heap

#[global_allocator]
pub static ALLOCATOR: Mutex<LinkedListAllocator> = Mutex::new(LinkedListAllocator::new());

// TODO: Limine-rs 0.2.0 does NOT have debug implemented for a lot of it's types, so until that is fixed, either go without Type, or hack limine-rs locally (tracking https://github.com/limine-bootloader/limine-rs/pull/30)
// pub fn log_memory_map() {
//     let memmap = get_memmap();

//     crate::log!(LogLevel::Trace, "====== MEMORY MAP ======");
//     for entry in memmap.iter() {
//         let label = (entry.length as usize).label_bytes();

//         crate::log!(
//             LogLevel::Trace,
//             "[ {:#018X?} ] Type: {:?} Size: {}",
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

    // log_memory_map();
}

pub enum ByteLabelKind {
    BYTE,
    KIB,
    MIB,
    GIB,
}

pub struct ByteLabel {
    byte_label: ByteLabelKind,
    count: usize,
}

impl core::fmt::Display for ByteLabel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let size = self.count;

        match self.byte_label {
            ByteLabelKind::BYTE => {
                write!(f, "{size} Byte")?;
            }
            ByteLabelKind::KIB => {
                write!(f, "{size} KiB")?;
            }
            ByteLabelKind::MIB => {
                write!(f, "{size} MiB")?;
            }
            ByteLabelKind::GIB => {
                write!(f, "{size} GiB")?;
            }
        }

        if size != 1 {
            write!(f, "s")?;
        }

        return Ok(());
    }
}
pub trait LabelBytes {
    fn label_bytes(&self) -> ByteLabel;
}

impl LabelBytes for usize {
    fn label_bytes(&self) -> ByteLabel {
        let bytes = *self;

        let mut byte_label = ByteLabel {
            byte_label: ByteLabelKind::BYTE,
            count: bytes,
        };

        if bytes >> 30 > 0 {
            byte_label.byte_label = ByteLabelKind::GIB;
            byte_label.count = bytes >> 30;
            // return Label::GIB(bytes >> 30);
        } else if bytes >> 20 > 0 {
            byte_label.byte_label = ByteLabelKind::MIB;
            byte_label.count = bytes >> 20;
            // return Label::MIB(bytes >> 20);
        } else if bytes >> 10 > 0 {
            byte_label.byte_label = ByteLabelKind::KIB;
            byte_label.count = bytes >> 10;
            // return Label::KIB(bytes >> 10);
        }

        return byte_label;
    }
}

/// # Safety
/// This will produce undefined behavior if dst is not valid for count writes
pub unsafe fn memset32(dst: VirtualPtr<u32>, val: u32, count: usize) {
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        let mut buf = dst;
        unsafe {
            while buf.addr() < dst.add(count).addr() {
                buf.write_volatile(val);
                buf = buf.offset(1);
            }
        }
        return;
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        let dst = dst.as_raw_ptr();

        core::arch::asm!(
            "rep stosd",
            inout("ecx") count => _,
            inout("edi") dst => _,
            inout("eax") val => _
        );
    }
}
