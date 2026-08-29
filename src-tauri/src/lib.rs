mod android;
mod process;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            android::detect_device,
            android::reboot_device,
            android::boot_twrp,
            android::factory_reset
        ])
        .run(tauri::generate_context!())
        .expect("error while running FlashROM");
}
