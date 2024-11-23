use core::arch::asm;

pub const PS2_DATA_PORT: u16 = 0x60;
pub const PS2_STATUS_PORT: u16 = 0x64;
pub const PS2_OUTPUT_BUFFER_STATUS_BIT: u8 = 1;

/// Reads from `PS2_STATUS_PORT` and returns the extracted value.
fn status() -> u8 {
    let res: u8;

    unsafe {
        res = read(PS2_STATUS_PORT);
    }

    res
}

/// Returns `true` if the least significant bit of the ps2 status port is set,
/// meaning it has been written to.
fn buffer_full() -> bool {
    status() & PS2_OUTPUT_BUFFER_STATUS_BIT != 0
}

/// Reads from the PS2 data port if the PS2 status port is ready. Returns `Some(char)`
/// if the converted scancode is a supported character.
pub fn read_if_ready() -> Option<char> {
    if !buffer_full() {
        return None;
    }

    let code = unsafe { read(PS2_DATA_PORT) };

    if let Some(char) = SCANCODE_TO_ASCII.get(code as usize).and_then(|&opt| opt) {
        return Some(char);
    }

    None
}

/// Reads from `port` and returns the extracted value.
/// ## SAFETY:
/// `port` is assumed to be one of `PS2_STATUS_PORT` or `PS2_DATA_PORT`. Passing another value
/// to this function will result in undefines behavior.
unsafe fn read(port: u16) -> u8 {
    let res: u8;

    asm!(
        "in al, dx",
        in("dx") port,
        out("al") res,
    );

    res
}

pub const BACKSPACE: char = 14 as char;
pub const ENTER: char = 28 as char;

/// Conversion table for all characters currently supported by our kernel for PS2 input.
const SCANCODE_TO_ASCII: [Option<char>; 58] = [
    None,
    None,
    Some('1'),
    Some('2'),
    Some('3'),
    Some('4'),
    Some('5'),
    Some('6'),
    Some('7'),
    Some('8'),
    Some('9'),
    Some('0'),
    Some('-'),
    Some('='),
    Some(BACKSPACE),
    Some('\t'),
    Some('q'),
    Some('w'),
    Some('e'),
    Some('r'),
    Some('t'),
    Some('y'),
    Some('u'),
    Some('i'),
    Some('o'),
    Some('p'),
    Some('['),
    Some(']'),
    Some(ENTER),
    None,
    Some('a'),
    Some('s'),
    Some('d'),
    Some('f'),
    Some('g'),
    Some('h'),
    Some('j'),
    Some('k'),
    Some('l'),
    Some(';'),
    Some('\''),
    Some('`'),
    None,
    Some('\\'),
    Some('z'),
    Some('x'),
    Some('c'),
    Some('v'),
    Some('b'),
    Some('n'),
    Some('m'),
    Some(','),
    Some('.'),
    Some('/'),
    None,
    Some('*'),
    None,
    Some(' '),
];
