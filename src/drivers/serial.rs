use core::sync::atomic::AtomicBool;

#[cfg(target_arch = "x86_64")]
use crate::arch::io::{inb, outb, outsb};
use crate::mem::VirtualPtr;

// COM1
pub static PORT: u16 = 0x3f8;
pub const UART: *mut u8 = 0x10000000 as *mut u8;

pub static POISONED: AtomicBool = AtomicBool::new(false);

// Serial ports are as follows:
// PORT + 0: Data register.
//           Reading this recieves from this buffer.
//           Writing to this writes to the transmit buffer.
// PORT + 1: Interrupt enable register.
// PORT + 2: Interrupt identification and FIFO control registers.
// PORT + 3: Line control register, this sets DLAB to the most significant bit.
// PORT + 4: Modem control register
#[cfg(target_arch = "x86_64")]
pub fn init_serial() -> u8 {
    outb(PORT + 7, 0x42);
    if inb(PORT + 7) != 0x42 {
        // serial port does not exist
        return 1;
    }

    outb(PORT + 1, 0x00);
    outb(PORT + 3, 0x80);
    outb(PORT, 0x03);
    outb(PORT + 1, 0x00);
    outb(PORT + 3, 0x03);
    outb(PORT + 2, 0xC7);
    outb(PORT + 4, 0x0B);
    outb(PORT + 4, 0x1E);
    outb(PORT, 0xAE);

    // Check if serial is faulty
    if inb(PORT) != 0xAE {
        POISONED.store(true, core::sync::atomic::Ordering::Relaxed);
        return 1;
    }

    // Set serial in normal operation mode
    outb(PORT + 4, 0x0F);
    return 0;
}

pub fn write_string(string: &str) {
    #[cfg(target_arch = "x86_64")]
    {
        while is_transmit_empty() {}

        unsafe { outsb(PORT, VirtualPtr::from(string.as_ptr()), string.len()) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        for &ch in string.as_bytes() {
            write_serial(ch);
        }
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn is_transmit_empty() -> bool {
    return inb((PORT + 5) & 0x20) == 0;
}

#[cfg(not(target_arch = "x86_64"))]
pub fn init_serial() -> u8 {
    return 0;
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn write_serial(character: u8) {
    while is_transmit_empty() {}
    if character == b'\n' {
        write_serial(b'\r');
    }

    outb(PORT, character);
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn serial_recieved() -> bool {
    return (inb(PORT + 5) & 0x01) == 0;
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn read_serial() -> u8 {
    while serial_recieved() {}
    return inb(PORT);
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
pub fn is_transmit_empty() -> bool {
    return true;
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
pub fn write_serial(character: u8) {
    unsafe {
        UART.write(character);
    };
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
pub fn read_serial() -> u8 {
    return 0;
}
