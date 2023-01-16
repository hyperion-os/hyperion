#!/usr/bin/env bash
#
# Hyperion x86_64 is runnable

set -xe

echo $@

LIMINE_GIT_URL="https://github.com/limine-bootloader/limine.git"
ISO_DIR=target/hyperion/x86_64/iso
KERNEL=$1

# Clone the `limine` repository if we don't have it yet.
if [ ! -d target/limine ]; then
    git clone $LIMINE_GIT_URL --depth=1 --branch v3.0-branch-binary target/limine
fi

# Make sure we have an up-to-date version of the bootloader.
cd target/limine
git fetch
make
cd -

# Copy the needed files into an ISO image.
mkdir -p $ISO_DIR
cp cfg/limine.cfg target/limine/limine{.sys,-cd.bin,-cd-efi.bin} $ISO_DIR
cp $KERNEL $ISO_DIR/hyperion

xorriso -as mkisofs \
    -b limine-cd.bin \
    -no-emul-boot -boot-load-size 4 -boot-info-table \
    --efi-boot limine-cd-efi.bin \
    -efi-boot-part --efi-boot-image --protective-msdos-label \
    $ISO_DIR -o $KERNEL.iso

# For the image to be bootable on BIOS systems, we must run `limine-deploy` on it.
target/limine/limine-deploy $KERNEL.iso

# A hack to detect if the kernel is a testing kernel
# Cargo test binary generates a 'random id' for testing binaries
if [ "$(basename $KERNEL)" = "hyperion" ]; then
    # Run the created image with QEMU.
    qemu-system-x86_64 \
        -enable-kvm \
        -machine q35 \
        -cpu qemu64 \
        -smp 8 \
        -M smm=off \
        -d int,guest_errors,cpu_reset \
        -no-reboot \
        -serial stdio \
        $KERNEL.iso
    #-s -S \
    #-no-shutdown \
    #-D target/log.txt \
else
    set +e
    # Run the created image with QEMU.
    qemu-system-x86_64 \
        -enable-kvm \
        -machine q35 \
        -cpu qemu64 \
        -smp 8 \
        -M smm=off \
        -d int,guest_errors,cpu_reset \
        -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
        -no-reboot \
        -serial stdio \
        -display none \
        $KERNEL.iso
    #-no-shutdown \
    #-D target/log.txt \

    [ $? -ne 33 ] && exit 1
    exit 0
fi
