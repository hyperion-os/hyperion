ENTRY(start)

SECTIONS {
    . = 1M;

    .multiboot : ALIGN(4k) {
        KEEP(*(.multiboot))
        KEEP(*(.multiboot_rust))
    }

    .boot : ALIGN(4k) {
        *(.boot)
    }

    .text : ALIGN(4k) {
        *(.text)
    }

    .rodata : ALIGN(4k) {
        *(.rodata)
    }

    .data : ALIGN(4k) {
        *(.data)
    }

    .bss : ALIGN(4k) {
        *(COMMON)
        *(.bss)
    }
}
