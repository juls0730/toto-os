mod font;

use limine::LimineFramebufferRequest;

static FRAMEBUFFER_REQUEST: LimineFramebufferRequest = LimineFramebufferRequest::new(0);

pub fn init_video() {
	put_char(62, 0, 0, 0xFFFFFF, 0x000000);
	put_char(32, 1, 0, 0xFFFFFF, 0x000000);
}

// This is slow, but significantly faster than filling the framebuffer pixel-by-pixel with for loops.
// idk, fix it later ig.
pub fn fill_screen(color: u32) {
	if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response().get() {
		if framebuffer_response.framebuffer_count < 1 {
			return;
		}

		let framebuffer = &framebuffer_response.framebuffers()[0];
		let buffer_size = (framebuffer.pitch * framebuffer.width) as usize;

		let buffer = unsafe {
			core::slice::from_raw_parts_mut(framebuffer.address.as_ptr().unwrap() as *mut u32, buffer_size)
		};

		buffer.fill(color);
	}
}

pub fn put_char(character: u8, cx: u16, cy: u16, fg: u32, bg: u32) {
	let font = font::G_8X16_FONT;

	let character_array = font[character as usize];

	for row in 0..character_array.len() {
		let character_byte = character_array[row as usize];
		for col in 0..8 {
			let pixel = (character_byte >> (7 - col)) & 0x01;

			let x = (cx * 8 + col) as u32;
			let y = (cy * 16 + row as u16) as u32;

			if pixel == 1 {
				put_pixel(x, y, fg);
			} else {
				put_pixel(x, y, bg);
			}
		}
	}
}

struct Cursor {
	cx: u16,
	cy: u16,
	fg: u32,
}

static mut CURSOR: Cursor = Cursor{ 
	cx: 0, 
	cy: 0,
	fg: 0xbababa
};

pub fn puts(string: &str) {
	if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response().get() {
		let framebuffer = &framebuffer_response.framebuffers()[0];

		for (_i, character) in string.chars().enumerate() {
			unsafe {
				if CURSOR.cx == (framebuffer.width / 8) as u16 {
					CURSOR.cy += 1;
					CURSOR.cx = 0;
				}
				// Newline character
				if character as u8 == 10 {
					CURSOR.cx = 0;
					CURSOR.cy += 1;
				} else {
					put_char(character as u8, CURSOR.cx, CURSOR.cy, CURSOR.fg, 0x000000);
					CURSOR.cx += 1;
				}
			}
		}
	}
}

pub fn set_color(color: u32) {
	unsafe {
		CURSOR.fg = color;
	}
}

pub fn put_pixel(x: u32, y: u32, color: u32) {
	if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response().get() {
		if framebuffer_response.framebuffer_count < 1 {
			return;
		}

		let framebuffer = &framebuffer_response.framebuffers()[0];

		unsafe {
			// let pixel_offset: *mut u32 = (y * (*g_vbe).pitch as u32 + (x * ((*g_vbe).bpp/8) as u32) + (*g_vbe).framebuffer) as *mut u32;
			*(framebuffer.address.as_ptr().unwrap().offset((y * framebuffer.pitch as u32 + (x * (framebuffer.bpp/8) as u32)) as isize) as *mut u32) = color;
		}
	}
}