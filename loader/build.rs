fn main() {
    println!("cargo:rustc-link-arg=-Tloader/riscv64.ld");
    println!("cargo:rustc-link-arg=--omagic");
}
