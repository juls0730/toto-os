#[inline(always)]
pub fn outb(port: u16, value: u8) {
    return;
}

#[inline(always)]
pub fn inb(port: u16) -> u8 {
    return 0;
}

#[inline(always)]
pub fn outw(port: u16, value: u16) {
    return;
}

#[inline(always)]
pub fn inw(port: u16) -> u16 {
    return 0;
}

/// Reads `count` 16-bit values from the specified `port` into the `buffer`.
///
/// # Safety
///
/// This function panics if the supplied buffer's size is smaller than `count`.
#[inline(always)]
pub unsafe fn insw(port: u16, buffer: *mut u16, count: usize) {
    return;
}

/// Outputs `count` 8-bit values from the specified `port` into the `buffer`.
///
/// # Safety
///
/// This function panics if the supplied buffer's size is smaller than `count`.
#[inline(always)]
pub unsafe fn outsb(port: u16, buffer: *const u8, count: usize) {
    return;
}

/// Outputs `count` 16-bit values from the specified `port` into the `buffer`.
///
/// # Safety
///
/// This function panics if the supplied buffer's size is smaller than `count`.
#[inline(always)]
pub unsafe fn outsw(port: u16, buffer: *mut u16, count: usize) {
    return;
}

#[inline(always)]
pub fn outl(port: u16, value: u32) {
    return;
}

#[inline(always)]
pub fn inl(port: u16) -> u32 {
    return 0;
}

#[inline(always)]
pub fn io_wait() {
    return;
}
