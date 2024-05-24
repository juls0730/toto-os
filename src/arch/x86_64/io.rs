use core::arch::asm;

#[inline(always)]
pub fn outb(port: u16, value: u8) {
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(preserves_flags, nomem, nostack)
        );
    }
}

#[inline(always)]
pub fn inb(port: u16) -> u8 {
    let mut value: u8;
    unsafe {
        asm!(
            "in al, dx",
            out("al") value,
            in("dx") port,
            options(preserves_flags, nomem, nostack)
        );
    }

    value
}

#[inline(always)]
pub fn outw(port: u16, value: u16) {
    unsafe {
        asm!(
            "out dx, ax",
            in("dx") port,
            in("ax") value,
            options(preserves_flags, nomem, nostack)
        );
    }
}

#[inline(always)]
pub fn inw(port: u16) -> u16 {
    let mut value: u16;
    unsafe {
        asm!(
            "in ax, dx",
            out("ax") value,
            in("dx") port,
            options(preserves_flags, nomem, nostack)
        );
    }

    value
}

/// Reads `count` 16-bit values from the specified `port` into the `buffer`.
///
/// # Safety
///
/// This function panics if the supplied buffer's size is smaller than `count`.
#[inline(always)]
pub unsafe fn insw(port: u16, buffer: *mut u16, count: usize) {
    asm!("cld",
        "rep insw",
        in("dx") port,
        inout("rdi") buffer => _,
        inout("rcx") count => _
    );
}

/// Outputs `count` 8-bit values from the specified `port` into the `buffer`.
///
/// # Safety
///
/// This function panics if the supplied buffer's size is smaller than `count`.
#[inline(always)]
pub unsafe fn outsb(port: u16, buffer: *const u8, count: usize) {
    asm!("cld",
        "rep outsb",
        in("dx") port,
        inout("rsi") buffer => _,
        inout("rcx") count => _
    );
}

/// Outputs `count` 16-bit values from the specified `port` into the `buffer`.
///
/// # Safety
///
/// This function panics if the supplied buffer's size is smaller than `count`.
#[inline(always)]
pub unsafe fn outsw(port: u16, buffer: *const u16, count: usize) {
    asm!("cld",
        "rep outsw",
        in("dx") port,
        inout("rsi") buffer => _,
        inout("rcx") count => _
    );
}

#[inline(always)]
pub fn outl(port: u16, value: u32) {
    unsafe {
        asm!(
            "out dx, eax",
            in("dx") port,
            in("eax") value,
            options(preserves_flags, nomem, nostack)
        );
    }
}

#[inline(always)]
pub fn inl(port: u16) -> u32 {
    let mut value: u32;
    unsafe {
        asm!(
            "in eax, dx",
            out("eax") value,
            in("dx") port,
            options(preserves_flags, nomem, nostack)
        );
    }

    value
}

#[inline(always)]
pub fn io_wait() {
    outb(0x80, 0);
}
