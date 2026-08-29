mod android;
mod partition;
mod plan;
mod process;
mod rom;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            android::detect_device,
            android::reboot_device,
            android::boot_twrp,
            android::factory_reset,
            rom::inspect_rom,
            plan::generate_flash_plan,
            partition::inspect_partitions
        ])
        .run(tauri::generate_context!())
        .expect("error while running FlashROM");
}
