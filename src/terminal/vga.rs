use core::ptr::{read_volatile, write_volatile};

use super::{
    cursor::Cursor,
    screen::{Screen, BUFFER_SIZE},
};

/// The `width` of the viewable area of the VGA Buffer in chars
pub const VIEW_WIDTH: usize = 80;

/// The `height` of the viewable area of the VGA Buffer in chars
pub const VIEW_HEIGHT: usize = 25;

/// The total number of character positions in the viewable area (width x height).
pub const VIEW_BUFFER_SIZE: usize = VIEW_WIDTH * VIEW_HEIGHT;

/// The base memory address of the VGA buffer for text mode display.
const VGA_BUFFER_ADDR: *mut u16 = 0xB8000 as *mut u16;

/// A struct representing a screen buffer for VGA entry handling and cursor management.
///
/// The `Buffer` holds a 2D array representing the screen's character data,
/// a cursor position (`cursor_x` and `cursor_y`), and provides methods for creating
/// a new buffer from a screen and flushing its contents to a device.
pub struct Buffer {
    /// A fixed-size array to hold screen data, representing characters and their colors.
    buffer: [u16; VIEW_BUFFER_SIZE],

    /// Cursor
    cursor: Option<Cursor>,
}

impl Buffer {
    /// Creates a new `Buffer` from a given `Screen` object.
    ///
    /// This function processes a `Screen`'s buffer and computes a corresponding `Buffer` for display.
    /// It handles cursor positioning, padding, and newline characters. The new buffer is populated
    /// with the screen's entries, and the cursor position is updated based on the screen's cursor.
    ///
    /// # Arguments
    /// * `s` - A reference to a `Screen` object that contains the data to be converted into a `Buffer`.
    ///
    /// # Returns
    /// A new `Buffer` with the formatted data from the `Screen` and the updated cursor position.
    pub fn from_screen(s: &Screen) -> Self {
        let mut view_padding_whitespace: usize = 0;

        let mut vga_buffer: Buffer = Buffer {
            buffer: [0; VIEW_BUFFER_SIZE],
            cursor: None,
        };

        let view_start_index = calculate_view_start_index(s);
        for (relative_index, &entry) in s.buffer.iter().skip(view_start_index).enumerate() {
            let padded_relative_index = relative_index + view_padding_whitespace;
            let index_after_viewport = padded_relative_index >= VIEW_BUFFER_SIZE;
            if index_after_viewport {
                break;
            }

            let relative_cursor = s.cursor - view_start_index;
            let padded_relative_cursor = relative_cursor + view_padding_whitespace;
            if relative_cursor == relative_index {
                vga_buffer.cursor = Some(Cursor::new(
                    (padded_relative_cursor % VIEW_WIDTH) as u16,
                    (padded_relative_cursor / VIEW_WIDTH) as u16,
                ));
            }

            match (entry & 0xFF) as u8 {
                b'\n' => {
                    let padding = VIEW_WIDTH - (padded_relative_index % VIEW_WIDTH) - 1;
                    view_padding_whitespace += padding;

                    for _ in 0..(padding + 1) {
                        vga_buffer.buffer[padded_relative_index] = Entry::new(b' ').to_u16()
                    }
                }
                _ => vga_buffer.buffer[padded_relative_index] = entry, // _ => write_entry_to_vga(padded_relative_index, entry).unwrap(),
            }
        }

        vga_buffer
    }
    /// Flushes the contents of the buffer to the hardware VGA device.
    ///
    /// This function writes the entries in the buffer to the VGA display,
    /// and updates the cursor position based on the `cursor_x` and `cursor_y` values stored in the buffer.
    ///
    /// # Example
    /// ```
    /// let buffer = Buffer::from_screen(&screen);
    /// buffer.flush();
    /// ```
    pub fn flush(&self) {
        for (i, e) in self.buffer.iter().enumerate() {
            write_entry_to_vga(i, *e).unwrap();
        }
        match self.cursor {
            Some(c) => {
                unsafe {
                    c.flush_pos();
                }
                Cursor::show();
            }
            None => Cursor::hide(),
        }
    }
}

fn calculate_view_start_index(t: &Screen) -> usize {
    let mut rows: [(usize, usize); BUFFER_SIZE] = [(0, 0); BUFFER_SIZE];
    let mut index_rows = 0;

    let mut current_line = (0, 0);
    for (i, e) in t.buffer.iter().enumerate() {
        if current_line == (0, 0) {
            current_line.0 = i;
        }
        if current_line.1 >= current_line.0 && (current_line.1 - current_line.0) == (VIEW_WIDTH - 1) {
            rows[index_rows] = current_line;
            index_rows += 1;
            current_line = (0, 0);
            continue;
        }
        match (e & 0xFF) as u8 {
            b'\n' => {
                current_line.1 = i;
                rows[index_rows] = current_line;
                index_rows += 1;
                current_line = (0, 0);
            }
            _ => {
                current_line.1 = i;
            }
        }
    }
    let mut row_position_last = 0;
    for (i, (start, end)) in rows.iter().enumerate() {
        if *start <= t.last_entry_index && t.last_entry_index - 1 <= *end {
            row_position_last = i;
            break;
        }
    }
    if row_position_last < t.rows_scrolled {
        row_position_last = 0;
    } else {
        row_position_last -= t.rows_scrolled;
    }
    if row_position_last < VIEW_HEIGHT {
        0
    } else {
        rows[row_position_last - (VIEW_HEIGHT - 1)].0
    }
}

#[derive(Debug)]
pub struct OutOfBoundsErr;

/// Writes an entry (a `u16` value) to the VGA buffer at the specified index.
///
/// This function ensures that an entry is only written if it's different from the existing one at that index.
/// It checks for the current value at the index and only performs the write if there's a change.
///
/// ### Parameters:
/// - `index`: The index in the VGA buffer to which the entry should be written.
/// - `entry`: The `u16` entry to be written to the VGA buffer.
///
/// ### Returns:
/// - `Ok(())` if the write is successful.
/// - `Err(OutOfBoundsErr)` if the index is out of bounds.
fn write_entry_to_vga(index: usize, entry: u16) -> Result<(), OutOfBoundsErr> {
    if index >= VIEW_BUFFER_SIZE {
        return Err(OutOfBoundsErr);
    }

    let written_entry = read_entry_from_vga(index).unwrap(); // Have to think about how we want to handle this
    if entry == written_entry {
        return Ok(());
    }

    unsafe { write_volatile(VGA_BUFFER_ADDR.add(index), entry) }
    Ok(())
}

/// Reads an entry (a `u16` value) from the VGA buffer at the specified index.
///
/// ### Parameters:
/// - `index`: The index in the VGA buffer to read from.
///
/// ### Returns:
/// - `Ok(u16)` if the read is successful.
/// - `Err(OutOfBoundsErr)` if the index is out of bounds.
fn read_entry_from_vga(index: usize) -> Result<u16, OutOfBoundsErr> {
    if index >= VIEW_BUFFER_SIZE {
        return Err(OutOfBoundsErr);
    }
    let e: u16 = unsafe { read_volatile(VGA_BUFFER_ADDR.add(index)) };
    Ok(e)
}

/// Represents a single character entry for the Screen buffer.
///
/// Each `Entry` consists of a character and a color attribute. The color is set to the default color (light gray on black)
/// by default, but it can be customized. Each `Entry` can be converted into a `u16` value, which is the format used for
/// writing to the VGA buffer.
pub struct Entry {
    color: u8,
    character: u8,
}

impl Entry {
    /// Creates a new `Entry` with the specified character and the default color.
    ///
    /// The default color is light gray (`0x07`).
    ///
    /// ### Parameters:
    /// - `character`: The character to be storedy.
    pub fn new(character: u8) -> Self {
        Entry {
            color: Color::Default as u8,
            character,
        }
    }

    /// Creates a new `Entry` with the specified character and custom color.
    ///
    /// This function allows the creation of a `Entry` with a specific character and color,
    /// where the color is passed as a parameter. The color is represented as an 8-bit value,
    /// allowing for a wide range of color codes (e.g., for screen colors). The character
    /// is displayed with this color when rendered to the VGA buffer.
    ///
    /// ### Parameters:
    /// - `character`: The character to be displayed (e.g., an ASCII value representing a letter or symbol).
    /// - `color`: The color code for the character (an 8-bit value that determines the character's color).
    ///   - The value should correspond to a color in the VGA color palette (for example, `0x0F` for white, `0x01` for blue, etc.).
    pub fn new_with_color(character: u8, color: u8) -> Self {
        Entry { color, character }
    }

    /// Converts this `Entry` into a `u16` value that can be written to the VGA buffer.
    ///
    /// The `u16` format stores the color in the upper 8 bits and the character in the lower 8 bits.
    ///
    /// ### Returns:
    /// A `u16` value representing this `Entry`.
    pub fn to_u16(&self) -> u16 {
        ((self.color as u16) << 8) | (self.character as u16)
    }
}

/// Represents the available color codes for screen entries.
///
/// The colors are defined as `u8` values, where each value corresponds to a particular color.
/// The default color is light gray on black.
#[repr(u8)]
pub enum Color {
    /// Light gray on black (default)
    Default = 0x07,
    /// White on Red
    #[allow(unused)]
    Error = 0x4F,
}

#[cfg(test)]
mod test {
    use crate::terminal::ps2::Key;

    use super::*;

    #[test]
    fn hello_world() {
        let test_string = "Hello World";
        let mut s = Screen::default();
        s.write_str(&test_string);
        let b = Buffer::from_screen(&s);
        assert_eq!(b.cursor.unwrap().x, 11);
        assert_eq!(b.cursor.unwrap().y, 0);
        for (i, c) in test_string.as_bytes().iter().enumerate() {
            assert_eq!(b.buffer[i], Entry::new(*c).to_u16())
        }
    }

    #[test]
    fn hitting_enter() {
        let mut s = Screen::default();
        s.write_str("A");
        for i in 0..VIEW_HEIGHT as u16 {
            if i % 2 == 0 {
                s.write_str("\n");
            } else {
                s.handle_key(Key::Enter);
            }
        }
        let b = Buffer::from_screen(&s);
        assert_eq!(b.buffer[0], Entry::new(b' ').to_u16());

        assert_eq!(b.cursor.unwrap().x, 0);
        assert_eq!(b.cursor.unwrap().y, (VIEW_HEIGHT - 1) as u16)
    }

    #[test]
    fn lines_of_coke() {
        let mut s = Screen::default();
        let test_string_1 = "Coka";
        let test_string_2 = "Cola";

        s.write_str(&test_string_1);
        s.handle_key(Key::Enter);
        s.write_str(&test_string_2);

        let b = Buffer::from_screen(&s);

        for (i, c) in test_string_1.as_bytes().iter().enumerate() {
            assert_eq!(b.buffer[i], Entry::new(*c).to_u16());
        }
        for (i, c) in test_string_2.as_bytes().iter().enumerate() {
            assert_eq!(b.buffer[VIEW_WIDTH + i], Entry::new(*c).to_u16());
        }

        assert_eq!(b.cursor.unwrap().x, test_string_2.len() as u16);
        assert_eq!(b.cursor.unwrap().y, 1);
    }

    #[test]
    fn a_long_line() {
        let mut s = Screen::default();
        for _ in 0..VIEW_WIDTH {
            s.handle_key(Key::A);
        }

        let b = Buffer::from_screen(&s);
        assert_eq!(b.cursor.unwrap().x, 0);
        assert_eq!(b.cursor.unwrap().y, 1);
    }

    #[test]
    fn backspacing() {
        let mut s = Screen::default();
        let test_string = "123";
        s.write_str(&test_string);
        s.handle_key(Key::Backspace);

        let b = Buffer::from_screen(&s);

        for (i, c) in test_string.as_bytes().iter().enumerate() {
            if test_string.len() - 1 == i {
                break;
            }
            assert_eq!(b.buffer[i], Entry::new(*c).to_u16());
        }

        assert_eq!(b.cursor.unwrap().x, test_string.len() as u16 - 1);
        assert_eq!(b.cursor.unwrap().y, 0);
    }
}
