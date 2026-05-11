#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

extern crate alloc;
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::main;
use esp_hal::time::{
    Duration,
    Instant,
};
use esp_nvs::Key;
use log::info;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

// ---------------------------------------------------------------------------
// Chip-specific partition constants
// ---------------------------------------------------------------------------
// Adjust PARTITION_OFFSET and PARTITION_SIZE to match your chip and partition
//   table.  The values below serve as reasonable defaults for each chip
//   family, but your actual layout may differ — always verify against the
//   partition.csv used when flashing.
//
// Common ways to find the NVS offset & size:
//   - Inspect `partitions.csv` in your project
//   - Run `cargo espflash partition-table` (if supported by your runner)
//   - Check the ESP-IDF default partition table for your chip
// ---------------------------------------------------------------------------

cfg_if::cfg_if! {
    if #[cfg(feature = "esp32c61")] {
        // ESP32-C61 (8 MB flash, custom partition table)
        const PARTITION_OFFSET: usize = 0x390000;
        const PARTITION_SIZE:   usize = 0x32000;
    } else if #[cfg(feature = "esp32c6")] {
        // ESP32-C6 (4 MB flash)
        const PARTITION_OFFSET: usize = 0x390000;
        const PARTITION_SIZE:   usize = 0x32000;
    } else if #[cfg(feature = "esp32c5")] {
        // ESP32-C5 (4 MB flash)
        const PARTITION_OFFSET: usize = 0x390000;
        const PARTITION_SIZE:   usize = 0x32000;
    } else {
        // Default ESP-IDF single-factory layout for 4 MB flash:
        //   nvs      data nvs     0x9000   0x6000
        //   phy_init data phy     0xf000   0x1000
        //   factory  app  factory 0x10000  1.75 MB
        const PARTITION_OFFSET: usize = 0x9000;
        const PARTITION_SIZE:   usize = 0x6000;
    }
}

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[main]
fn main() -> ! {
    // generator version: 1.3.0
    // generator parameters: --chip esp32c61 -o esp32c61-wroom-1 -o unstable-hal -o alloc -o log -o
    // esp-backtrace -o vscode

    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 65536);

    let storage = esp_storage::FlashStorage::new(peripherals.FLASH);

    let mut nvs = esp_nvs::Nvs::new(PARTITION_OFFSET, PARTITION_SIZE, storage).expect("failed to create nvs");
    let namespace = &Key::from_str("test");
    let key_str = &Key::from_str("world");
    nvs.set(namespace, key_str, "123");
    let value: alloc::string::String = nvs.get(namespace, key_str).unwrap_or_default();
    nvs.delete(namespace, key_str);

    loop {
        info!("Hello world!");
        let delay_start = Instant::now();
        while delay_start.elapsed() < Duration::from_millis(500) {}
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.1.0/examples
}
