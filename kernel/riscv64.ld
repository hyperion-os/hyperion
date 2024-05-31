OUTPUT_ARCH(riscv)
ENTRY(_start)
KERNEL_BASE = 0x80200000; 

SECTIONS {
  . = KERNEL_BASE;

  .text : {
    PROVIDE(_text_start = .);
    *(.text.boot)
    . = ALIGN(16);
    *(.text .text.*)
    . = ALIGN(0x1000);
    PROVIDE(_text_end = .);
  }

  PROVIDE(_global_pointer = .);

  .rodata : {
    PROVIDE(_rodata_start = .);
    *(.rodata .rodata.*)
    . = ALIGN(16);
    *(.srodata .srodata.*)
    . = ALIGN(0x1000);
    PROVIDE(_rodata_end = .);
  }

  .data : {
    PROVIDE(_data_start = .);
    *(.data .data.*)
    . = ALIGN(16);
    *(.sdata .sdata.*)
    . = ALIGN(0x1000);
    PROVIDE(_data_end = .);
  }

  .bss : {
    PROVIDE(_bss_start = .);
    *(.bss .bss.*)
    . = ALIGN(16);
    *(.sbss .sbss.*)
    . = ALIGN(0x1000);
    PROVIDE(_bss_end = .);
  }

  /* temporary boot stack */
  . += 0x4000;
  PROVIDE(_boot_stack_top = .);
  PROVIDE(_kernel_end = .);
}
