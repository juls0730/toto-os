use core::sync::atomic::{AtomicU8, Ordering};

use super::idt_set_gate;
use crate::{hcf, log, mem::VirtualPtr, LogLevel};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Registers {
    // Pushed by wrapper
    int: usize,

    // Pushed by push_gprs in crate::arch::x86_64
    r15: usize,
    r14: usize,
    r13: usize,
    r12: usize,
    r11: usize,
    r10: usize,
    r9: usize,
    r8: usize,
    rbp: usize,
    rdi: usize,
    rsi: usize,
    rdx: usize,
    rcx: usize,
    rbx: usize,
    rax: usize,

    // Pushed by interrupt
    rip: usize,
    cs: usize,
    rflags: usize,
    rsp: usize,
    ss: usize,
}

static FAULTED: AtomicU8 = AtomicU8::new(0);

extern "C" fn exception_handler(registers: u64) {
    let registers = unsafe { VirtualPtr::<Registers>::from(registers as usize).read() };

    match FAULTED.fetch_add(1, Ordering::SeqCst) {
        0 => {}
        1 => {
            log!(LogLevel::Fatal, "Exception Loop detected, stopping here");
            print_registers(&registers);
            hcf();
        }
        // We have faulted multiple times, this could indicate an issue with the allocator, stop everything without further logging since it will likely cause more issues
        _ => hcf(),
    }

    let int = registers.int;

    match int {
        0x00 => {
            log!(LogLevel::Fatal, "DIVISION ERROR!");
        }
        0x06 => {
            log!(LogLevel::Fatal, "INVALID OPCODE!");
        }
        0x08 => {
            log!(LogLevel::Fatal, "DOUBLE FAULT!");
        }
        0x0D => {
            log!(LogLevel::Fatal, "GENERAL PROTECTION FAULT!");
        }
        0x0E => {
            log!(LogLevel::Fatal, "PAGE FAULT!");
            log!(
                LogLevel::Debug,
                "HINT: Find the last pointer you touched and make sure it's in virtual memory"
            );
        }
        _ => {
            log!(LogLevel::Fatal, "EXCEPTION!");
        }
    }

    print_registers(&registers);

    crate::arch::stack_trace::print_stack_trace(6, registers.rbp as u64);
}

fn print_registers(registers: &Registers) {
    log!(LogLevel::Info, "{:─^width$}", " REGISTERS ", width = 98);

    log!(
        LogLevel::Info,
        "INT: {:#018X}, RIP: {:#018X},  CS: {:#018X}, FLG: {:#018X}",
        registers.int,
        registers.rip,
        registers.cs,
        registers.rflags
    );

    log!(
        LogLevel::Info,
        "RSP: {:#018X},  SS: {:#018X}, RAX: {:#018X}, RBX: {:#018X}",
        registers.rsp,
        registers.ss,
        registers.rax,
        registers.rbx
    );

    log!(
        LogLevel::Info,
        "RCX: {:#018X}, RDX: {:#018X}, RSI: {:#018X}, RDI: {:#018X}",
        registers.rcx,
        registers.rdx,
        registers.rsi,
        registers.rdi
    );

    log!(
        LogLevel::Info,
        "RBP: {:#018X},  R8: {:#018X},  R9: {:#018X}, R10: {:#018X}",
        registers.rbp,
        registers.r8,
        registers.r9,
        registers.r10
    );

    log!(
        LogLevel::Info,
        "R11: {:#018X}, R12: {:#018X}, R13: {:#018X}, R14: {:#018X}",
        registers.r11,
        registers.r12,
        registers.r13,
        registers.r14
    );

    log!(LogLevel::Info, "R15: {:#018X}", registers.r15);
}

// *macro intensifies*
macro_rules! exception_function {
    ($code:expr, $handler:ident) => {
        #[inline(always)]
        extern "C" fn $handler() {
            crate::arch::push_gprs();

            unsafe {
                core::arch::asm!(
                    "push {0:r}",
                    "mov rdi, rsp",
                    "call {1}",
                    "pop {0:r}",
                    "mov rsp, rdi",
                    in(reg) $code,
                    sym exception_handler,
                    options(nostack)
                );
            };

            crate::arch::pop_gprs();

            super::signal_end_of_interrupt();

            hcf();
        }
    };
}

exception_function!(0x00, div_error);
exception_function!(0x06, invalid_opcode);
exception_function!(0x08, double_fault);
exception_function!(0x0D, general_protection_fault);
exception_function!(0x0E, page_fault);
exception_function!(0xFF, generic_handler);

pub fn exceptions_init() {
    for i in 0..32 {
        idt_set_gate(i, generic_handler as usize);
    }

    idt_set_gate(0x00, div_error as usize);
    idt_set_gate(0x06, invalid_opcode as usize);
    idt_set_gate(0x08, double_fault as usize);
    idt_set_gate(0x0D, general_protection_fault as usize);
    idt_set_gate(0x0E, page_fault as usize);
}
