// #[derive(Debug, Clone, Copy, Default)]
// pub struct FontChar {
//     bitmap: [u8; 16],
// }

pub static FONT: [[u8; 16]; 256] = {
    let mut font = [[0u8; 16]; 256];

    font[b'a' as usize] = [
        0b_11111111,
        0b_11111111,
        0b_11000011,
        0b_11000011,
        0b_11000011,
        0b_11000011,
        0b_11000011,
        0b_11000011,
        0b_11000011,
        0b_11000011,
        0b_11000011,
        0b_11000011,
        0b_11000011,
        0b_11000011, //
        0b_11111111,
        0b_11111111,
    ];

    font
};
