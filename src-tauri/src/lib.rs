mod android;
mod compatibility;
mod execution_guard;
mod execution_preview;
mod final_plan;
mod flash;
mod ordering;
mod partition;
mod plan;
mod process;
mod recovery;
mod restore;
mod restore_profile;
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
            partition::inspect_partitions,
            compatibility::inspect_rom_compatibility,
            final_plan::resolve_final_flash_plan,
            execution_preview::build_execution_preview,
            execution_guard::build_execution_guard,
            restore::scan_restore_profile,
            restore::backup_restore_apks,
            restore::restore_local_apks,
            restore::verify_restore_packages,
            restore_profile::save_restore_profile,
            restore_profile::load_restore_profile,
            recovery::adb_sideload,
            flash::flash_image
        ])
        .run(tauri::generate_context!())
        .expect("error while running FlashROM");
}
