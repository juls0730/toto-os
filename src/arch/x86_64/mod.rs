pub mod gdt;
pub mod interrupts;
pub mod io;
pub mod paging;
pub mod stack_trace;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline(always)]
pub fn pause() {
    unsafe {
        core::arch::asm!("pause");
    };
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline(always)]
pub fn cpu_has_msr() -> bool {
    return unsafe { core::arch::x86_64::__cpuid_count(1, 0).edx } & 1 << 5 != 0;
}

pub unsafe fn cpu_get_msr(msr: u32, lo: &mut u32, hi: &mut u32) {
    core::arch::asm!(
        "rdmsr",
        in("ecx") msr,
        inout("eax") *lo,
        inout("edx") *hi,
    );
}

pub unsafe fn cpu_set_msr(msr: u32, lo: &u32, hi: &u32) {
    core::arch::asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") *lo,
        in("edx") *hi,
    );
}

// struct Registers {
//     // Pushed by wrapper
//     int: usize,

//     // Pushed by push_gprs in crate::arch::x86_64
//     r15: usize,
//     r14: usize,
//     r13: usize,
//     r12: usize,
//     r11: usize,
//     r10: usize,
//     r9: usize,
//     r8: usize,
//     rbp: usize,
//     rdi: usize,
//     rsi: usize,
//     rdx: usize,
//     rcx: usize,
//     rbx: usize,
//     rax: usize,

//     // Pushed by interrupt
//     rip: usize,
//     cs: usize,
//     rflags: usize,
//     rsp: usize,
//     ss: usize,
// }

pub fn push_gprs() {
    unsafe {
        core::arch::asm!(
            "push rax",
            "push rbx",
            "push rcx",
            "push rdx",
            "push rsi",
            "push rdi",
            "push rbp",
            "push r8",
            "push r9",
            "push r10",
            "push r11",
            "push r12",
            "push r13",
            "push r14",
            "push r15",
            options(nostack)
        )
    }
}

pub fn pop_gprs() {
    unsafe {
        core::arch::asm!(
            "pop r15",
            "pop r14",
            "pop r13",
            "pop r12",
            "pop r11",
            "pop r10",
            "pop r9",
            "pop r8",
            "pop rbp",
            "pop rdi",
            "pop rsi",
            "pop rdx",
            "pop rcx",
            "pop rbx",
            "pop rax",
            options(nostack)
        )
    }
}
