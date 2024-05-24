#![no_std]
#![no_main]

// extern crate alloc;

// use core::alloc::GlobalAlloc;

// use alloc::{borrow::ToOwned, format, string::String};

// struct Allocator;

// unsafe impl GlobalAlloc for Allocator {
//     unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
//         return malloc(layout.size());
//     }

//     unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
//         free(ptr, layout.size());
//     }
// }

// #[global_allocator]
// static ALLOC: Allocator = Allocator;

// extern "C" {
//     fn malloc(size: usize) -> *mut u8;
//     fn free(ptr: *mut u8, size: usize);
// }

#[no_mangle]
pub fn _start(_args: &[&str]) {
    let message = "Hello, World!\n";

    // if args.len() > 1 {
    //     message = format!("Hello, {}!\n", args[1]);
    // }

    print(message);
}

fn print(message: &str) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    unsafe {
        core::arch::asm!(
            "mov rdi, 0x01", // write syscall
            "mov rsi, 0x01", // stdio (but it doesnt matter)
            "mov rdx, {0:r}", // pointer
            "mov rcx, {1:r}", // count
            "int 0x80",
            in(reg) message.as_ptr(),
            in(reg) message.len()
        );
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    print("An exception occured!\n");
    loop {}
}
