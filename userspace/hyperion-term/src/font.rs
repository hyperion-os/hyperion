use std::{fs::File, io::Read};

#[derive(Debug, Clone, Copy)]
pub struct MonoFont {
    pub glyphs: [MonoGlyph; 256],
}

impl MonoFont {
    pub const fn glyph(&self, i: u8) -> MonoGlyph {
        self.glyphs[i as usize]
    }
}

//

#[derive(Debug, Clone, Copy)]
pub struct MonoGlyph {
    pub bitmap: [u16; 16],

    // is 16x16 instead of 8x16
    pub is_wide: bool,
}

//

pub fn load_monospace_ttf() -> MonoFont {
    // FIXME: doesn't work:
    // let bmp = image::open("/font.bmp").unwrap().into_luma8();

    let mut f = File::open("/font.bmp").unwrap();
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).unwrap();
    let bmp = image::load_from_memory(&buf).unwrap().into_luma8();

    assert_eq!(bmp.width(), 4096);
    assert_eq!(bmp.height(), 16);

    let mut font = MonoFont {
        glyphs: [MonoGlyph {
            bitmap: [0; 16],
            is_wide: false,
        }; 256],
    };

    for i in 0..=255_u8 {
        let mut glyph = MonoGlyph {
            bitmap: [0u16; 16],
            is_wide: false,
        };

        bmp.chunks(16)
            .skip(i as usize)
            .step_by(256)
            .enumerate()
            .for_each(|(i, s)| {
                // convert byte per pixel to bit per pixel
                s.iter().enumerate().for_each(|(j, b)| {
                    if *b != 255 {
                        glyph.bitmap[i] |= 1 << j;
                    }
                });
            });

        // set the flag if the character is 16 wide instead of 8 wide
        glyph.is_wide = !glyph.bitmap.iter().all(|c| *c < 0x100);

        font.glyphs[i as usize] = glyph;
    }

    font
}
