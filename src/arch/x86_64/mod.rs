pub mod gdt;
pub mod interrupts;
pub mod io;
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
