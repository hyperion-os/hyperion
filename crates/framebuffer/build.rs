use std::{env, fs, path::Path};

use proc_macro2::TokenStream;
use quote::quote;

//

fn main() {
    let bmp_path = "../../asset/font.bmp";
    println!("cargo:rerun-if-changed={bmp_path:?}");
    let bmp = image::open(bmp_path).unwrap().to_luma8();
    assert_eq!(bmp.width(), 4096);
    assert_eq!(bmp.height(), 16);

    let mut result = TokenStream::new();

    for i in 0..=255_u8 {
        let mut byte = ([0u16; 16], false);

        bmp.chunks(16)
            .skip(i as usize)
            .step_by(256)
            .enumerate()
            .for_each(|(i, s)| {
                s.iter().enumerate().for_each(|(j, b)| {
                    if *b != 255 {
                        byte.0[i] |= 1 << j;
                    }
                });
            });

        // set the flag if the character is 16 wide instead of 8 wide
        byte.1 = !byte.0.iter().all(|c| *c < 0x100);

        let ascii_char_rows = byte
            .0
            .into_iter()
            .fold(quote! {}, |acc, s| quote! { #acc #s, });
        let is_double_wide = byte.1;

        result = quote! {
            #result
            ([#ascii_char_rows], #is_double_wide),
        };
    }

    result = quote! {
        [#result]
    };

    let out_dir = env::var("OUT_DIR").unwrap();

    fs::write(Path::new(&out_dir).join("font.arr.rs"), result.to_string()).unwrap();
}
