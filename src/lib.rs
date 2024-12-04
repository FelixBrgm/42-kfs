#![no_std]

mod gdt;
mod panic;
mod print;
mod terminal;

#[no_mangle]
pub extern "C" fn kernel_main() {
    let mut t = terminal::Terminal::new();
    t.write(b'4');
    t.write(b'2');
    t.write(b'\n');
    t.flush();
    loop {
        if let Some(key) = terminal::ps2::read_if_ready() {
            t.handle_key(key);
            t.flush();
        }
    }
}
