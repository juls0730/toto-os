use limine::{framebuffer, request::FramebufferRequest};

use crate::libs::cell::OnceCell;

#[derive(Clone, Copy, Debug)]
pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub bpp: usize,
    pub pitch: usize,
    pub pointer: *mut u8,
}

impl Framebuffer {
    #[inline]
    const fn new(bpp: usize, pitch: usize, ptr: *mut u8, width: usize, height: usize) -> Self {
        return Self {
            width,
            height,
            bpp,
            pitch,
            pointer: ptr,
        };
    }

    // Returns the size of the framebuffer in bytes
    pub fn len(&self) -> usize {
        return self.pitch * self.height;
    }

    pub fn put_pixel(&self, x: u32, y: u32, color: u32) {
        let pixel_offset = (y * self.pitch as u32 + (x * (self.bpp / 8) as u32)) as isize;

        unsafe {
            *(self.pointer.offset(pixel_offset).cast::<u32>()) = color;
        }
    }

    // This is slow, but significantly faster than filling the framebuffer pixel-by-pixel with for loops.
    // idk, fix it later ig.
    pub fn fill_screen(&self, color: u32, mirror_buffer: Option<Self>) {
        let buffer_size = (self.pitch / (self.bpp / 8)) * self.height;

        unsafe {
            if let Some(mirror_buffer) = mirror_buffer {
                crate::mem::memset32(mirror_buffer.pointer.cast::<u32>(), color, buffer_size);
            }

            crate::mem::memset32(self.pointer.cast::<u32>(), color, buffer_size);
        }
    }

    pub fn blit_screen(&self, buffer: &mut [u32], mirror_buffer: Option<Self>) {
        unsafe {
            core::ptr::copy_nonoverlapping(
                buffer.as_ptr(),
                self.pointer.cast::<u32>(),
                buffer.len(),
            );

            if let Some(mirror_buffer) = mirror_buffer {
                core::ptr::copy_nonoverlapping(
                    buffer.as_ptr(),
                    mirror_buffer.pointer.cast::<u32>(),
                    buffer.len(),
                );
            }
        };
    }
}

#[used]
#[link_section = ".requests"]
pub static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();
pub static FRAMEBUFFER: OnceCell<Option<Framebuffer>> = OnceCell::new();

pub fn get_framebuffer() -> Option<Framebuffer> {
    *FRAMEBUFFER.get_or_set(|| {
        let framebuffer_response = crate::drivers::video::FRAMEBUFFER_REQUEST.get_response()?;
        let framebuffer = framebuffer_response.framebuffers().next();

        if framebuffer.is_none() {
            return None;
        }

        let framebuffer_response = framebuffer.as_ref().unwrap();

        let framebuffer = Framebuffer::new(
            framebuffer_response.bpp() as usize,
            framebuffer_response.pitch() as usize,
            framebuffer_response.addr(),
            framebuffer_response.width() as usize,
            framebuffer_response.height() as usize,
        );

        return Some(framebuffer);
    })
}
