use crate::{libs::cell::OnceCell, mem::VirtualPtr};

#[derive(Clone, Copy, Debug)]
pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub bpp: usize,
    pub pitch: usize,
    pub pointer: VirtualPtr<u8>,
}

impl Framebuffer {
    #[inline]
    const fn new(
        bpp: usize,
        pitch: usize,
        ptr: VirtualPtr<u8>,
        width: usize,
        height: usize,
    ) -> Self {
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
            self.pointer.offset(pixel_offset).cast::<u32>().write(color);
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
                self.pointer.cast::<u32>().as_raw_ptr(),
                buffer.len(),
            );

            if let Some(mirror_buffer) = mirror_buffer {
                core::ptr::copy_nonoverlapping(
                    buffer.as_ptr(),
                    mirror_buffer.pointer.cast::<u32>().as_raw_ptr(),
                    buffer.len(),
                );
            }
        };
    }
}

pub static FRAMEBUFFER: OnceCell<Option<Framebuffer>> = OnceCell::new();

pub fn get_framebuffer() -> Option<Framebuffer> {
    *FRAMEBUFFER.get_or_set(|| {
        let limine_frambuffer = crate::libs::limine::get_framebuffer()?;

        let framebuffer = Framebuffer::new(
            limine_frambuffer.bpp() as usize,
            limine_frambuffer.pitch() as usize,
            unsafe { VirtualPtr::new(limine_frambuffer.addr()) },
            limine_frambuffer.width() as usize,
            limine_frambuffer.height() as usize,
        );

        return Some(framebuffer);
    })
}
