use core::{
    fmt::{Arguments, Write},
    ops::{Deref, DerefMut},
};
use spin::{Mutex, MutexGuard};
use volatile::Volatile;

//

#[macro_export]
macro_rules! println {
    () => {
        println!("");
    };

    ($($arg:tt)*) => {
        $crate::vga::_println(format_args!($($arg)*))
    }
}

#[macro_export]
macro_rules! print {
    () => {
        print!("");
    };

    ($($arg:tt)*) => {
        $crate::vga::_print(format_args!($($arg)*))
    };
}

//

pub struct Writer {
    cursor: [usize; 2],
    color: ColorCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub enum Color {
    #[default]
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
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ColorCode(u8);

//

impl Writer {
    pub fn lock() -> MutexGuard<'static, Self> {
        WRITER.lock()
    }

    pub fn write_str(&mut self, s: &str) {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
    }

    pub fn write_char(&mut self, c: char) {
        self.write_str(c.encode_utf8(&mut [0; 4]))
    }

    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            // 'special' ascii chars
            b'\r' => self.cursor[0] = 0,
            b'\n' => self.new_line(),
            b'\0' => self.clear(),

            // 'normal' ascii chars
            byte => {
                // line wrapping
                if self.cursor[0] >= WIDTH {
                    self.new_line();
                }

                // insert the byte
                self.set_char(
                    self.cursor,
                    Char {
                        byte,
                        color: self.color,
                    },
                );

                // move the cursor
                self.cursor[0] += 1;
            }
        }
    }

    pub fn clear(&mut self) {
        self.cursor = [0, 0];
        for row in 0..HEIGHT {
            self.clear_row(row);
        }
    }

    /// SAFETY: Only one [`Writer`] should ever exist
    const unsafe fn new() -> Self {
        Self {
            cursor: [0, 0],
            color: ColorCode::new(Color::White, Color::Black),
        }
    }

    fn buffer(&self) -> &'static [[Volatile<Char>; WIDTH]; HEIGHT] {
        // SAFETY: Only one [`Writer`] should ever exist
        // then multiple immutable refs are allowed
        unsafe { &*(0xB8000 as *const _) }
    }

    fn buffer_mut(&mut self) -> &'static mut [[Volatile<Char>; WIDTH]; HEIGHT] {
        // SAFETY: Only one [`Writer`] should ever exist
        // then one mutable ref is allowed
        unsafe { &mut *(0xB8000 as *mut _) }
    }

    fn new_line(&mut self) {
        if self.cursor[1] + 1 >= HEIGHT {
            // move all rows upwards
            for row in 0..HEIGHT - 1 {
                for col in 0..WIDTH {
                    self.set_char([col, row], self.get_char([col, row + 1]));
                }
            }
        } else {
            // next row
            self.cursor[1] += 1;
        }
        self.clear_row(HEIGHT - 1);
        self.cursor[0] = 0;
    }

    fn clear_row(&mut self, row: usize) {
        self.fill_row(
            row,
            Char {
                byte: b' ',
                color: ColorCode::default(),
            },
        )
    }

    fn fill_row(&mut self, row: usize, fill: Char) {
        for col in 0..WIDTH {
            self.set_char([col, row], fill);
        }
    }

    fn get_char(&self, cursor: [usize; 2]) -> Char {
        self.buffer()[cursor[1]][cursor[0]].read()
    }

    fn set_char(&mut self, cursor: [usize; 2], ch: Char) {
        self.buffer_mut()[cursor[1]][cursor[0]].write(ch);
    }
}

impl ColorCode {
    pub const fn new(fg: Color, bg: Color) -> ColorCode {
        ColorCode((bg as u8) << 4 | (fg as u8))
    }
}

impl Default for ColorCode {
    fn default() -> Self {
        Self::new(Color::White, Color::Black)
    }
}

//

const WIDTH: usize = 80;
const HEIGHT: usize = 25;

//

/// SAFETY: safe, because this is the only Writer
static WRITER: Mutex<Writer> = Mutex::new(unsafe { Writer::new() });

//

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct Char {
    // ascii
    byte: u8,

    // foreground and background
    color: ColorCode,
}

//

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write_str(s);
        Ok(())
    }
}

impl Deref for Char {
    type Target = Self;

    fn deref(&self) -> &Self::Target {
        self
    }
}

impl DerefMut for Char {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self
    }
}

//

#[doc(hidden)]
pub fn _print(args: Arguments) {
    let mut writer = WRITER.lock();
    writer.write_fmt(args).unwrap();
}

#[doc(hidden)]
pub fn _println(args: Arguments) {
    let mut writer = WRITER.lock();
    writer.write_fmt(args).unwrap();
    writer.write_byte(b'\n');
}
