fn main() {
    println!("cargo:rustc-link-arg=-no-pie");
    println!("cargo:rustc-link-arg-bin=bootstrap=--script=userspace/bootstrap/link.ld");
}
