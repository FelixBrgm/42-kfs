use core::error::Error;

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum VGAColor {
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

impl VGAColor {
    pub fn to_foreground(&self) -> u8 {
        *self as u8
    }

    pub fn to_background(&self) -> u8 {
        (*self as u8) << 4
    }
}

const VGA_WIDTH: u8 = 80;
const VGA_HEIGHT: u8 = 25;
const VGA_BUFFER_ADDR: *mut u16 = 0xB8000 as *mut u16;

pub struct  OutOffBoundsError;

pub struct Buffer {
    color: u8,
    buf: *mut u16,
}

impl Buffer {
    pub fn new() -> Self {
        let mut buffer = Buffer {
            color: 0,
            buf: VGA_BUFFER_ADDR
        };
        buffer.set_colors(VGAColor::White, VGAColor::Black);
        buffer
    }

    pub fn set_colors(&mut self, foreground: VGAColor, background: VGAColor) {
        self.color = foreground.to_foreground() | background.to_background()
    }
    
    pub fn set_foreground_color(&mut self, foreground: VGAColor) {
        self.color = self.color & 0xF0;
        self.color |= foreground.to_foreground();
    }

    pub fn set_background_color(&mut self, background: VGAColor) {
        self.color = self.color & 0x0F;
        self.color |= background.to_background();
    }

    pub fn write_char_at(&self, row: u8, column: u8, character: u8) -> Result<(), OutOffBoundsError>  {

        if row >= VGA_HEIGHT || column >= VGA_WIDTH {
            return Err(OutOffBoundsError);
        }
        let entry: u16 = (character as u16) | (self.color as u16) << 8;
        let index: isize = row as isize * VGA_WIDTH as isize + column as isize;
        // Safety:
        // The if statement above ensures that the index never goes out of the dedicated memory for the chars on screen
        unsafe {
            *self.buf.offset(index) = entry;
        }
        Ok(())
    }
    pub fn clear_screen(&mut self) {
        for row in 0..VGA_HEIGHT {
            for col in 0..VGA_WIDTH {
                self.write_char_at(row, col, b' ');
            }
        }
    }
}