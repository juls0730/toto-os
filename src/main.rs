#![feature(
    allocator_api,
    abi_x86_interrupt,
    naked_functions,
    const_mut_refs,
    negative_impls
)]
#![allow(clippy::needless_return)]
#![no_std]
#![no_main]

use core::ffi::CStr;

use alloc::vec::Vec;
use limine::KernelFileRequest;

use crate::drivers::fs::{
    initramfs,
    vfs::{vfs_open, UserCred},
};

extern crate alloc;

pub mod arch;
pub mod drivers;
pub mod libs;
pub mod mem;

pub static KERNEL_REQUEST: KernelFileRequest = KernelFileRequest::new(0);

#[no_mangle]
pub extern "C" fn _start() -> ! {
    drivers::serial::init_serial();
    arch::gdt::gdt_init();
    arch::interrupts::idt_init();
    arch::interrupts::exceptions::exceptions_init();
    arch::interrupts::enable_interrupts();
    // TODO: memory stuff
    mem::pmm::pmm_init();
    mem::init_allocator();
    drivers::acpi::init_acpi();

    parse_kernel_cmdline();

    kmain()
}

pub fn kmain() -> ! {
    let _ = drivers::fs::vfs::add_vfs("/", alloc::boxed::Box::new(initramfs::init()));

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    drivers::pci::enumerate_pci_bus();

    crate::println!(
        "YEAH.TXT: {:X?}",
        vfs_open("/firstdir/seconddirbutlonger/yeah.txt")
            .unwrap()
            .open(0, UserCred { uid: 0, gid: 0 })
            .read(0, 0, 0)
    );

    drivers::storage::ide::init();
    let limine_dir = vfs_open("/mnt/boot/limine").unwrap();

    crate::println!(
        "LIMINE BOOT: {:X?}",
        limine_dir
            .lookup("limine.cfg")
            .unwrap()
            .open(0, UserCred { uid: 0, gid: 0 })
            .read(0, 0, 0)
    );

    let root_dir = vfs_open("/").unwrap();

    crate::println!(
        "LIMINE BOOT THROUGH LOOKUP: {:X?}",
        root_dir
            .lookup("mnt")
            .unwrap()
            .lookup("boot")
            .unwrap()
            .lookup("limine")
            .unwrap()
            .lookup("limine.cfg")
            .unwrap()
            .open(0, UserCred { uid: 0, gid: 0 })
            .read(0, 10, 0)
    );

    let _ = drivers::fs::vfs::del_vfs("/mnt");

    println!("wa");

    let limine_dir = vfs_open("/mnt/boot/limine").unwrap();

    crate::println!(
        "LIMINE BOOT: {:X?}",
        limine_dir
            .lookup("limine.cfg")
            .unwrap()
            .open(0, UserCred { uid: 0, gid: 0 })
            .read(0, 0, 0)
    );

    // let file = vfs_open("/example.txt").unwrap();

    // as a sign that we didnt panic
    draw_gradient();

    // loop {
    //     let ch = crate::drivers::serial::read_serial();

    //     if ch == b'\x00' {
    //         continue;
    //     }

    //     if ch == b'\x08' {
    //         crate::drivers::serial::write_serial(b'\x08');
    //         crate::drivers::serial::write_serial(b' ');
    //         crate::drivers::serial::write_serial(b'\x08');
    //     }

    //     if ch > 0x1F && ch < 0x7F {
    //         crate::drivers::serial::write_serial(ch);
    //     }
    // }

    hcf();
}

fn draw_gradient() {
    let fb = drivers::video::get_framebuffer().unwrap();
    let length = (fb.height * fb.width) * (fb.bpp / 8);
    let pages = length / crate::mem::pmm::PAGE_SIZE;

    let buffer_ptr = crate::mem::PHYSICAL_MEMORY_MANAGER.alloc(pages);

    if buffer_ptr.is_null() {
        panic!("Failed to allocate screen buffer")
    }

    let buffer = unsafe {
        core::slice::from_raw_parts_mut(
            crate::mem::PHYSICAL_MEMORY_MANAGER
                .alloc(pages)
                .cast::<u32>(),
            length,
        )
    };

    for y in 0..fb.height {
        for x in 0..fb.width {
            let r = (255 * x) / (fb.width - 1);
            let g = (255 * y) / (fb.height - 1);
            let b = 255 - r;

            let pixel = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
            buffer[((y * fb.pitch) / (fb.bpp / 8)) + x] = pixel
        }
    }

    fb.blit_screen(buffer, None);

    crate::mem::PHYSICAL_MEMORY_MANAGER.dealloc(buffer_ptr, pages);
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", &alloc::format!($($arg)*)));
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => (
        $crate::drivers::serial::write_string(&alloc::format!($($arg)*).replace('\n', "\n\r"))
    )
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => ($crate::println!("\x1B[97m[ \x1B[90m? \x1B[97m]\x1B[0m {}", &alloc::format!($($arg)*)));
}

#[macro_export]
macro_rules! log_serial {
    ($($arg:tt)*) => (
            $crate::drivers::serial::write_string(&alloc::format!($($arg)*).replace('\n', "\n\r"))
    );
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => ($crate::println!("\x1B[97m[ \x1B[91m! \x1B[97m]\x1B[0m {}", &alloc::format!($($arg)*)));
}

#[macro_export]
macro_rules! log_ok {
    ($($arg:tt)*) => ($crate::println!("\x1B[97m[ \x1B[92m* \x1B[97m]\x1B[0;m {}", &alloc::format!($($arg)*)));
}

#[derive(Debug)]
pub struct KernelFeatures {
    pub fat_in_mem: bool,
}

impl KernelFeatures {
    fn update_option(&mut self, option: &str, value: &str) {
        #[allow(clippy::single_match)]
        match option {
            "fat_in_mem" => self.fat_in_mem = value == "true",
            _ => {}
        }
    }
}

// TODO: Do this vastly differently
pub static KERNEL_FEATURES: libs::cell::OnceCell<KernelFeatures> = libs::cell::OnceCell::new();

fn parse_kernel_cmdline() {
    let mut kernel_features: KernelFeatures = KernelFeatures { fat_in_mem: true };

    let kernel_file_response = KERNEL_REQUEST.get_response().get();
    if kernel_file_response.is_none() {
        KERNEL_FEATURES.set(kernel_features);
        return;
    }

    let cmdline_ptr = kernel_file_response
        .unwrap()
        .kernel_file
        .get()
        .unwrap()
        .cmdline
        .as_ptr();

    if cmdline_ptr.is_none() {
        KERNEL_FEATURES.set(kernel_features);
        return;
    }

    let cmdline = unsafe { CStr::from_ptr(cmdline_ptr.unwrap()) };
    let kernel_arguments = cmdline
        .to_str()
        .unwrap()
        .split_whitespace()
        .collect::<Vec<&str>>();

    crate::println!("{kernel_arguments:?}");

    for item in kernel_arguments {
        let parts: Vec<&str> = item.split('=').collect();

        if parts.len() == 2 {
            let (option, value) = (parts[0], parts[1]);

            kernel_features.update_option(option, value);
        }
    }

    KERNEL_FEATURES.set(kernel_features);
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    crate::println!("{info}");

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        let rbp: u64;
        unsafe {
            core::arch::asm!("mov {0:r}, rbp", out(reg) rbp);
        };
        crate::arch::stack_trace::print_stack_trace(6, rbp);
    }

    hcf();
}

pub fn hcf() -> ! {
    loop {
        unsafe {
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            core::arch::asm!("hlt");

            #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
            core::arch::asm!("wfi");
        }
    }
}
