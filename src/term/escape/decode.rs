use crate::video::framebuffer::Color;

/// foreground color can be changed like this: "\x1B[38;2;<r>;<g>;<b>m"
/// background color can be changed like this: "\x1B[48;2;<r>;<g>;<b>m"
///
/// THESE ARE NON STANDARD ESCAPE SEQUENCES
pub struct EscapeDecoder {
    buf: [u8; LONGEST_ESCAPE],
    len: u8,
}

pub enum DecodedPart {
    Byte(u8),

    /// Null terminated
    Bytes([u8; LONGEST_ESCAPE]),

    FgColor(Color),
    BgColor(Color),
    Reset,

    None,
}

//

impl EscapeDecoder {
    pub const fn new() -> Self {
        Self {
            buf: [0; LONGEST_ESCAPE],
            len: 0,
        }
    }

    pub fn next(&mut self, byte: u8) -> DecodedPart {
        match (self.len, byte) {
            (0, b'\x1B') => {
                self.len += 1;
                self.buf[0_usize] = byte;
                DecodedPart::None
            }
            (0, _) => DecodedPart::Byte(byte),
            (1, b'[') => {
                self.len += 1;
                self.buf[1_usize] = byte;
                DecodedPart::None
            }
            (i, b'm') => {
                self.len += 1;
                self.buf[i as usize] = byte;

                // crate::qemu::_print(format_args_nl!(
                //     "seq part: {:?}",
                //     core::str::from_utf8(&self.buf[..self.len as usize])
                // ));

                let result = match self.buf[..self.len as usize] {
                    [b'\x1B', b'[', b'3', b'8', b';', b'2', b';', ref rgb @ .., b'm'] => {
                        Self::parse_rgb_part(rgb).map(DecodedPart::FgColor)
                    }
                    [b'\x1B', b'[', b'4', b'8', b';', b'2', b';', ref rgb @ .., b'm'] => {
                        Self::parse_rgb_part(rgb).map(DecodedPart::BgColor)
                    }
                    [b'\x1B', b'[', b'm'] => Some(DecodedPart::Reset),
                    _ => None,
                };

                if let Some(result) = result {
                    self.clear();
                    result
                } else {
                    self.clear()
                }
            }
            (i @ LONGEST_ESCAPE_PREV_U8.., _) => {
                self.len += 1;
                self.buf[i as usize] = byte;
                self.clear()
            }
            (i, _) => {
                self.len += 1;
                self.buf[i as usize] = byte;
                DecodedPart::None
            }
        }
    }

    pub fn clear(&mut self) -> DecodedPart {
        self.len = 0;
        DecodedPart::Bytes(core::mem::take(&mut self.buf))
    }

    fn parse_rgb_part(rgb: &[u8]) -> Option<Color> {
        let mut iter = rgb.split(|c| *c == b';');
        let r = core::str::from_utf8(iter.next()?).ok()?.parse().ok()?;
        let g = core::str::from_utf8(iter.next()?).ok()?.parse().ok()?;
        let b = core::str::from_utf8(iter.next()?).ok()?.parse().ok()?;
        Some(Color::new(r, g, b))
    }
}

//

// longest supported: "\x1B[48;2;255;255;255m"
const LONGEST_ESCAPE: usize = "\x1B[48;2;255;255;255m".len();
const LONGEST_ESCAPE_PREV_U8: u8 = LONGEST_ESCAPE as u8 - 1;
