#![no_std]
#![no_main]
#![feature(format_args_nl)]

//

use libstd::println;

//

#[no_mangle]
fn main() {
    println!("Hello, world!");
}
