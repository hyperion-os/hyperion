##
# Hyperion
#
# @file
# @version 0.1

ARCH          ?= x86_64
#ARCH          ?= x86
PROFILE       ?= debug
#PROFILE       ?= release

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
BOOT_SRC      := ${ARCH_DIR}/start.asm
BOOT_OBJ      := ${HYPER_DIR}/start.o
NASM_F_x86_64 := elf64
NASM_F_x86    := elf32
NASM_FLAGS    ?=
NASM_FLAGS    += ${BOOT_SRC}
NASM_FLAGS    += -o ${BOOT_OBJ}
NASM_FLAGS    += -f ${NASM_F_${ARCH}}
${BOOT_OBJ} : ${BOOT_SRC} Makefile
	@echo "\n\033[32m--[[ building Hyperion boot ]]--\033[0m"
	mkdir -p ${HYPER_DIR}
	${NASM} ${NASM_FLAGS}

# hyperion kernel elf
LD_SCRIPT     := ${ARCH_DIR}/link.ld
KERNEL_ELF    := ${HYPER_DIR}/hyperion
KERNEL_DEPS   := ${BOOT_OBJ} ${KERNEL_LIB}
LD_M_x86_64   := elf_x86_64
LD_M_x86      := elf_i386
LD_FLAGS      ?=
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

# build alias
build : ${KERNEL_ELF}

# qemu alias
QEMU_x86_64   ?= qemu-system-x86_64
QEMU_x86      ?= qemu-system-x86
QEMU_FLAGS    ?=
QEMU_FLAGS    += -enable-kvm
QEMU_FLAGS    += -d cpu_reset,guest_errors
QEMU_FLAGS    += -kernel ${KERNEL_ELF}
qemu : ${KERNEL_ELF}
	${QEMU_${ARCH}} ${QEMU_FLAGS}

.PHONY : build qemu

# end
