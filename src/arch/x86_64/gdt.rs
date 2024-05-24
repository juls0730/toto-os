#[derive(Default)]
#[repr(C)]
struct GDTDescriptor {
    limit: u16,
    base_low: u16,
    base_mid: u8,
    access: u8,
    granularity: u8,
    base_high: u8,
}

#[derive(Default)]
#[repr(C)]
struct TSSDescriptor {
    length: u16,
    base_low: u16,
    base_mid: u8,
    flags1: u8,
    flags2: u8,
    base_high: u8,
    base_upper: u32,
    _reserved: u32,
}

#[derive(Default)]
#[repr(C)]
struct GDT {
    descriptors: [GDTDescriptor; 11],
    tss: TSSDescriptor,
}

#[repr(C, packed)]
struct GDTPtr {
    limit: u16,
    base: u64,
}

static mut GDT: Option<GDT> = None;
static mut GDTR: GDTPtr = GDTPtr { limit: 0, base: 0 };

pub fn gdt_init() {
    unsafe {
        GDT = Some(GDT::default());
        let gdt = GDT.as_mut().unwrap();

        gdt.descriptors[0].limit = 0;
        gdt.descriptors[0].base_low = 0;
        gdt.descriptors[0].base_mid = 0;
        gdt.descriptors[0].access = 0;
        gdt.descriptors[0].granularity = 0;
        gdt.descriptors[0].base_high = 0;

        gdt.descriptors[1].limit = 0xFFFF;
        gdt.descriptors[1].base_low = 0;
        gdt.descriptors[1].base_mid = 0;
        gdt.descriptors[1].access = 0x9A;
        gdt.descriptors[1].granularity = 0;
        gdt.descriptors[1].base_high = 0;

        gdt.descriptors[2].limit = 0xFFFF;
        gdt.descriptors[2].base_low = 0;
        gdt.descriptors[2].base_mid = 0;
        gdt.descriptors[2].access = 0x92;
        gdt.descriptors[2].granularity = 0;
        gdt.descriptors[2].base_high = 0;

        gdt.descriptors[3].limit = 0xFFFF;
        gdt.descriptors[3].base_low = 0;
        gdt.descriptors[3].base_mid = 0;
        gdt.descriptors[3].access = 0x9A;
        gdt.descriptors[3].granularity = 0xCF;
        gdt.descriptors[3].base_high = 0;

        gdt.descriptors[4].limit = 0xFFFF;
        gdt.descriptors[4].base_low = 0;
        gdt.descriptors[4].base_mid = 0;
        gdt.descriptors[4].access = 0x92;
        gdt.descriptors[4].granularity = 0xCF;
        gdt.descriptors[4].base_high = 0;

        gdt.descriptors[5].limit = 0;
        gdt.descriptors[5].base_low = 0;
        gdt.descriptors[5].base_mid = 0;
        gdt.descriptors[5].access = 0x9A;
        gdt.descriptors[5].granularity = 0x20;
        gdt.descriptors[5].base_high = 0;

        gdt.descriptors[6].limit = 0;
        gdt.descriptors[6].base_low = 0;
        gdt.descriptors[6].base_mid = 0;
        gdt.descriptors[6].access = 0x92;
        gdt.descriptors[6].granularity = 0;
        gdt.descriptors[6].base_high = 0;

        // descriptors[7] and descriptors[8] are already dummy entries for SYSENTER

        gdt.descriptors[9].limit = 0;
        gdt.descriptors[9].base_low = 0;
        gdt.descriptors[9].base_mid = 0;
        gdt.descriptors[9].access = 0xFA;
        gdt.descriptors[9].granularity = 0x20;
        gdt.descriptors[9].base_high = 0;

        gdt.descriptors[10].limit = 0;
        gdt.descriptors[10].base_low = 0;
        gdt.descriptors[10].base_mid = 0;
        gdt.descriptors[10].access = 0xF2;
        gdt.descriptors[10].granularity = 0;
        gdt.descriptors[10].base_high = 0;

        gdt.tss.length = 104;
        gdt.tss.base_low = 0;
        gdt.tss.base_mid = 0;
        gdt.tss.flags1 = 0x89;
        gdt.tss.flags2 = 0;
        gdt.tss.base_high = 0;
        gdt.tss.base_upper = 0;
        gdt.tss._reserved = 0;

        GDTR.limit = core::mem::size_of::<GDT>() as u16 - 1;
        GDTR.base = gdt as *mut GDT as u64;
    }

    gdt_reload();
}

pub fn gdt_reload() {
    unsafe {
        core::arch::asm!(
            "lgdt [{}]",
            "push 0x28",
            "lea rax, [rip+0x3]",
            "push rax",
            "retfq",
            "mov eax, 0x30",
            "mov ds, eax",
            "mov es, eax",
            "mov fs, eax",
            "mov gs, eax",
            "mov ss, eax",
            in(reg) core::ptr::addr_of!(GDTR)
        );
    }
}
