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
# target dir is usually ./target/ but I like to set the cargo target directory to a shared
# location to save disk space and build times
TARGET_DIR       ?= $(shell cargo metadata --format-version=1  | jq -r ".target_directory")
ifeq (${TARGET_DIR},)
TARGET_DIR       := target
endif
HYPER_DIR        := ${TARGET_DIR}/hyperion/${BOOTLOADER}/${ARCH}
ARCH_DIR         := crates/kernel/src/arch/${ARCH}
BOOT_DIR         := crates/boot-${BOOTLOADER}/src
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
CARGO_FLAGS      += --package=hyperion-kernel
KERNEL           := ${CARGO_DIR}/hyperion-kernel
KERNEL_TESTING   := ${KERNEL}-testing
KERNEL_SRC       := $(filter-out %: ,$(file < ${CARGO_DIR}/hyperion.d)) src/testfw.rs

# gdb
override GDB_FLAGS += --eval-command="target remote localhost:1234"
override GDB_FLAGS += --eval-command="symbol-file ${KERNEL}"

${KERNEL_SRC}:

# hyperion kernel compilation
${KERNEL}: ${KERNEL_SRC} Makefile Cargo.toml Cargo.lock
	@echo -e "\n\033[32m--[[ building Hyperion ]]--\033[0m"
	${CARGO} build ${CARGO_FLAGS}
	@touch ${KERNEL}

${KERNEL_TESTING}: ${KERNEL_SRC} Makefile Cargo.toml Cargo.lock
	@echo -e "\n\033[32m--[[ building Hyperion-Testing ]]--\033[0m"
	@${CARGO} test --no-run ${CARGO_FLAGS} # first one prints human readable errors
	${CARGO} test --no-run --message-format=json ${CARGO_FLAGS} | \
		jq -r "select(.profile.test == true) | .filenames[]" | \
		xargs -I % cp "%" ${KERNEL_TESTING}
	@touch ${KERNEL_TESTING}

# ISO generation
include ./${BOOT_DIR}/Makefile

# ISO running (including unit tests in QEMU)
include ./qemu.mk

# nextest doesn't support excluding packages
EXCLUDED_UNITS   := sample-elf kernel
unittest:
	${CARGO} test \
		--no-fail-fast \
		--workspace $(patsubst %, --exclude hyperion-%, ${EXCLUDED_UNITS}) \
		-- --test-threads=$(shell nproc --all) \
		2>&1 | rg --pcre2 --multiline --multiline-dotall -e '^test' -e 'failures:.+?(?=\n\n\n)\n\n\n' \
		2>&1 | rg -v '^test result: '
# a crazy hack that I somehow came up with to debloat the cargo test --workspace output ^^^

# build alias
build: ${KERNEL}

# bootable iso alias
iso: ${HYPERION}

clippy:
	${CARGO} clippy ${CLIPPY_FLAGS} ${CARGO_FLAGS} -- -D warnings

# connect gdb to qemu
gdb:
	gdb ${GDB_FLAGS}

kernel: ${KERNEL}
	@echo "${KERNEL}"

# objdump
objdump: ${KERNEL}
	objdump -D -C ${KERNEL}

readelf: ${KERNEL}
	readelf --all ${KERNEL}

addr2line: ${KERNEL}
	addr2line -e ${KERNEL}

src:
	@echo -e "\n\033[32m--[[ Hyperion source files ]]--\033[0m"
	@echo "from: ${CARGO_DIR}/hyperion.d"
	@echo "${KERNEL_SRC}" | tr " " "\n" | sort

.PHONY : build iso run test unittest gdb kernel objdump readelf src

# end
