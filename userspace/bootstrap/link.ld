ENTRY(_start_bootstrap)
OUTPUT_ARCH(i386:x86-64)
/* OUTPUT_FORMAT("binary") */

SECTIONS {
    . = 0x80000000;

    .boot : ALIGN(4k) {
        KEEP(*(.boot))
    }

    .text : ALIGN(4k) {
        *(.text .text.*)
    }

    .rodata : ALIGN(4k) {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(4k) {
        *(.data .data.*)
    }

    .bss : ALIGN(4k) {
        *(COMMON)
        *(.bss .bss.*)
    }

    .eh_frame_hdr           : {
        PROVIDE(__eh_frame_hdr = .);
        KEEP(*(.eh_frame_hdr))
        PROVIDE(__eh_frame_hdr_end = .);
    }
    .eh_frame               : {
        PROVIDE(__eh_frame = .);
        KEEP(*(.eh_frame))
        PROVIDE(__eh_frame_end = .);
    }
}
