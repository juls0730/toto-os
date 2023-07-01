use limine::LimineFramebufferRequest;
use core::ascii;

static FRAMEBUFFER_REQUEST: LimineFramebufferRequest = LimineFramebufferRequest::new(0);

pub fn init_video() {
	put_char(62, 0, 0, 0xFFFFFF, 0x000000);
	put_char(32, 1, 0, 0xFFFFFF, 0x000000);
}

pub fn fill_screen(color: u32) {
	if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response().get() {
		if framebuffer_response.framebuffer_count < 1 {
			return;
		}

		let framebuffer = &framebuffer_response.framebuffers()[0];
		
		for x in 0..framebuffer.width {
			for y in 0..framebuffer.height {
				put_pixel(x as u32, y as u32, color);
			}
		}
	}
}

pub fn write_string(string: &str) {
	let mut column = 0;
	for (i, character) in string.encode_utf16().enumerate() {
		put_char(character as u8, column, 0, 0xFFFFFF, 0x000000);
		column += 1;
	}
}

pub fn put_char(character: u8, cx: u16, cy: u16, fg: u32, bg: u32) {
	let font = crate::libs::font::G_8X16_FONT;

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