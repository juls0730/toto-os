use super::idt_set_gate;
use crate::hcf;
use crate::{log_error, log_info};

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

extern "C" fn exception_handler(registers: u64) {
    let registers = unsafe { *(registers as *const Registers) };

    crate::println!("{:X?}", registers);

    let int = registers.int;

    match int {
        0x00 => {
            log_error!("DIVISION ERROR!");
        }
        0x06 => {
            log_error!("INVALID OPCODE!");
        }
        0x08 => {
            log_error!("DOUBLE FAULT!");
        }
        0x0D => {
            log_error!("GENERAL PROTECTION FAULT!");
        }
        0x0E => {
            log_error!("PAGE FAULT!");
        }
        0xFF => {
            log_error!("EXCEPTION!");
        }
        _ => {
            log_error!("EXCEPTION!");
        }
    }

    log_info!(
        "INT: {:x} RIP: {:X}, CS: {:X}, EFLAGS: {:b}",
        int,
        registers.rip,
        registers.cs,
        registers.rflags
    );

    crate::arch::stack_trace::print_stack_trace(6, registers.rbp as u64);
}

// *macro intensifies*
macro_rules! exception_function {
    ($code:expr, $handler:ident) => {
        #[inline(always)]
        extern "C" fn $handler() {
            unsafe {
                core::arch::asm!(
                    "pushfq",
                    "push {0:r}",
                    "mov rdi, rsp",
                    "call {1}",
                    "pop {0:r}",
                    "mov rsp, rdi",
                    "popfq",
                    in(reg) $code,
                    sym exception_handler,
                );
            };

            hcf();
        }
    };
}

exception_function!(0x00, div_error);
exception_function!(0x06, invalid_opcode);
exception_function!(0x08, double_fault);
exception_function!(0x0D, general_protection_fault);
// TODO: fix the page fault then gracefully return.
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
