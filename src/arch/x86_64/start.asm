    ;; ----------
    ;; Multiboot2
    ;; ----------

    section .multiboot2

header_start:
    dd 0xE85250D6                                             ;; multiboot2 magic
    dd 0                                                      ;; arch: 32 bit (protected mode)
    dd header_end - header_start                              ;; header length
    dd 0x100000000 - (0xE85250D6 + header_end - header_start) ;; checksum

    dd 0 ;; end tag
    dd 8
header_end:

    ;; ----------
    ;; Multiboot1
    ;; ----------

    section .multiboot1
    dd 0x1BADB002                     ;; multiboot1 magic
    dd 3                              ;; flags
    dd 0x100000000 - (0x1BADB002 + 3) ;; checksum

    section .boot
    global start
    bits 32

start:
    cli
    cld
    hlt
