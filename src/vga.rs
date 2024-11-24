use core::{arch::asm, ptr::write_volatile};

#[cfg(test)]
use spin::Mutex;

#[repr(u8)]
#[allow(unused)]
#[derive(Debug, Clone, Copy)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGrey = 7,
    DarkGrey = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    LightMagenta = 13,
    LightBrown = 14,
    White = 15,
}

impl Color {
    pub fn to_foreground(self) -> u8 {
        self as u8
    }

    pub fn to_background(self) -> u8 {
        (self as u8) << 4
    }
}

pub const VGA_WIDTH: u8 = 80;
pub const VGA_HEIGHT: u8 = 25;
pub const VGA_BUFFER_SIZE: u16 = (VGA_WIDTH as u16) * (VGA_HEIGHT as u16);
pub const MAX_BUFFERED_LINES: u8 = 100;

#[cfg(test)]
static mut VGA_BUFFER_ADDR: [u16; VGA_WIDTH as usize * VGA_HEIGHT as usize] = [0; VGA_WIDTH as usize * VGA_HEIGHT as usize];

#[cfg(not(test))]
const VGA_BUFFER_ADDR: *mut u16 = 0xB8000 as *mut u16;

#[cfg(test)]
#[allow(static_mut_refs)]
fn get_vga_buffer_ptr() -> *mut u16 {
    unsafe { VGA_BUFFER_ADDR.as_mut_ptr() }
}

#[derive(PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

pub struct OutOfBoundsError;

/// Abstraction for managing the [Text-mode cursor](https://wiki.osdev.org/Text_Mode_Cursor).
#[derive(Clone, Copy)]
pub struct Cursor {}

#[allow(unused)]
impl Cursor {
    const LOCATION_REG_LOW: u8 = 0x0F;
    const LOCATION_REG_HIGH: u8 = 0x0E;
    const REG_START: u8 = 0x0A;
    const REG_END: u8 = 0x0B;

    /// Updates the text-mode cursor position in the VGA buffer by setting the CRTC's
    /// [location registers](http://www.osdever.net/FreeVGA/vga/crtcreg.htm#0F) (`0x0F` and `0x0D`)
    /// to `x, y`.
    ///
    /// ## SAFETY
    /// 1.  This function uses `Cursor::update`, which writes directly to the VGA buffer. In user-mode, this **will** result
    ///     in invalid memory access.
    ///
    /// 2.  `update_pos` may cause undefined behavior if called with `x` or `y` values outside of the range `0x00..=0x0F`.
    pub unsafe fn update_pos(&self, x: u16, y: u16) {
        let out_of_bounds: bool = !(0..VGA_HEIGHT).contains(&(y as u8)) || !(0..VGA_WIDTH).contains(&(x as u8));
        if out_of_bounds {
            return;
        }

        let pos = y * VGA_WIDTH as u16 + x;

        self.update(Cursor::LOCATION_REG_LOW, (pos & 0xFF) as u8);
        self.update(Cursor::LOCATION_REG_HIGH, ((pos >> 8) & 0xFF) as u8);
    }

    /// Resizes the cursor by updating the [cursor end & start register](http://www.osdever.net/FreeVGA/vga/crtcreg.htm#0A)
    /// (`0x0A` and `0x0B`) to `start, end`. The values of `start` and `end` are expected to be in the range `0x00..=0x0F`.
    ///
    /// ## SAFETY
    /// 1.  This function uses `Cursor::update`, which writes directly to the VGA buffer. In user-mode, this **will** result
    ///     in invalid memory access.
    ///
    /// 2.  `resize` may cause undefined behavior if called with `start` or `end` values outside of the range `0x00..=0x0F`.
    pub unsafe fn resize(&self, start: u8, end: u8) {
        self.update(Cursor::REG_START, start);
        self.update(Cursor::REG_END, end);
    }

    /// Abstraction for the ugliness behind updating the cursor.
    ///
    /// `0x3D4` is the I/O port address for the VGA's CRTC ([Cathode-ray tube](https://en.wikipedia.org/wiki/Cathode-ray_tube))'s
    /// index register. The value being loaded into it defines which CRTC functionality we want to access.
    /// The different indices that can be loaded into it are documented [here](http://www.osdever.net/FreeVGA/vga/).
    ///
    /// After the index has been loaded into the `0x3D4`, `dx`, (where the index register is stored) can be incremented by
    /// one. This will move it to `0x3D5`, the CRTC's data register, signifying the CRTC's readiness to receive the input values.
    ///
    /// ## SAFETY:
    /// This writes to the VGA buffer directly, running this in a non-bare-metal environment
    /// will result in invalid memory access.
    unsafe fn update(&self, index: u8, value: u8) {
        asm!(
            "mov dx, 0x3D4",
            "mov al, {index}",
            "out dx, al",
            "inc dx",
            "mov al, {value}",
            "out dx, al",
            index = in(reg_byte) (index),
            value = in(reg_byte) (value),
            out("dx") _,
            out("al") _,
        )
    }
}

#[derive(Clone, Copy)]
/// Buffer implementation for storing content beyond the VGA buffer size of 4000 bytes (80 x 25
/// u16 entries).
///
/// Intended to be used in the `Vga` implementation. Can be written to, and flushed into the VGA buffer
/// using `Vga::flush`.
pub struct Buffer {
    buf: [u16; (VGA_WIDTH as usize) * (MAX_BUFFERED_LINES as usize)],
}

impl Buffer {
    const NEWLINE: u8 = 0xFF;

    /// Returns a new `Buffer` object with a buffer size of `VGA_WIDTH * MAX_BUFFERED_LINES`. `VGA_WIDTH`
    /// is fixed to 80 (hardware limitation), and `MAX_BUFFERED_LINES` can be increased freely as long as enough memory
    /// is available.
    pub fn new() -> Self {
        Self {
            buf: [0u16; (VGA_WIDTH as usize) * (MAX_BUFFERED_LINES as usize)],
        }
    }

    /// Writes `entry` to `self.buf[line_offset * VGA_WIDTH + rel_index]`.
    /// ### Note
    /// This does **not** write to the VGA buffer, only to the internal one. Writes to the VGA buffer are to be handled
    /// by `Vga::flush`.
    pub fn write(&mut self, line_offset: u8, rel_index: u16, entry: u16) {
        let abs_index: usize = ((line_offset as usize * VGA_WIDTH as usize) + rel_index as usize) % self.buf.len();

        self.buf[abs_index] = entry;
    }

    /// Returns a `self.buf` slice of `VGA_BUFFER_SIZE` starting at `line_offset`.
    pub fn slice(&self, line_offset: u8) -> &[u16] {
        let start = (line_offset as usize) * (VGA_WIDTH as usize);
        let end = start + VGA_BUFFER_SIZE as usize;

        &self.buf[start..end]
    }

    pub fn at(&self, pos: u16) -> Option<&u16> {
        self.buf.get(pos as usize)
    }

    /// Returns the length of the written content starting from `from_x, from_y`, until either
    /// the next newline, or if no newline is found, until the next null VGA entry, i.e the next
    /// entry for which `(x & 0xFF) == 0` is true.
    pub fn block_length(&self, from_x: u8, from_y: u8) -> u16 {
        let slice = &self.buf[from_y as usize * VGA_WIDTH as usize + from_x as usize..];
        if let Some(ind) = slice.iter().position(|x| *x == Buffer::NEWLINE as u16) {
            return ind as u16;
        }
        slice.iter().position(|x| (*x & 0xFF) == 0).unwrap() as u16
    }
}

#[derive(Clone, Copy)]
/// Abstraction for VGA buffer interactions.
pub struct Vga {
    color: u8,
    x: u8,
    y: u8,
    cursor: Cursor,
    buffer: Buffer,
    line_offset: u8,
}

impl Default for Vga {
    fn default() -> Self {
        Self::new()
    }
}

impl Vga {
    pub fn new() -> Self {
        let mut t = Vga {
            color: 0,
            x: 0,
            y: 0,
            cursor: Cursor {},
            buffer: Buffer::new(),
            line_offset: 0,
        };

        t.set_foreground_color(Color::White);
        t.set_background_color(Color::Black);

        #[cfg(not(test))]
        unsafe {
            t.cursor.update_pos(0, 0);
            t.cursor.resize(0x0D, 0x0F);
        }

        t
    }

    pub fn move_cursor(&mut self, dir: Direction) {
        match dir {
            Direction::Up => self.y = (self.y - 1).max(0),
            Direction::Down => self.y = (self.y + 1).min(MAX_BUFFERED_LINES),
            Direction::Left => self.x = (self.x - 1).max(0),
            Direction::Right => self.x = (self.x + 1).min(VGA_WIDTH - 1),
        }

        unsafe {
            self.cursor.update_pos(self.x as u16, self.y as u16);
        }
    }

    /// Writes a character to the VGA buffer at `self.x, self.y` and increments its cursor.
    pub fn write_char(&mut self, c: u8) {
        self.shift_text_right(self.x, 1);

        let _ = self.write_char_at(self.y, self.x, c);
        self.inc_cursor();
        self.flush();
    }

    /// Deletes the character from the VGA buffer at `self.x, self.y` and decrements the cursor.
    pub fn delete_char(&mut self) {
        self.dec_cursor();

        let _ = self.write_char_at(self.y, self.x, 0);
        self.flush();
    }

    #[allow(unused)]
    /// Writes `s` to the VGA buffer starting at `self.x, self.y` and increments the cursor by `s.len()`.
    pub fn write_u8_arr(&mut self, s: &[u8]) {
        for c in s.iter() {
            if *c == 0 {
                return;
            }
            self.write_char(*c);
        }
        self.flush();
    }

    /// Fills the whole VGA buffer with `0u16`, clearing the screen.
    pub fn clear_screen(&mut self) {
        for row in 0..VGA_HEIGHT {
            for col in 0..VGA_WIDTH {
                let _ = self.write_char_at(row, col, 0);
            }
        }
        self.flush();
    }

    /// Moves `self.y` to `self.y + 1` and `self.x` to `0`, and updates the cursor.
    pub fn new_line(&mut self) {
        self.write_char(Buffer::NEWLINE);
        self.y += 1;
        self.x = 0;

        if self.y >= VGA_HEIGHT {
            self.scroll_down();
        }

        #[cfg(not(test))]
        unsafe {
            self.cursor.update_pos(self.x as u16, self.y as u16);
        }

        self.flush();
    }

    /// Sets the 4 most significant bits of `self.color` to `foreground`, setting the
    /// font color of the VGA buffer.
    pub fn set_foreground_color(&mut self, foreground: Color) {
        self.color &= 0xF0;
        self.color |= foreground.to_foreground();
    }

    /// Sets the 4 least significant bits of `self.color` to `background`, setting the
    /// background color of the VGA buffer.
    pub fn set_background_color(&mut self, background: Color) {
        self.color &= 0x0F;
        self.color |= background.to_background();
    }

    /// Shifts the text back if a write is happening in the middle of a text block. Else, does nothing.
    ///
    /// Needs to be called at every write.
    fn shift_text_right(&mut self, from_x: u8, by: u8) {
        let block_length = self.buffer.block_length(from_x, self.y);

        for x in (from_x..(from_x + block_length as u8)).rev() {
            let x_shifted = (x + by) % VGA_WIDTH;
            let y_shifted = self.y + if (x + by) >= VGA_WIDTH { 1 } else { 0 };

            let current_char = self.buffer.at(self.y as u16 * VGA_WIDTH as u16 + x as u16).unwrap();
            let _ = self.write_char_at(y_shifted, x_shifted, (*current_char & 0xFF) as u8);
        }
    }

    /// Abstraction around the VGA buffer address to avoid invalid memory access when running
    /// in test mode, where we do not have direct access to the VGA buffer.
    fn get_buffer_addr(&self) -> *mut u16 {
        #[cfg(test)]
        {
            get_vga_buffer_ptr()
        }

        #[cfg(not(test))]
        {
            VGA_BUFFER_ADDR
        }
    }

    fn flush(&self) {
        let current_displayed_content = self.buffer.slice(self.line_offset);

        for (idx, &entry) in current_displayed_content.iter().enumerate() {
            unsafe {
                write_volatile(self.get_buffer_addr().add(idx), entry);
            }
        }
    }

    /// Writes `character` at `self.x == x` and `self.y == y` into the VGA buffer.
    fn write_char_at(&mut self, y: u8, x: u8, character: u8) -> Result<(), OutOfBoundsError> {
        if y >= VGA_HEIGHT || x >= VGA_WIDTH {
            return Err(OutOfBoundsError);
        }
        let entry: u16 = (character as u16) | (self.color as u16) << 8;
        let index: isize = y as isize * VGA_WIDTH as isize + x as isize;

        self.buffer.write(self.line_offset, index as u16, entry);

        Ok(())
    }

    /// Decrements the cursor, taking line wrapping into account.
    fn dec_cursor(&mut self) {
        let on_first_col = self.x == 0;
        let on_first_row = self.y == 0;

        if on_first_col && on_first_row {
            self.line_offset = (self.line_offset - 1).max(0);
            self.x = self.get_x_for_y(self.y as usize) as u8;
        } else if on_first_col {
            self.y = (self.y - 1).max(0);
            self.x = self.get_x_for_y(self.y as usize) as u8;
        } else {
            self.x -= 1;
        }

        #[cfg(not(test))]
        unsafe {
            self.cursor.update_pos(self.x as u16, self.y as u16);
        }
    }

    /// Increments the cursor, taking line wrapping into account.
    fn inc_cursor(&mut self) {
        self.x += 1;

        if self.x >= VGA_WIDTH {
            self.x = 0;
            self.y += 1;
        }

        if self.y >= VGA_HEIGHT {
            self.scroll_down();
        }

        #[cfg(not(test))]
        unsafe {
            self.cursor.update_pos(self.x as u16, self.y as u16);
        }
    }

    fn scroll_down(&mut self) {
        self.line_offset = (self.line_offset + 1).min(MAX_BUFFERED_LINES - VGA_HEIGHT);
        self.y = VGA_HEIGHT - 1;

        self.flush();
    }

    /// Gets the position of the last written character for `y`, to ensure the cursor returns
    /// to the correct position when backspacing at `x == 0`.
    fn get_x_for_y(&self, y: usize) -> usize {
        (self.buffer.block_length(0, y as u8) - 1) as usize
    }
}

#[cfg(test)]
static VGA_BUFFER_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_new_vga() {
        let _guard = VGA_BUFFER_LOCK.lock();

        let v = Vga::new();

        assert_eq!(v.x, 0, "Vga::x should be initialized to 0");
        assert_eq!(v.y, 0, "Vga::x should be initialized to 0");

        let expected_color = Color::Black.to_background() | Color::White.to_foreground();
        assert_eq!(
            v.color, expected_color,
            "Vga::color should be initialized to Color::Black.to_background() | Color::White.to_foreground()"
        );
    }

    #[test]
    fn test_line_wrap() {
        let _guard = VGA_BUFFER_LOCK.lock();

        let mut v = Vga::new();

        for _ in 0..VGA_WIDTH {
            v.inc_cursor();
        }

        assert_eq!(v.x, 0, "Vga::x should wrap around when reaching 64");

        v.clear_screen();
    }

    #[test]
    fn test_backspace_line_start_empty_previous_line() {
        let _guard = VGA_BUFFER_LOCK.lock();

        let mut v = Vga::new();

        v.new_line();
        v.delete_char();

        assert_eq!(v.y, 0, "Vga::y should decrease by 1 when deleting a character at the beginning of a line");
        assert_eq!(v.x, 0, "Vga::x should return to the beginning of the previous line when it is empty");

        v.clear_screen();
    }

    #[test]
    fn test_backspace_line_start_previous_line_with_content() {
        let _guard = VGA_BUFFER_LOCK.lock();

        let mut v = Vga::new();

        v.write_u8_arr(b"Hello, World");
        v.new_line();
        v.delete_char();

        assert_eq!(v.y, 0, "Vga::y should decrease by 1 when deleting a character at the beginning of a line");
        assert_eq!(
            v.x, 12,
            "Vga::x should return to the last written non-null character of the previous line when deleting a line"
        );

        v.clear_screen();
    }

    #[test]
    fn test_hello_world() {
        let _guard = VGA_BUFFER_LOCK.lock();

        let mut v = Vga::new();

        v.write_u8_arr(b"Hello, World");
        unsafe {
            let buf = &VGA_BUFFER_ADDR[0..12];

            let mut written_content: [u8; 12] = [0u8; 12];

            for (idx, &entry) in buf.iter().enumerate() {
                written_content[idx] = (entry & 0x00FF) as u8;
            }

            assert_eq!(&written_content, b"Hello, World", "Content has not been written to VGA_BUFFER_ADDR");
        }

        v.clear_screen();
    }
}
