OUTPUT_ARCH(riscv)
ENTRY(_start)
KERNEL_BASE = 0x80200000; 
/* KERNEL_BASE = 0x80040000; */  /* this works, but might not work in the future */

SECTIONS {
  . = KERNEL_BASE;
  PROVIDE(_kernel_beg = .);

  .text : {
    PROVIDE(_text_beg = .);
    PROVIDE(_trampoline_beg = .);
    *(.text.boot)
    *(.text.trampoline)
    . = ALIGN(0x1000);
    PROVIDE(_trampoline_end = .);
    *(.text .text.*)
    . = ALIGN(0x1000);
    PROVIDE(_text_end = .);
  }

  PROVIDE(_global_pointer = .);

  .rodata : {
    PROVIDE(_rodata_beg = .);
    *(.rodata .rodata.*)
    . = ALIGN(16);
    *(.srodata .srodata.*)
    . = ALIGN(0x1000);
    PROVIDE(_rodata_end = .);
  }

  .data : {
    PROVIDE(_data_beg = .);
    *(.data .data.*)
    . = ALIGN(16);
    *(.sdata .sdata.*)
    . = ALIGN(0x1000);
    PROVIDE(_data_end = .);
  }

  .bss : {
    PROVIDE(_bss_beg = .);
    *(.bss .bss.*)
    . = ALIGN(16);
    *(.sbss .sbss.*)
    . = ALIGN(0x1000);
    PROVIDE(_bss_end = .);
  }

  /* temporary boot stack */
  . += 0x8000;
  PROVIDE(_boot_stack_top = .);
  PROVIDE(_kernel_end = .);
}
