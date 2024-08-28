use crate::{hcf, libs::cell::OnceCell, mem::VirtualPtr, LogLevel};

use alloc::{sync::Arc, vec::Vec};

use super::super::{cpu_get_msr, cpu_set_msr, io::outb};

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
struct MADT {
    pub local_apic_address: u32,
    pub flags: u32,
}

const IA32_APIC_BASE_MSR: u32 = 0x1B;
const IA32_APIC_BASE_MSR_ENABLE: usize = 0x800;

pub fn has_apic() -> bool {
    return unsafe { core::arch::x86_64::__cpuid_count(1, 0).edx } & 1 << 9 != 0;
}

fn set_apic_base(apic: usize) {
    let edx: u32 = 0;
    let eax = (apic & 0xfffff0000) | IA32_APIC_BASE_MSR_ENABLE;

    unsafe { cpu_set_msr(IA32_APIC_BASE_MSR, &(eax as u32), &edx) };
}

fn get_apic_base() -> u32 {
    let mut eax: u32 = 0;
    let mut edx: u32 = 0;
    unsafe { cpu_get_msr(IA32_APIC_BASE_MSR, &mut eax, &mut edx) };

    return eax & 0xfffff000;
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct LAPIC {
    pub acpi_processor_id: u8,
    pub apic_id: u8,
    pub flags: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct IOAPIC {
    pub ioapic_id: u8,
    _reserved: u8,
    pub ptr: VirtualPtr<u8>,
    pub global_interrupt_base: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct IOAPICSourceOverride {
    bus_source: u8,
    irq_source: u8,
    global_system_interrupt: u32,
    flags: u16,
}

#[derive(Debug)]
pub struct APIC {
    pub io_apic: IOAPIC,
    local_apic: VirtualPtr<u8>,
    pub cpus: Arc<[LAPIC]>,
}

unsafe extern "C" fn test(cpu: &limine::smp::Cpu) -> ! {
    crate::log!(LogLevel::Debug, "hey from CPU {:<02}", cpu.id);

    hcf();
}

impl APIC {
    pub fn new() -> Result<Self, ()> {
        disable_pic();

        if !has_apic() {
            return Err(());
        }

        let apic_base = get_apic_base() as usize;

        set_apic_base(apic_base);

        let madt = crate::drivers::acpi::find_table::<MADT>("APIC");

        if madt.is_none() {
            return Err(());
        }

        let mut cpus: Vec<LAPIC> = Vec::new();

        let madt = madt.unwrap();

        crate::log!(
            LogLevel::Trace,
            "MADT located at: {:p}",
            core::ptr::addr_of!(madt)
        );

        let hhdm_offset = crate::libs::limine::get_hhdm_offset();

        let mut lapic_ptr: VirtualPtr<u8> =
            VirtualPtr::from(madt.inner.local_apic_address as usize + hhdm_offset);
        let mut io_apic = None;
        let mut io_apic_source_override = None;

        // constant pointers are a lie anyways
        let mut ptr: VirtualPtr<u8> = VirtualPtr::from(madt.extra.unwrap().as_ptr());
        let ptr_end: VirtualPtr<u8> = unsafe { ptr.add(madt.header.length as usize - 44) };

        while (ptr.addr()) < (ptr_end.addr()) {
            // ptr may or may bot be aligned, although I have had crashes related to this pointer being not aligned
            // and tbh I dont really care about the performance impact of reading unaligned pointers right now
            // TODO
            match unsafe { ptr.read_unaligned() } {
                0 => {
                    if unsafe { ptr.add(4).read() } & 1 != 0 {
                        cpus.push(unsafe { ptr.add(2).cast::<LAPIC>().read_unaligned() });
                    }
                }
                1 => unsafe {
                    io_apic = Some(IOAPIC {
                        ioapic_id: ptr.add(2).read_unaligned(),
                        _reserved: ptr.add(3).read_unaligned(),
                        ptr: VirtualPtr::from(
                            ptr.add(4).cast::<u32>().read_unaligned() as usize + hhdm_offset,
                        ),
                        global_interrupt_base: ptr.add(8).cast::<u32>().read_unaligned(),
                    })
                },
                2 => unsafe {
                    io_apic_source_override = Some(IOAPICSourceOverride {
                        bus_source: ptr.add(2).read_unaligned(),
                        irq_source: ptr.add(3).read_unaligned(),
                        global_system_interrupt: ptr.add(4).cast::<u32>().read_unaligned(),
                        flags: ptr.add(8).cast::<u16>().read_unaligned(),
                    })
                },
                5 => {
                    lapic_ptr = unsafe {
                        VirtualPtr::from(
                            ptr.add(4).cast::<u64>().read_unaligned() as usize + hhdm_offset,
                        )
                    }
                }
                _ => {}
            }

            ptr = unsafe { ptr.add(ptr.add(1).read_unaligned() as usize) };
        }

        if io_apic.is_none() || io_apic_source_override.is_none() {
            return Err(());
        }

        let io_apic_ptr = io_apic.unwrap().ptr;

        crate::log!(
            LogLevel::Debug,
            "Found {} core{}, IOAPIC {:p}, LAPIC {lapic_ptr:p}, Processor IDs:",
            cpus.len(),
            if cpus.len() > 1 { "s" } else { "" },
            io_apic_ptr,
        );

        for apic in &cpus {
            crate::log!(LogLevel::Debug, "    {}", apic.acpi_processor_id);
        }

        let apic = Self {
            io_apic: io_apic.unwrap(),
            local_apic: lapic_ptr,
            cpus: cpus.into(),
        };

        // Enable APIC by setting bit 8 to 1
        apic.write_lapic(0xF0, apic.read_lapic(0xF0) | 0x100);

        let io_apic_ver = apic.read_ioapic(0x01);

        let _number_of_inputs = ((io_apic_ver >> 16) & 0xFF) + 1;

        // crate::println!("{number_of_inputs}");

        let smp_request = crate::libs::limine::get_smp();

        assert!(smp_request.is_some(), "Failed to get smp from limine!");

        let smp_request = smp_request.unwrap();
        let bsp_lapic_id = smp_request.bsp_lapic_id();

        for cpu in smp_request.cpus() {
            if cpu.id == bsp_lapic_id {
                continue;
            }

            cpu.goto_address.write(test);
        }

        // Set and enable keyboard interrupt
        apic.set_interrupt(0x01, 0x01);

        return Ok(apic);
    }

    pub fn read_ioapic(&self, reg: u32) -> u32 {
        unsafe {
            let ptr = self.io_apic.ptr;
            ptr.cast::<u32>().as_raw_ptr().write_volatile(reg & 0xFF);
            return ptr.cast::<u32>().add(4).read_volatile();
        }
    }

    pub fn write_ioapic(&self, reg: u32, value: u32) {
        unsafe {
            let ptr = self.io_apic.ptr;
            ptr.cast::<u32>().write_volatile(reg & 0xFF);
            ptr.cast::<u32>().add(4).write_volatile(value);
        }
    }

    pub fn read_lapic(&self, reg: u32) -> u32 {
        unsafe {
            return self.local_apic.add(reg as usize).cast::<u32>().read();
        }
    }

    pub fn write_lapic(&self, reg: u32, value: u32) {
        unsafe {
            self.local_apic
                .add(reg as usize)
                .cast::<u32>()
                .write_volatile(value);
        }
    }

    pub fn lapic_send_ipi(&self, dest_id: u32, vec: u32) {
        self.write_lapic(0x310, dest_id << 24);
        self.write_lapic(0x300, vec);
    }

    pub fn set_interrupt(&self, int_num: u8, redirtion_num: u8) {
        let retbl_offset: u32 = 0x10 + (int_num as u32 * 2);
        let interrupts_vt =
            ((self.read_ioapic(retbl_offset) & 0x7FFF) & 0xFF00) | (0x20 + redirtion_num as u32);

        self.write_ioapic(retbl_offset, interrupts_vt)
    }

    pub fn end_of_interrupt(&self) {
        self.write_lapic(0xB0, 0x00);
    }
}

const PIC_CMD_MASTER: u16 = 0x20;
const PIC_CMD_SLAVE: u16 = 0xA0;
const PIC_DATA_MASTER: u16 = 0x21;
const PIC_DATA_SLAVE: u16 = 0xA1;

fn disable_pic() {
    // Tell each PIC we're going to initialize it.
    outb(PIC_CMD_MASTER, 0x11);
    outb(PIC_CMD_SLAVE, 0x11);

    // Byte 1: Set up our base offsets.
    outb(PIC_DATA_MASTER, 0x20);
    outb(PIC_DATA_SLAVE, 0x28);

    // Byte 2: Configure chaining
    outb(PIC_DATA_MASTER, 0x04); // Tell Master Pic that there is a slave Pic at IRQ2
    outb(PIC_DATA_SLAVE, 0x02); // Tell Slave PIC it's cascade identity

    // Byte 3: Set out mode to 8086.
    outb(PIC_DATA_MASTER, 0x01);
    outb(PIC_DATA_SLAVE, 0x01);

    // Set each PIC's mask to 0xFF, disabling PIC interrupts
    outb(PIC_DATA_MASTER, 0xFF);
    outb(PIC_DATA_SLAVE, 0xFF);
}

// // TODO: last I remember these didnt work
// pub fn usdelay(useconds: u16) {
//     let pit_count = ((useconds as u32 * 1193) / 1000) as u16;

//     pit_delay(pit_count);
// }

// pub fn msdelay(ms: u32) {
//     let mut total_count = ms * 1193;

//     while total_count > 0 {
//         let chunk_count = if total_count > u16::MAX as u32 {
//             u16::MAX
//         } else {
//             total_count as u16
//         };

//         pit_delay(chunk_count);

//         total_count -= chunk_count as u32;
//     }
// }

// pub fn pit_delay(count: u16) {
//     // Set PIT to mode 0
//     outb(0x43, 0x30);
//     outb(0x40, (count & 0xFF) as u8);
//     outb(0x40, ((count & 0xFF00) >> 8) as u8);
//     loop {
//         // Tell PIT to give us a timer status
//         outb(0x43, 0xE2);
//         if ((inb(0x40) >> 7) & 0x01) != 0 {
//             break;
//         }
//     }
// }

pub static APIC: OnceCell<APIC> = OnceCell::new();
