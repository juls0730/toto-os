use limine::{
    file::File,
    framebuffer::Framebuffer,
    paging::Mode,
    request::{
        FramebufferRequest, KernelAddressRequest, KernelFileRequest, ModuleRequest, RsdpRequest,
        SmpRequest,
    },
    response::{KernelAddressResponse, KernelFileResponse, SmpResponse},
    BaseRevision,
};

use super::cell::OnceCell;

// Be sure to mark all limine requests with #[used], otherwise they may be removed by the compiler.
#[used]
// The .requests section allows limine to find the requests faster and more safely.
#[link_section = ".requests"]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[link_section = ".requests"]
static KERNEL_REQUEST: KernelFileRequest = KernelFileRequest::new();

#[used]
#[link_section = ".requests"]
static KERNEL_ADDRESS_REQUEST: KernelAddressRequest = KernelAddressRequest::new();

#[used]
#[link_section = ".requests"]
static mut SMP_REQUEST: SmpRequest = SmpRequest::new();

#[used]
#[link_section = ".requests"]
static RSDP_REQ: RsdpRequest = RsdpRequest::new();

#[used]
#[link_section = ".requests"]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[link_section = ".requests"]
static MODULE_REQUEST: ModuleRequest = ModuleRequest::new();

#[used]
#[link_section = ".requests"]
static mut MEMMAP_REQUEST: limine::request::MemoryMapRequest =
    limine::request::MemoryMapRequest::new();

#[used]
#[link_section = ".requests"]
static HHDM_REQUEST: limine::request::HhdmRequest = limine::request::HhdmRequest::new();

#[used]
#[link_section = ".requests"]
static PAGING_REQUEST: limine::request::PagingModeRequest =
    limine::request::PagingModeRequest::new();

pub fn get_module<'a>(module_name: &str) -> Option<&'a File> {
    if MODULE_REQUEST.get_response().is_none() {
        panic!("Module request in none!");
    }
    let module_response = MODULE_REQUEST.get_response().unwrap();

    let mut file = None;

    for &module in module_response.modules() {
        let path = core::str::from_utf8(module.path());
        if path.is_err() {
            continue;
        }

        if !path.unwrap().contains(module_name) {
            continue;
        }

        file = Some(module);
    }

    return file;
}

pub fn get_rdsp_ptr() -> Option<*const ()> {
    return Some(RSDP_REQ.get_response()?.address());
}

pub fn get_smp<'a>() -> Option<&'a mut SmpResponse> {
    return unsafe { SMP_REQUEST.get_response_mut() };
}

pub fn get_framebuffer<'a>() -> Option<Framebuffer<'a>> {
    return FRAMEBUFFER_REQUEST.get_response()?.framebuffers().next();
}

pub static HHDM_OFFSET: OnceCell<usize> = OnceCell::new();

pub fn get_hhdm_offset() -> usize {
    if let Err(()) = HHDM_OFFSET.get() {
        HHDM_OFFSET.set(
            HHDM_REQUEST
                .get_response()
                .expect("Failed to get HHDM!")
                .offset() as usize,
        );
    }

    // Note: this clones the usize
    return *HHDM_OFFSET.get_unchecked();
}

pub fn get_memmap<'a>() -> &'a mut [&'a mut limine::memory_map::Entry] {
    return unsafe {
        MEMMAP_REQUEST
            .get_response_mut()
            .expect("Failed to get Memory map!")
            .entries_mut()
    };
}

pub fn get_kernel_address<'a>() -> &'a KernelAddressResponse {
    return KERNEL_ADDRESS_REQUEST.get_response().unwrap();
}

pub fn get_kernel_file<'a>() -> Option<&'a KernelFileResponse> {
    return KERNEL_REQUEST.get_response();
}

pub fn get_paging_level() -> Mode {
    return PAGING_REQUEST.get_response().unwrap().mode();
}
