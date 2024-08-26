#![feature(abi_x86_interrupt, naked_functions, negative_impls)]
#![allow(clippy::needless_return)]
#![no_std]
#![no_main]

use core::arch::x86_64::__cpuid;

use alloc::vec::Vec;
use libs::limine::{get_hhdm_offset, get_kernel_file};
use mem::{pmm::total_memory, LabelBytes};

use crate::drivers::fs::{
    initramfs,
    vfs::{vfs_open, UserCred},
};

extern crate alloc;

pub mod arch;
pub mod drivers;
pub mod libs;
pub mod mem;

// the build id will be an md5sum of the kernel binary and will replace __BUILD_ID__ in the final binary
pub static BUILD_ID: &str = "__BUILD_ID__";

pub static LOG_LEVEL: u8 = if cfg!(debug_assertions) { 1 } else { 2 };

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
    mem::vmm::vmm_init();
    drivers::acpi::init_acpi();

    parse_kernel_cmdline();

    kmain()
}

pub fn kmain() -> ! {
    print_boot_info();

    let _ = drivers::fs::vfs::add_vfs("/", alloc::boxed::Box::new(initramfs::init()));

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    drivers::pci::enumerate_pci_bus();

    crate::println!(
        "YEAH.TXT: {:X?}",
        vfs_open("/firstdir/seconddirbutlonger/yeah.txt")
            .unwrap()
            .open(0, UserCred { uid: 0, gid: 0 })
            .read_all(0, 0)
    );

    drivers::storage::ide::init();

    let limine_dir = vfs_open("/mnt/boot/limine").unwrap();

    crate::println!(
        "LIMINE BOOT: {:X?}",
        limine_dir
            .lookup("limine.conf")
            .unwrap()
            .open(0, UserCred { uid: 0, gid: 0 })
            .read_all(0, 0)
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
            .lookup("limine.conf")
            .unwrap()
            .open(0, UserCred { uid: 0, gid: 0 })
            .read_all(10, 0)
    );

    let _ = drivers::fs::vfs::del_vfs("/mnt");

    let limine_dir = vfs_open("/mnt/boot/limine").unwrap();

    crate::println!(
        "LIMINE BOOT: {:X?}",
        limine_dir
            .lookup("limine.conf")
            .unwrap()
            .open(0, UserCred { uid: 0, gid: 0 })
            .read_all(0, 0)
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
    let pages = length / crate::mem::PAGE_SIZE;

    let hhdm_offset = get_hhdm_offset();

    let buffer_ptr = crate::mem::pmm::pmm_alloc(pages).to_higher_half();

    if buffer_ptr.is_null() {
        panic!("Failed to allocate screen buffer")
    }

    let buffer =
        unsafe { core::slice::from_raw_parts_mut(buffer_ptr.cast::<u32>().as_raw_ptr(), length) };

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

    crate::mem::pmm::pmm_dealloc(unsafe { buffer_ptr.to_lower_half() }, pages);
}

fn print_boot_info() {
    // I dont really like this 'a'
    crate::println!("╔═╗───────────────────╔╗────╔═╗╔══╗");
    crate::println!("║╔╝╔═╗ ╔═╗╔═╗╔╦╗╔═╗╔═╗╠╣╔═╦╗║║║║══╣");
    crate::println!("║╚╗║╬╚╗║╬║║╬║║║║║═╣║═╣║║║║║║║║║╠══║");
    crate::println!("╚═╝╚══╝║╔╝║╔╝╚═╝╚═╝╚═╝╚╝╚╩═╝╚═╝╚══╝");
    crate::println!("───────╚╝─╚╝ ©juls0730 {BUILD_ID}");
    crate::println!("{} of memory available", total_memory().label_bytes());
    crate::println!(
        "The kernel was built in {} mode",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    if let Some(processor_brand) = get_processor_brand() {
        crate::println!("Detected CPU: {processor_brand}");
    }
}

fn get_processor_brand() -> Option<alloc::string::String> {
    if unsafe { __cpuid(0x80000000).eax } >= 0x80000004 {
        return None;
    }

    let mut brand_buf = [0u8; 48];

    let mut offset = 0;
    for i in 0..=2 {
        let cpuid_result = unsafe { __cpuid(0x80000002 + i) };
        brand_buf[offset..offset + 4].copy_from_slice(&cpuid_result.eax.to_le_bytes());
        brand_buf[(offset + 4)..(offset + 8)].copy_from_slice(&cpuid_result.ebx.to_le_bytes());
        brand_buf[(offset + 8)..(offset + 12)].copy_from_slice(&cpuid_result.ecx.to_le_bytes());
        brand_buf[(offset + 12)..(offset + 16)].copy_from_slice(&cpuid_result.edx.to_le_bytes());
        offset += 16;
    }

    // there's probably a better way to do this, but wikipedia says to not rely on the null byte, so I cant use Cstr (and I dont really want to tbh) but if it's shorter than 48bytes it will be null terminated
    let mut brand = alloc::string::String::new();
    for char in brand_buf {
        if char == 0 {
            break;
        }
        brand.push(char as char);
    }

    return Some(brand);
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

#[repr(u8)]
enum LogLevel {
    Trace = 0,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

#[macro_export]
macro_rules! log {
    ($level:expr, $($arg:tt)*) => {{
        let kernel_log_level = if let Ok(kernel_features) = $crate::KERNEL_FEATURES.get() {
            kernel_features.log_level
        } else {
            $crate::LOG_LEVEL
        };

        if ($level as u8) >= kernel_log_level {
            let color_code = match $level {
                $crate::LogLevel::Trace => "\x1B[90m",
                $crate::LogLevel::Debug =>  "\x1B[94m",
                $crate::LogLevel::Info =>  "\x1B[92m",
                $crate::LogLevel::Warn =>  "\x1B[93m",
                $crate::LogLevel::Error =>  "\x1B[91m",
                $crate::LogLevel::Fatal =>  "\x1B[95m",
            };
            $crate::println!("\x1B[97m[ {}* \x1B[97m]\x1B[0;m {}", color_code, &alloc::format!($($arg)*))
        }
    }};
}

#[derive(Debug)]
pub struct KernelFeatures {
    pub log_level: u8,
    pub fat_in_mem: bool,
}

impl KernelFeatures {
    fn update_option(&mut self, option: &str, value: &str) {
        #[allow(clippy::single_match)]
        match option {
            "log_level" => self.log_level = value.parse().unwrap_or(crate::LOG_LEVEL),
            "fat_in_mem" => self.fat_in_mem = value == "true",
            _ => {}
        }
    }
}

// TODO: Do this vastly differently
pub static KERNEL_FEATURES: libs::cell::OnceCell<KernelFeatures> = libs::cell::OnceCell::new();

fn parse_kernel_cmdline() {
    let mut kernel_features: KernelFeatures = KernelFeatures {
        fat_in_mem: true,
        log_level: crate::LOG_LEVEL,
    };

    let kernel_file_response = get_kernel_file();
    if kernel_file_response.is_none() {
        KERNEL_FEATURES.set(kernel_features);
        return;
    }

    let cmdline = core::str::from_utf8(kernel_file_response.unwrap().file().cmdline());

    let kernel_arguments = cmdline.unwrap().split_whitespace().collect::<Vec<&str>>();

    // crate::log!(LogLevel::Trace, "{kernel_arguments:?}");

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
