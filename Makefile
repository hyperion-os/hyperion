##
# Hyperion
#
# @file
# @version 0.1

ARCH          ?= x86_64
#ARCH          ?= x86
PROFILE       ?= debug
#PROFILE       ?= release
GDB           ?= false

# binary config
NASM          ?= nasm
LD            ?= ld.lld
OBJCOPY       ?= llvm-objcopy
CARGO         ?= cargo
#CARGO         ?= cargo-clif

# common directories
TARGET_DIR    ?= target
HYPER_DIR     := ${TARGET_DIR}/hyperion/${ARCH}
ARCH_DIR      := src/arch/${ARCH}
CARGO_DIR      = ${TARGET_DIR}/${RUST_T_${ARCH}}/${PROFILE}

# hyperion kernel lib
RUST_T_x86_64 := x86_64-unknown-none
RUST_F_debug  :=
RUST_F_release:= --release
CARGO_FLAGS   ?=
CARGO_FLAGS   += ${RUST_F_${PROFILE}}
CARGO_FLAGS   += --target=${RUST_T_${ARCH}}
KERNEL_LIB    := ${CARGO_DIR}/libhyperion.a
KERNEL_SRC    := $(filter-out %: ,$(file < ${CARGO_DIR}/libhyperion.d))
${KERNEL_LIB} : ${KERNEL_SRC} Makefile Cargo.toml Cargo.lock
	@echo "\n\033[32m--[[ building Hyperion lib ]]--\033[0m"
	${CARGO} build ${CARGO_FLAGS}

# hyperion boot code
# BOOT_SRC      := ${ARCH_DIR}/start.asm
# BOOT_OBJ      := ${HYPER_DIR}/start.o
# NASM_F_x86_64 := elf64
# NASM_F_x86    := elf32
# NASM_FLAGS    ?=
# NASM_FLAGS    += ${BOOT_SRC}
# NASM_FLAGS    += -o ${BOOT_OBJ}
# NASM_FLAGS    += -f ${NASM_F_${ARCH}}
# ${BOOT_OBJ} : ${BOOT_SRC} Makefile
# 	@echo "\n\033[32m--[[ building Hyperion boot ]]--\033[0m"
# 	mkdir -p ${HYPER_DIR}
# 	${NASM} ${NASM_FLAGS}

# hyperion kernel elf
LD_SCRIPT     := ${ARCH_DIR}/link.ld
KERNEL_ELF    := ${HYPER_DIR}/hyperion
KERNEL_DEPS   := ${KERNEL_LIB} #${BOOT_OBJ}
LD_M_x86_64   := elf_x86_64
LD_M_x86      := elf_i386
LD_FLAGS      ?=
#LD_FLAGS      += --whole-archive
LD_FLAGS      += ${KERNEL_DEPS}
LD_FLAGS      += -o ${KERNEL_ELF}
LD_FLAGS      += --gc-sections
LD_FLAGS      += -T ${LD_SCRIPT}
LD_FLAGS      += -m ${LD_M_${ARCH}}
${KERNEL_ELF} : ${KERNEL_DEPS} ${LD_SCRIPT} Makefile
	@echo "\n\033[32m--[[ building Hyperion kernel ]]--\033[0m"
	mkdir -p ${HYPER_DIR}
	${LD} ${LD_FLAGS}
#	evil hack to satisfy qemu and grub:
#	the entry format has to be x86 not x86_64
	${OBJCOPY} -O elf32-i386 ${KERNEL_ELF}

# hyperion iso
HYPERION      := ${HYPER_DIR}/hyperion.iso
ISO_DIR       := ${HYPER_DIR}/iso
BOOT_DIR      := ${ISO_DIR}/boot
GRUB_DIR      := ${BOOT_DIR}/grub
${HYPERION} : ${KERNEL_ELF} cfg/grub.cfg Makefile
	@echo "\n\033[32m--[[ building Hyperion iso ]]--\033[0m"
	mkdir -p ${GRUB_DIR}
	cp cfg/grub.cfg ${GRUB_DIR}
	cp ${KERNEL_ELF} ${BOOT_DIR}/
	grub-mkrescue /usr/lib/grub/i386-pc -o $@ ${ISO_DIR}

# build alias
build : ${KERNEL_ELF}

# qemu direct kernel boot alias
QEMU_x86_64   ?= qemu-system-x86_64
QEMU_x86      ?= qemu-system-i386
QEMU_FLAGS    ?=
QEMU_FLAGS    += -serial stdio
QEMU_FLAGS    += -s
ifeq (${GDB},true)
QEMU_FLAGS    += -S
endif
QEMU_FLAGS    += -enable-kvm
QEMU_FLAGS    += -d cpu_reset,guest_errors
#QEMU_FLAGS    += -M pc-i440fx-7.2
#QEMU_FLAGS    += -device VGA,vgamem_mb=64
QEMU_FLAGS    += -vga std
QEMU_KERNEL   := -kernel ${KERNEL_ELF} -append qemu
qemu : ${KERNEL_ELF}
	${QEMU_${ARCH}} ${QEMU_FLAGS} ${QEMU_KERNEL}

# qemu iso boot alias
#QEMU_FLAGS    += -bios ${QEMU_OVMF}
QEMU_OVMF     ?= /usr/share/ovmf/x64/OVMF.fd
QEMU_ISO      := -drive format=raw,file=${HYPERION}
qemu_iso : ${HYPERION}
	${QEMU_${ARCH}} ${QEMU_FLAGS} ${QEMU_ISO}

# connect gdb to qemu
GDB_FLAGS     ?=
GDB_FLAGS     += --eval-command="target remote localhost:1234"
GDB_FLAGS     += --eval-command="symbol-file ${KERNEL_ELF}"
gdb:
	gdb ${GDB_FLAGS}

# objdump
objdump : ${KERNEL_ELF}
	objdump -D ${KERNEL_ELF}

readelf : ${KERNEL_ELF}
	readelf --all ${KERNEL_ELF}

.PHONY : build qemu objdump readelf

# end
