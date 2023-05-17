MEMORY          ?= 256m

QEMU_FLAGS      ?=
ifeq (${KVM},true)
QEMU_FLAGS      += -enable-kvm
endif
ifeq (${GDB},true)
QEMU_FLAGS      += -s -S
endif
QEMU_FLAGS      += -machine q35
QEMU_FLAGS      += -cpu qemu64,+rdrand,+rdseed
QEMU_FLAGS      += -smp 4
QEMU_FLAGS      += -m ${MEMORY}
QEMU_FLAGS      += -M smm=off,accel=kvm
# QEMU_FLAGS      += -M smm=off
# QEMU_FLAGS      += -d int,guest_errors,cpu_reset
# QEMU_FLAGS      += -d int,guest_errors
QEMU_FLAGS      += -d guest_errors
QEMU_FLAGS      += -no-reboot
QEMU_FLAGS      += -serial stdio
QEMU_FLAGS      += -rtc base=localtime
QEMU_OVMF       ?= /usr/share/ovmf/x64/OVMF.fd
ifeq (${UEFI},true)
QEMU_FLAGS      += -bios ${QEMU_OVMF}
endif
# QEMU_FLAGS      += -vga virtio
QEMU_FLAGS      += -vga std

QEMU_RUN_FLAGS  ?=
QEMU_RUN_FLAGS  += ${QEMU_FLAGS}

QEMU_TEST_FLAGS ?=
QEMU_TEST_FLAGS += ${QEMU_FLAGS}
QEMU_TEST_FLAGS += -device isa-debug-exit,iobase=0xf4,iosize=0x04
QEMU_TEST_FLAGS += -display none

QEMU_KERNEL     := -kernel ${KERNEL} -append qemu
QEMU_DRIVE      := -drive format=raw,file

# TODO: multiboot1 direct kernel boot

# qemu normal run
run: ${HYPERION}
	@echo -e "\n\033[32m--[[ running Hyperion in QEMU ]]--\033[0m"
	${QEMU} ${QEMU_RUN_FLAGS} ${QEMU_DRIVE}=${HYPERION}

# run tests in qemu
test: ${HYPERION_TESTING}
	@echo -e "\n\033[32m--[[ running Hyperion-Testing in QEMU ]]--\033[0m"
	${QEMU} ${QEMU_TEST_FLAGS} ${QEMU_DRIVE}=${HYPERION_TESTING};\
	[ $$? -ne 33 ] && exit 1;\
	exit 0

		
