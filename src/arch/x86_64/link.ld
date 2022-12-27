ENTRY(start)

SECTIONS {
    . = 1M;

    .boot_text : ALIGN(4k) {
          KEEP(*(.multiboot1))
          *(.boot)
    }

    .text : ALIGN(4k) {
          *(.text)
    }
}
