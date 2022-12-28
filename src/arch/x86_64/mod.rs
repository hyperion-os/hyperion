// both cannot coexist (AFAIK.) and QEMU
// cannot boot multiboot2 kernels directly
//
// so multiboot1 it is .. temporarily

// multiboot1 header and glue code
#[cfg(all())]
mod multiboot1;

// multiboot2 header and glue code
#[cfg(any())]
mod multiboot2;
