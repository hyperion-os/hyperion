MEMORY          ?= 1g
CPUS            ?= 4

QEMU_FLAGS      ?=
ifeq (${KVM},true)
QEMU_FLAGS      += -enable-kvm
endif
ifeq (${GDB},true)
QEMU_FLAGS      += -s -S
endif
QEMU_FLAGS      += -machine q35
QEMU_FLAGS      += -cpu qemu64,+rdrand,+rdseed,+rdtscp,+rdpid
QEMU_FLAGS      += -smp ${CPUS}
QEMU_FLAGS      += -m ${MEMORY}
ifeq (${KVM},true)
QEMU_FLAGS      += -M smm=off,accel=kvm
else
QEMU_FLAGS      += -M smm=off
endif
ifeq (${DEBUG},1)
QEMU_FLAGS      += -d guest_errors
else ifeq (${DEBUG},2)
QEMU_FLAGS      += -d int,guest_errors
else ifeq (${DEBUG},3)
QEMU_FLAGS      += -d int,guest_errors,cpu_reset
endif
QEMU_FLAGS      += -no-reboot
# QEMU_FLAGS      += -no-shutdown
QEMU_FLAGS      += -serial stdio
QEMU_FLAGS      += -rtc base=localtime
QEMU_OVMF       ?= /usr/share/ovmf/x64/OVMF.fd
ifeq (${UEFI},true)
QEMU_FLAGS      += -bios ${QEMU_OVMF}
endif

QEMU_RUN_FLAGS  ?=
QEMU_RUN_FLAGS  += ${QEMU_FLAGS}
# QEMU_FLAGS      += -vga virtio
QEMU_RUN_FLAGS  += -vga std
QEMU_RUN_FLAGS  += -display gtk,show-cursor=on
QEMU_RUN_FLAGS  += -usb
QEMU_RUN_FLAGS  += -device virtio-sound

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

		
