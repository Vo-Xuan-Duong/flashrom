mod android;
mod boot_verify;
mod compatibility;
mod execution_guard;
mod execution_preview;
mod executor;
mod final_plan;
mod flash;
mod journal;
mod operation;
mod ordering;
mod partition;
mod plan;
mod platform_tools;
mod process;
mod recovery;
mod restore;
mod restore_profile;
mod rom;
mod special_tools;
mod verified_executor;
mod zip_inspection;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(operation::OperationManager::default())
        .invoke_handler(tauri::generate_handler![
            android::detect_device,
            android::list_devices,
            android::reboot_device,
            android::boot_twrp,
            android::factory_reset,
            boot_verify::verify_android_boot,
            platform_tools::inspect_platform_tools,
            special_tools::inspect_special_tools,
            special_tools::prepare_payload_input,
            special_tools::prepare_super_input,
            rom::inspect_rom,
            zip_inspection::inspect_rom_zip,
            zip_inspection::extract_rom_zip_inputs,
            plan::generate_flash_plan,
            partition::inspect_partitions,
            compatibility::inspect_rom_compatibility,
            final_plan::resolve_final_flash_plan,
            execution_preview::build_execution_preview,
            execution_guard::build_execution_guard,
            verified_executor::execute_full_rom,
            journal::list_execution_journals,
            journal::inspect_execution_journal,
            journal::delete_execution_journal,
            operation::get_operation_status,
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
