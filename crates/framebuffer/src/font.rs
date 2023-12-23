pub static FONT: [([u16; 16], bool); 256] = include!(concat!(env!("OUT_DIR"), "/font.arr.rs"));

// hyperion_macros::bmp_to_font!("./../../font.bmp");
