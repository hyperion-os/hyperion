#![no_std]
#![no_main]

const CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.kernel_stack_size = 4096 * 4; // 16KiB
    config
};

bootloader_api::entry_point!(hyperion_kernel::kernel_main, config = &CONFIG);
