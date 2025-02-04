use core::arch::asm;

use crate::{
    conv::hextou,
    terminal::{
        ps2::{self, read_if_ready, Key},
        vga::Buffer,
        Screen,
    },
};

const PROMPT_MAX_LENGTH: usize = 1000;

pub fn launch(s: &mut Screen) {
    let mut prompt_start: usize;

    loop {
        s.write_str("sh> ");
        flush(s);

        prompt_start = s.cursor;

        loop {
            if let Some(key) = ps2::read_if_ready() {
                match key {
                    Key::Enter => {
                        let mut prompt: [u8; PROMPT_MAX_LENGTH] = [0; PROMPT_MAX_LENGTH];
                        s.move_cursor_to_end();
                        for (place, data) in prompt.iter_mut().zip(s.buffer[prompt_start..s.cursor].iter()) {
                            *place = (*data & 0xFF) as u8
                        }
                        s.handle_key(key);
                        prompt_execute(&prompt, s);
                        break;
                    }
                    Key::ArrowLeft | Key::Backspace => {
                        if prompt_start < s.cursor {
                            s.handle_key(key);
                        }
                    }
                    Key::Escape => {
                        reboot_cmd(&[], s);
                    }
                    _ => s.handle_key(key),
                }
                flush(s);
            }
        }
    }
}

fn flush(s: &mut Screen) {
    let b: Buffer = Buffer::from_screen(s);
    b.flush();
}

struct Command<'a> {
    name: &'a str,
    func: fn(args: &[u8], s: &mut Screen),
}

fn prompt_execute(prompt: &[u8], s: &mut Screen) {
    static COMMANDS: &[Command] = &[
        Command { name: "echo", func: echo_cmd },
        Command {
            name: "panic",
            func: panic_cmd,
        },
        Command { name: "halt", func: halt_cmd },
        Command {
            name: "reboot",
            func: reboot_cmd,
        },
        Command {
            name: "prints",
            func: prints_cmd,
        },
        Command { name: "help", func: help_cmd },
    ];

    let cmd_end = match prompt.iter().position(|&c| c == b' ' || c == 0) {
        Some(pos) => pos,
        None => prompt.len(),
    };
    // TODO: add a way to get the total prompt length from the prompt (`prompt.len()` does not work since the prompt
    // is padded with trailing zeros).
    let prompt_len = match prompt.iter().position(|&c| c == 0) {
        Some(pos) => pos,
        None => prompt.len(),
    };

    let cmd = &prompt[..cmd_end];

    for command in COMMANDS {
        if cmd == command.name.as_bytes() {
            let args = if cmd_end < prompt_len { &prompt[cmd_end + 1..] } else { &[] };
            (command.func)(args, s);
            return;
        }
    }
    s.write_str("'");
    for byte in &cmd[..cmd_end] {
        s.write(*byte);
    }
    s.write_str("': command not found\n");
}

#[allow(unused)]
fn help_cmd(args: &[u8], s: &mut Screen) {
    s.write_str("\nAvailable commands:\n\n");
    s.write_str("    echo:                echoes input to the console\n");
    s.write_str("    panic:               trigger a kernel panic\n");
    s.write_str("    halt:                halt the kernel execution\n");
    s.write_str("    reboot:              reboot the kernel\n");
    s.write_str("    prints <address>:    display 1024 bytes of memory starting from <address>\n");
    s.write_str("    prints               display the kernel stack boundaries\n");
    s.write_str("    help                 display this help message\n\n");
}

fn contains_non_null(bytes: &[u8]) -> bool {
    for byte in bytes {
        if *byte != 0 {
            return true;
        }
    }
    false
}

fn print_stack_slice(addr: usize, s: &mut Screen) {
    let ptr: *const u8 = addr as *const u8;

    for row_idx in (addr..(addr + 1024)).step_by(16) {
        let mut bytes: [u8; 16] = [0u8; 16];

        #[allow(clippy::needless_range_loop)]
        for byte_idx in 0..16 {
            let byte = unsafe { *ptr.add(row_idx + byte_idx) };
            bytes[byte_idx] = byte;
        }

        if contains_non_null(&bytes) {
            s.write_str("0x");
            s.write_hex((addr + row_idx) as u32);
            s.write_str("-0x");
            s.write_hex((addr + row_idx + 15) as u32);
            s.write_str(": ");

            for word in bytes.chunks(4) {
                s.write_str("0x");
                for b in word {
                    s.write_hex_byte(*b);
                }
                s.write_str(" ");
            }
            s.write_str("\n");
            flush(s);
        }
    }

    s.write_str("\n1024 bytes displayed by rows of 16. Zeroed out rows omitted.\n");
}

extern "C" {
    static stack_top: u8;
}

fn prints_cmd(args: &[u8], s: &mut Screen) {
    let sp: usize;
    #[cfg(not(test))]
    unsafe {
        asm!(
            "mov {0}, esp",
            out(reg) sp,
        )
    }
    #[cfg(test)]
    unsafe {
        asm!(
            "mov {0}, rsp",
            out(reg) sp,
        )
    }

    if args.is_empty() || args.iter().all(|&c| c == b' ' || c == 0) {
        s.write_str("ESP: 0x");
        s.write_hex(sp as u32);
        s.write_str(" STACK_TOP: 0x");
        unsafe {
            s.write_hex(&stack_top as *const u8 as u32);
        }
        s.write_str("\n");
    } else {
        let addr = match hextou(args) {
            Some(a) => a,
            None => {
                s.write_str("No valid hex found in input\n");
                return;
            }
        };
        print_stack_slice(addr, s);
    }
}

#[allow(unused)]
fn echo_cmd(args: &[u8], s: &mut Screen) {
    let args_len = match args.iter().position(|&c| c == 0) {
        Some(pos) => pos,
        None => args.len(),
    };

    for byte in &args[..args_len] {
        s.write(*byte);
    }
    s.write_str("\n");
}

fn reboot_cmd(args: &[u8], s: &mut Screen) {
    while read_if_ready().is_some() {}

    unsafe { asm!("out dx, al", in("dx") 0x64, in("al") 0xFEu8) };

    halt_cmd(args, s);
}

#[allow(unused)]
fn halt_cmd(args: &[u8], s: &mut Screen) {
    unsafe { asm!("hlt") }
}

#[allow(unused)]
fn panic_cmd(args: &[u8], s: &mut Screen) {
    panic!()
}
