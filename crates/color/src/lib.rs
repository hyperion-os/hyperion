#![no_std]

//

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

//

impl Color {
    pub const WHITE: Color = Color::new(0xff, 0xff, 0xff);
    pub const BLACK: Color = Color::new(0x00, 0x00, 0x00);

    pub const RED: Color = Color::new(0xff, 0x00, 0x00);
    pub const GREEN: Color = Color::new(0x00, 0xff, 0x00);
    pub const BLUE: Color = Color::new(0x00, 0x00, 0xff);

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const fn from_u32(code: u32) -> Self {
        let [r, g, b, _] = code.to_ne_bytes();
        Self::new(r, g, b)
    }

    pub const fn from_hex(hex_code: &str) -> Option<Self> {
        Self::from_hex_bytes(hex_code.as_bytes())
    }

    pub const fn from_hex_bytes(hex_code: &[u8]) -> Option<Self> {
        match hex_code {
            [r0, r1, g0, g1, b0, b1, _, _]
            | [r0, r1, g0, g1, b0, b1]
            | [b'#', r0, r1, g0, g1, b0, b1, _, _]
            | [b'#', r0, r1, g0, g1, b0, b1] => {
                Some(Self::from_hex_bytes_2([*r0, *r1, *g0, *g1, *b0, *b1]))
            }
            _ => None,
        }
    }

    pub const fn from_hex_bytes_2(hex_code: [u8; 6]) -> Self {
        const fn parse_hex_char(c: u8) -> u8 {
            match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 0xa,
                _ => c,
            }
        }

        const fn parse_byte(str_byte: [u8; 2]) -> u8 {
            parse_hex_char(str_byte[0]) | parse_hex_char(str_byte[1]) << 4
        }

        let r = parse_byte([hex_code[0], hex_code[1]]);
        let g = parse_byte([hex_code[2], hex_code[3]]);
        let b = parse_byte([hex_code[4], hex_code[5]]);

        Self::new(r, g, b)
    }

    pub const fn as_u32(&self) -> u32 {
        // self.b as u32 | (self.g as u32) << 8 | (self.r as u32) << 16
        u32::from_le_bytes([self.b, self.g, self.r, 0])
    }

    pub const fn as_arr(&self) -> [u8; 4] {
        self.as_u32().to_ne_bytes()
        // [self.r, self.g, self.b, 0]
    }
}
