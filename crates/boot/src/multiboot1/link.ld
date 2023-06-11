ENTRY(_start)

SECTIONS {
    . = 1M;

    .boot : ALIGN(4k) {
        KEEP(*(.boot))
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
