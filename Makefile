##
# Hyperion
#
# @file
# @version 0.1

# config
ARCH             ?= x86_64
#ARCH             ?= x86
PROFILE          ?= debug
#PROFILE          ?= release
GDB              ?= false
BOOTLOADER       ?= limine
KVM              ?= true

# binaries
NASM             ?= nasm
LD               ?= ld.lld
OBJCOPY          ?= llvm-objcopy
CARGO            ?= cargo
#CARGO            ?= cargo-clif
XORRISO          ?= xorriso
JQ               ?= jq
QEMU_x86_64      ?= qemu-system-x86_64
QEMU_x86         ?= qemu-system-i386
QEMU             ?= ${QEMU_${ARCH}}

# rust targets
RUST_T_x86_64    := x86_64-unknown-none

# common directories
TARGET_DIR       ?= target
HYPER_DIR        := ${TARGET_DIR}/hyperion/${BOOTLOADER}/${ARCH}
ARCH_DIR         := src/arch/${ARCH}
BOOT_DIR         := src/boot
CARGO_DIR        := ${TARGET_DIR}/${RUST_T_${ARCH}}/${PROFILE}
ISO_DIR          := ${HYPER_DIR}/iso
ISO_TESTING_DIR  := ${HYPER_DIR}/iso-testing

# artefacts
HYPERION         := ${HYPER_DIR}/hyperion.iso
HYPERION_TESTING := ${HYPER_DIR}/hyperion-testing.iso

# rust/cargo
RUST_F_debug     :=
RUST_F_release   := --release
CARGO_FLAGS      ?=
CARGO_FLAGS      += ${RUST_F_${PROFILE}}
CARGO_FLAGS      += --target=${RUST_T_${ARCH}}
CARGO_FLAGS      += --package=hyperion
KERNEL           := ${CARGO_DIR}/hyperion
KERNEL_TESTING   := ${KERNEL}-testing
KERNEL_SRC       := $(filter-out %: ,$(file < ${CARGO_DIR}/hyperion.d)) src/testfw.rs

# gdb
GDB_FLAGS        ?=
GDB_FLAGS        += --eval-command="target remote localhost:1234"
GDB_FLAGS        += --eval-command="symbol-file ${KERNEL}"

# hyperion kernel compilation
${KERNEL}: ${KERNEL_SRC} Makefile Cargo.toml Cargo.lock
	@echo "\n\033[32m--[[ building Hyperion ]]--\033[0m"
	${CARGO} build ${CARGO_FLAGS}
	@touch ${KERNEL}

${KERNEL_TESTING}: ${KERNEL_SRC} Makefile Cargo.toml Cargo.lock
	@echo "\n\033[32m--[[ building Hyperion-Testing ]]--\033[0m"
	@${CARGO} test --no-run # first one prints human readable errors
	${CARGO} test --no-run --message-format=json ${CARGO_FLAGS} | \
		jq -r "select(.profile.test == true) | .filenames[]" | \
		xargs -I % cp "%" ${KERNEL_TESTING}
	@touch ${KERNEL_TESTING}

# ISO generation
include ./${BOOT_DIR}/${BOOTLOADER}/Makefile

# ISO running
include ./qemu.mk

# build alias
build: ${KERNEL}

# bootable iso alias
iso: ${HYPERION}

clippy:
	${CARGO} clippy ${CARGO_FLAGS} -- -D warnings

# connect gdb to qemu
gdb:
	gdb ${GDB_FLAGS}

# objdump
objdump : ${KERNEL}
	objdump -D ${KERNEL}

readelf : ${KERNEL}
	readelf --all ${KERNEL}

.PHONY : build iso reset-cargo-deps run test gdb objdump readelf

# end
