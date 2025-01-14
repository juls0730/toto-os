use alloc::{borrow::ToOwned, string::String, vec::Vec};

use crate::{
    drivers::fs::vfs::{vfs_open, UserCred},
    log,
    mem::VirtualPtr,
    LogLevel,
};

// use crate::drivers::fs::vfs::VfsFileSystem;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct StackFrame {
    back: usize,
    rip: usize,
}

pub fn print_stack_trace(max_frames: usize, rbp: u64) {
    let mut stackframe: VirtualPtr<StackFrame> = VirtualPtr::from(rbp as usize);
    let mut frames_processed = 0;

    log!(LogLevel::Info, "{:─^width$}", " Stack Trace ", width = 98);
    for _ in 0..max_frames {
        frames_processed += 1;

        if stackframe.is_null() {
            break;
        }

        let instruction_ptr = unsafe { (stackframe.read()).rip };

        if instruction_ptr < crate::libs::limine::get_hhdm_offset() {
            stackframe = unsafe { VirtualPtr::from((stackframe.read()).back) };
            continue;
        }

        let instruction_info = get_function_name(instruction_ptr as u64);

        let address_info = if let Ok((function_name, function_offset)) = instruction_info {
            &alloc::format!("<{}+{:#X}>", function_name, function_offset)
        } else {
            ""
        };

        log!(LogLevel::Info, "{:#X} {address_info}", instruction_ptr);

        stackframe = unsafe { VirtualPtr::from((stackframe.read()).back) };
    }

    if frames_processed == max_frames && !stackframe.is_null() {
        log!(LogLevel::Info, "... <frames omitted>");
    }
}

fn get_function_name(function_address: u64) -> Result<(String, u64), ()> {
    // TODO: dont rely on initramfs being mounted at /
    let symbols_fd = vfs_open("/symbols.table")?;

    let symbols_table_bytes = symbols_fd
        .open(0, UserCred { uid: 0, gid: 0 })
        .read_all(0, 0)?;
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

    return Err(());
}
