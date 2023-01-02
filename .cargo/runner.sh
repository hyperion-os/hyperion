#!/usr/bin/env bash
#
# Hyperion x86_64 is runnable

set -xe

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
cp $KERNEL cfg/limine.cfg target/limine/limine{.sys,-cd.bin,-cd-efi.bin} $ISO_DIR

xorriso -as mkisofs \
    -b limine-cd.bin \
    -no-emul-boot -boot-load-size 4 -boot-info-table \
    --efi-boot limine-cd-efi.bin \
    -efi-boot-part --efi-boot-image --protective-msdos-label \
    $ISO_DIR -o $KERNEL.iso

# For the image to be bootable on BIOS systems, we must run `limine-deploy` on it.
target/limine/limine-deploy $KERNEL.iso

# Run the created image with QEMU.
qemu-system-x86_64 \
    -machine q35 -cpu qemu64 -M smm=off \
    -D target/log.txt -d int,guest_errors -no-reboot -no-shutdown \
    -serial stdio \
    $KERNEL.iso
