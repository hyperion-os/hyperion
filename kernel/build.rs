fn main() {
    println!("cargo:rustc-link-arg=-Tkernel/riscv64.ld");
    println!("cargo:rustc-link-arg=--omagic");
}
