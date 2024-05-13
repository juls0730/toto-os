use alloc::{borrow::ToOwned, string::String, vec::Vec};

use crate::drivers::fs::vfs::vfs_open;

// use crate::drivers::fs::vfs::VfsFileSystem;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct StackFrame {
    back: *const StackFrame,
    rip: u64,
}

pub fn print_stack_trace(max_frames: usize, rbp: u64) {
    let mut stackframe = rbp as *const StackFrame;

    crate::println!("Stack Trace:");
    for _frame in 0..max_frames {
        if stackframe.is_null() || unsafe { (*stackframe).back.is_null() } {
            break;
        }

        let instruction_pointer = unsafe { (*stackframe).rip };

        if instruction_pointer == 0x0 {
            unsafe {
                stackframe = (*stackframe).back;
            };
            continue;
        }

        crate::print!("  {:#X} ", instruction_pointer);

        let instrcution_info = get_function_name(instruction_pointer);

        if let Ok((function_name, function_offset)) = instrcution_info {
            crate::println!("<{}+{:#X}>", function_name, function_offset);
        } else {
            crate::println!();
        }

        unsafe {
            stackframe = (*stackframe).back;
        };
    }
}

fn get_function_name(function_address: u64) -> Result<(String, u64), ()> {
    // TODO: dont rely on initramfs being mounted at /
    let mut symbols_fd = vfs_open("/symbols.table")?;

    let symbols_table_bytes = symbols_fd.ops.open(
        0,
        crate::drivers::fs::vfs::UserCred { uid: 0, gid: 0 },
        symbols_fd.as_ptr(),
    )?;
    let symbols_table = core::str::from_utf8(&symbols_table_bytes).ok().ok_or(())?;

    let mut previous_symbol: Option<(&str, u64)> = None;

    let symbols_table_lines: Vec<&str> = symbols_table.lines().collect();

    for (i, line) in symbols_table_lines.iter().enumerate() {
        let line_parts: Vec<&str> = line.splitn(2, ' ').collect();

        if line_parts.len() < 2 {
            continue;
        }

        let (address, function_name) = (
            u64::from_str_radix(line_parts[0], 16).ok().ok_or(())?,
            line_parts[1],
        );

        if address == function_address {
            return Ok((function_name.to_owned(), 0));
        }

        if i == symbols_table_lines.len() - 1 {
            return Ok((function_name.to_owned(), function_address - address));
        }

        if i == 0 {
            if function_address < address {
                return Err(());
            }

            previous_symbol = Some((function_name, address));
            continue;
        }

        if let Some(prev_symbol) = previous_symbol {
            if function_address > prev_symbol.1 && function_address < address {
                // function is previous symbol
                return Ok((prev_symbol.0.to_owned(), address - prev_symbol.1));
            }
        }

        previous_symbol = Some((function_name, address));
    }

    unreachable!();
}
