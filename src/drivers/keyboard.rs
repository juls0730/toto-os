#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use crate::arch::interrupts::{idt_set_gate, InterruptIndex};
// Shitty keyboard driver
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use crate::arch::{
    interrupts,
    io::{inb, outb},
};
use core::sync::atomic::{AtomicBool, Ordering};

const KBD_DATA_PORT: u16 = 0x60;
const KBD_COMMAND_AND_STATUS_PORT: u16 = 0x64;

pub struct Key<'a> {
    pub pressed: bool,
    pub name: &'a str,
    pub character: Option<char>,
}

static EXTENDED_KEY: AtomicBool = AtomicBool::new(false);

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub extern "x86-interrupt" fn keyboard_interrupt_handler() {
    use crate::drivers::serial::write_serial;

    interrupts::signal_end_of_interrupt();

    let scancode = inb(KBD_DATA_PORT);

    let key = parse_key(scancode);

    if let Some(key) = key {
        // crate::usr::shell::handle_key(key)
        write_serial(key.character.unwrap() as u8);
    }
}

#[derive(Debug)]
pub enum KBDError {
    TestFailed,
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn init() -> Result<(), KBDError> {
    // flush output buffer
    while (inb(KBD_COMMAND_AND_STATUS_PORT) & 1) != 0 {
        inb(KBD_DATA_PORT);
    }

    // Disable PS/2 Devices (second then first)
    outb(KBD_COMMAND_AND_STATUS_PORT, 0xA7);
    outb(KBD_COMMAND_AND_STATUS_PORT, 0xAD);

    // TODO: Test the controller correctly

    idt_set_gate(
        InterruptIndex::Keyboard.as_u8(),
        crate::drivers::keyboard::keyboard_interrupt_handler as usize,
    );

    // Enable PS/2 Devices (second then first)
    outb(KBD_COMMAND_AND_STATUS_PORT, 0xA8);
    outb(KBD_COMMAND_AND_STATUS_PORT, 0xAE);

    // Reset Devices
    inb(KBD_COMMAND_AND_STATUS_PORT);

    return Ok(());
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn set_leds(led_byte: u8) {
    // Command bytes
    outb(KBD_DATA_PORT, 0xED);
    while inb(KBD_DATA_PORT) != 0xFA {}

    // Data byte
    outb(KBD_DATA_PORT, led_byte);
}

fn parse_key(mut scancode: u8) -> Option<Key<'static>> {
    if scancode == 0xE0 {
        EXTENDED_KEY.store(!EXTENDED_KEY.load(Ordering::SeqCst), Ordering::SeqCst);
        return None;
    }

    let pressed = scancode & 0x80 == 0x0;
    scancode &= !(1 << 7);

    let key: Option<Key<'static>> = match scancode {
        0x01 => Some(Key {
            pressed,
            name: "Esc",
            character: Some('\u{001B}'),
        }),
        0x02 => Some(Key {
            pressed,
            name: "1",
            character: Some('1'),
        }),
        0x03 => Some(Key {
            pressed,
            name: "2",
            character: Some('2'),
        }),
        0x04 => Some(Key {
            pressed,
            name: "3",
            character: Some('3'),
        }),
        0x05 => Some(Key {
            pressed,
            name: "4",
            character: Some('4'),
        }),
        0x06 => Some(Key {
            pressed,
            name: "5",
            character: Some('5'),
        }),
        0x07 => Some(Key {
            pressed,
            name: "6",
            character: Some('6'),
        }),
        0x08 => Some(Key {
            pressed,
            name: "7",
            character: Some('7'),
        }),
        0x09 => Some(Key {
            pressed,
            name: "8",
            character: Some('8'),
        }),
        0x0A => Some(Key {
            pressed,
            name: "9",
            character: Some('9'),
        }),
        0x0B => Some(Key {
            pressed,
            name: "0",
            character: Some('0'),
        }),
        0x0C => Some(Key {
            pressed,
            name: "-",
            character: Some('-'),
        }),
        0x0D => Some(Key {
            pressed,
            name: "=",
            character: Some('='),
        }),
        0x0E => Some(Key {
            pressed,
            name: "Backspace",
            character: Some('\u{0008}'),
        }),
        0x0F => Some(Key {
            pressed,
            name: "Tab",
            character: Some('\u{0009}'),
        }),
        0x10 => Some(Key {
            pressed,
            name: "q",
            character: Some('q'),
        }),
        0x11 => Some(Key {
            pressed,
            name: "w",
            character: Some('w'),
        }),
        0x12 => Some(Key {
            pressed,
            name: "e",
            character: Some('e'),
        }),
        0x13 => Some(Key {
            pressed,
            name: "r",
            character: Some('r'),
        }),
        0x14 => Some(Key {
            pressed,
            name: "t",
            character: Some('t'),
        }),
        0x15 => Some(Key {
            pressed,
            name: "y",
            character: Some('y'),
        }),
        0x16 => Some(Key {
            pressed,
            name: "u",
            character: Some('u'),
        }),
        0x17 => Some(Key {
            pressed,
            name: "i",
            character: Some('i'),
        }),
        0x18 => Some(Key {
            pressed,
            name: "o",
            character: Some('o'),
        }),
        0x19 => Some(Key {
            pressed,
            name: "p",
            character: Some('p'),
        }),
        0x1A => Some(Key {
            pressed,
            name: "[",
            character: Some('['),
        }),
        0x1B => Some(Key {
            pressed,
            name: "]",
            character: Some(']'),
        }),
        0x1C => Some(Key {
            pressed,
            name: "Enter",
            character: Some('\u{000A}'),
        }),
        0x1D => {
            if EXTENDED_KEY.load(Ordering::SeqCst) {
                return Some(Key {
                    pressed,
                    name: "RCtrl",
                    character: None,
                });
            }

            Some(Key {
                pressed,
                name: "LCtrl",
                character: None,
            })
        }
        0x1E => Some(Key {
            pressed,
            name: "a",
            character: Some('a'),
        }),
        0x1F => Some(Key {
            pressed,
            name: "s",
            character: Some('s'),
        }),
        0x20 => Some(Key {
            pressed,
            name: "d",
            character: Some('d'),
        }),
        0x21 => Some(Key {
            pressed,
            name: "f",
            character: Some('f'),
        }),
        0x22 => Some(Key {
            pressed,
            name: "g",
            character: Some('g'),
        }),
        0x23 => Some(Key {
            pressed,
            name: "h",
            character: Some('h'),
        }),
        0x24 => Some(Key {
            pressed,
            name: "j",
            character: Some('j'),
        }),
        0x25 => Some(Key {
            pressed,
            name: "k",
            character: Some('k'),
        }),
        0x26 => Some(Key {
            pressed,
            name: "l",
            character: Some('l'),
        }),
        0x27 => Some(Key {
            pressed,
            name: ";",
            character: Some(';'),
        }),
        0x28 => Some(Key {
            pressed,
            name: "'",
            character: Some('\''),
        }),
        0x29 => Some(Key {
            pressed,
            name: "`",
            character: Some('`'),
        }),
        0x2A => Some(Key {
            pressed,
            name: "LShift",
            character: None,
        }),
        0x2B => Some(Key {
            pressed,
            name: "\\",
            character: Some('\\'),
        }),
        0x2C => Some(Key {
            pressed,
            name: "z",
            character: Some('z'),
        }),
        0x2D => Some(Key {
            pressed,
            name: "x",
            character: Some('x'),
        }),
        0x2E => Some(Key {
            pressed,
            name: "c",
            character: Some('c'),
        }),
        0x2F => Some(Key {
            pressed,
            name: "v",
            character: Some('v'),
        }),
        0x30 => Some(Key {
            pressed,
            name: "b",
            character: Some('b'),
        }),
        0x31 => Some(Key {
            pressed,
            name: "n",
            character: Some('n'),
        }),
        0x32 => Some(Key {
            pressed,
            name: "m",
            character: Some('m'),
        }),
        0x33 => Some(Key {
            pressed,
            name: ",",
            character: Some(','),
        }),
        0x34 => Some(Key {
            pressed,
            name: ".",
            character: Some('.'),
        }),
        0x35 => Some(Key {
            pressed,
            name: "/",
            character: Some('/'),
        }),
        0x36 => Some(Key {
            pressed,
            name: "RShift",
            character: None,
        }),
        0x37 => Some(Key {
            pressed,
            name: "*",
            character: Some('*'),
        }),
        0x38 => Some(Key {
            pressed,
            name: "Alt",
            character: None,
        }),
        0x39 => Some(Key {
            pressed,
            name: " ",
            character: Some(' '),
        }),
        0x3A => Some(Key {
            pressed,
            name: "CapsLock",
            character: None,
        }),
        0x3B => Some(Key {
            pressed,
            name: "F1",
            character: None,
        }),
        0x3C => Some(Key {
            pressed,
            name: "F2",
            character: None,
        }),
        0x3D => Some(Key {
            pressed,
            name: "F3",
            character: None,
        }),
        0x3E => Some(Key {
            pressed,
            name: "F4",
            character: None,
        }),
        0x3F => Some(Key {
            pressed,
            name: "F5",
            character: None,
        }),
        0x40 => Some(Key {
            pressed,
            name: "F6",
            character: None,
        }),
        0x41 => Some(Key {
            pressed,
            name: "F7",
            character: None,
        }),
        0x42 => Some(Key {
            pressed,
            name: "F8",
            character: None,
        }),
        0x43 => Some(Key {
            pressed,
            name: "F9",
            character: None,
        }),
        0x44 => Some(Key {
            pressed,
            name: "F10",
            character: None,
        }),
        0x45 => Some(Key {
            pressed,
            name: "NumLock",
            character: None,
        }),
        0x46 => Some(Key {
            pressed,
            name: "ScrLock",
            character: None,
        }),
        0x47 => {
            if EXTENDED_KEY.load(Ordering::SeqCst) {
                return Some(Key {
                    pressed,
                    name: "Home",
                    character: None,
                });
            }

            Some(Key {
                pressed,
                name: "Keypad 7",
                character: Some('7'),
            })
        }
        0x48 => {
            if EXTENDED_KEY.load(Ordering::SeqCst) {
                return Some(Key {
                    pressed,
                    name: "CurUp",
                    character: None,
                });
            }

            Some(Key {
                pressed,
                name: "Keypad 8",
                character: Some('8'),
            })
        }
        0x49 => {
            if EXTENDED_KEY.load(Ordering::SeqCst) {
                return Some(Key {
                    pressed,
                    name: "PgUp",
                    character: None,
                });
            }

            Some(Key {
                pressed,
                name: "Keypad 9",
                character: Some('9'),
            })
        }
        0x4A => Some(Key {
            pressed,
            name: "-",
            character: Some('-'),
        }),
        0x4B => {
            if EXTENDED_KEY.load(Ordering::SeqCst) {
                return Some(Key {
                    pressed,
                    name: "CurLeft",
                    character: None,
                });
            }

            Some(Key {
                pressed,
                name: "Keypad 4",
                character: Some('4'),
            })
        }
        0x4C => Some(Key {
            pressed,
            name: "Keypad 5",
            character: Some('5'),
        }),
        0x4D => {
            if EXTENDED_KEY.load(Ordering::SeqCst) {
                return Some(Key {
                    pressed,
                    name: "CurRight",
                    character: None,
                });
            }

            Some(Key {
                pressed,
                name: "Keypad 6",
                character: Some('6'),
            })
        }
        0x4E => Some(Key {
            pressed,
            name: "+",
            character: Some('+'),
        }),
        0x4F => {
            if EXTENDED_KEY.load(Ordering::SeqCst) {
                return Some(Key {
                    pressed,
                    name: "End",
                    character: None,
                });
            }

            Some(Key {
                pressed,
                name: "Keypad 1",
                character: Some('1'),
            })
        }
        0x50 => {
            if EXTENDED_KEY.load(Ordering::SeqCst) {
                return Some(Key {
                    pressed,
                    name: "CurDown",
                    character: None,
                });
            }

            Some(Key {
                pressed,
                name: "Keypad 2",
                character: Some('2'),
            })
        }
        0x51 => {
            if EXTENDED_KEY.load(Ordering::SeqCst) {
                return Some(Key {
                    pressed,
                    name: "PgDn",
                    character: None,
                });
            }

            Some(Key {
                pressed,
                name: "Keypad 3",
                character: Some('3'),
            })
        }
        0x52 => {
            if EXTENDED_KEY.load(Ordering::SeqCst) {
                return Some(Key {
                    pressed,
                    name: "Insert",
                    character: None,
                });
            }

            Some(Key {
                pressed,
                name: "Keypad 0",
                character: Some('0'),
            })
        }
        0x53 => {
            if EXTENDED_KEY.load(Ordering::SeqCst) {
                return Some(Key {
                    pressed,
                    name: "Del",
                    character: None,
                });
            }

            Some(Key {
                pressed,
                name: "Keypad .",
                character: Some('.'),
            })
        }
        0x57 => Some(Key {
            pressed,
            name: "F11",
            character: None,
        }),
        0x58 => Some(Key {
            pressed,
            name: "F12",
            character: None,
        }),
        _ => None,
    };

    return key;
}
