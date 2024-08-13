pub mod apic;
pub mod exceptions;

use self::apic::APIC;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    base_lo: u16,
    sel: u16,
    ist: u8,
    flags: u8,
    base_mid: u16,
    base_hi: u32,
    always0: u32,
}

impl IdtEntry {
    const fn new() -> Self {
        return Self {
            base_lo: 0,
            sel: 0,
            ist: 0,
            always0: 0,
            flags: 0,
            base_hi: 0,
            base_mid: 0,
        };
    }
}

impl !Sync for IdtEntry {}

#[repr(C, packed)]
struct IdtPtr {
    limit: u16,
    base: u64,
}

static mut IDT: [IdtEntry; 256] = [IdtEntry::new(); 256];

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = 32,
    Keyboard,
}

impl InterruptIndex {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

static mut IDT_PTR: IdtPtr = IdtPtr {
    limit: (core::mem::size_of::<IdtEntry>() * 256) as u16 - 1,
    base: 0,
};

pub fn idt_set_gate(num: u8, function_ptr: usize) {
    let base = function_ptr;
    unsafe {
        IDT[num as usize] = IdtEntry {
            base_lo: (base & 0xFFFF) as u16,
            base_mid: ((base >> 16) & 0xFFFF) as u16,
            base_hi: ((base >> 32) & 0xFFFFFFFF) as u32,
            sel: 0x28,
            ist: 0,
            always0: 0,
            flags: 0xEE,
        };
    }

    // If the interrupt with this number occurred with the "null" interrupt handler
    // We will need to tell the PIC that interrupt is over, this stops new interrupts
    // From never firing because "it was never finished"
    // signal_end_of_interrupt();
}

extern "x86-interrupt" fn null_interrupt_handler() {
    crate::log_info!("Unhandled interrupt!");
    signal_end_of_interrupt();
}

pub fn idt_init() {
    unsafe {
        let idt_size = core::mem::size_of::<IdtEntry>() * 256;
        IDT_PTR.base = IDT.as_ptr() as u64;

        core::ptr::write_bytes(IDT.as_mut_ptr().cast::<core::ffi::c_void>(), 0, idt_size);

        // Set every interrupt to the "null" interrupt handler (it does nothing)
        for num in 0..=255 {
            idt_set_gate(num, null_interrupt_handler as usize);
        }

        idt_set_gate(0x80, syscall as usize);

        core::arch::asm!(
            "lidt [{}]",
            in(reg) core::ptr::addr_of!(IDT_PTR)
        );
    }
}

pub fn signal_end_of_interrupt() {
    APIC.end_of_interrupt();
}

#[naked]
pub extern "C" fn syscall() {
    unsafe {
        core::arch::asm!(
            "push rdi",
            "push rsi",
            "push rdx",
            "push rcx",
            "call {}",
            "pop rdi",
            "pop rsi",
            "pop rdx",
            "pop rcx",
            "iretq",
            options(noreturn),
            sym syscall_handler
        );
    }
}

pub extern "C" fn syscall_handler(_rdi: u64, _rsi: u64, rdx: u64, rcx: u64) {
    let buf = rdx as *const u8; // Treat as pointer to u8 (byte array)
    let count = rcx as usize;

    let slice = unsafe { core::slice::from_raw_parts(buf, count) };
    let message = core::str::from_utf8(slice).unwrap();
    crate::print!("{message}");
}

pub fn enable_interrupts() {
    unsafe {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        core::arch::asm!("sti");

        // TODO: arm and riscv stuff
    }
}
