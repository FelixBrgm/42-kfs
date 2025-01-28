use super::{ps2::Key, screen::Screen};

pub const NBR_OF_SCREENS_PER_TERMINAL: usize = 5;

pub struct Terminal {
    pub active_screen_index: usize,
    screens: [Screen; NBR_OF_SCREENS_PER_TERMINAL],
}

impl Terminal {
    /// Creates a new `Terminal` instance with the default screen and sets the first screen as active.
    ///
    /// # Returns
    /// A `Terminal` instance with the default screen state.
    pub fn default() -> Terminal {
        Terminal {
            active_screen_index: 0,
            screens: [Screen::default(); NBR_OF_SCREENS_PER_TERMINAL],
        }
    }

    /// Handles a key press event by updating the terminal's state.
    ///
    /// If the key is the `Tab` key, it switches to the next screen. Otherwise, the key event is passed
    /// to the active screen for processing.
    ///
    /// # Parameters
    /// - `key`: The key that was pressed.
    pub fn handle_key(&mut self, key: Key) {
        match key {
            Key::Tab => {
                self.active_screen_index += 1;
                if self.active_screen_index >= NBR_OF_SCREENS_PER_TERMINAL {
                    self.active_screen_index = 0;
                }
            }
            _ => self.screens[self.active_screen_index].handle_key(key),
        }
    }

    pub fn write_str(&mut self, string: &str) {
        self.screens[self.active_screen_index].write_str(string);
    }

    #[allow(dead_code)]
    pub fn write_color_str(&mut self, string: &str, color: u8) {
        self.screens[self.active_screen_index].write_color_str(string, color);
    }

    pub fn active_screen(&mut self) -> &mut Screen {
        &mut self.screens[self.active_screen_index]
    }
}
