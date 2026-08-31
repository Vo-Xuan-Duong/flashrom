use std::time::Duration;

use serde_json::{json, Value};

use crate::{
    boot_verify::wait_for_android_boot_inner, executor, operation::OperationManager,
    partition::getvar,
};

const POST_FLASH_BOOT_TIMEOUT: Duration = Duration::from_secs(300);

#[tauri::command]
pub async fn execute_full_rom(
    app: tauri::AppHandle,
    manager: tauri::State<'_, OperationManager>,
    path: String,
    serial: String,
    slot_strategy: String,
    confirmation: String,
    clean_data_after: bool,
    reboot_after: bool,
) -> Result<Value, String> {
    let expected_product = getvar(&serial, "product");
    let operation_manager = manager.inner().clone();

    let report = executor::execute_full_rom(
        app,
        manager,
        path,
        serial.clone(),
        slot_strategy,
        confirmation,
        clean_data_after,
        reboot_after,
    )
    .await?;

    let mut value = serde_json::to_value(&report)
        .map_err(|error| format!("Unable to serialize Full-ROM report: {error}"))?;
    let execution_success = value
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if !execution_success || !reboot_after {
        return Ok(value);
    }

    let permit = operation_manager.acquire("post-flash-boot-verification", &serial)?;
    let verification_serial = serial.clone();
    let verification = tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        wait_for_android_boot_inner(
            &verification_serial,
            expected_product.as_deref(),
            POST_FLASH_BOOT_TIMEOUT,
        )
    })
    .await
    .map_err(|error| format!("Post-flash boot verification worker failed: {error}"))??;

    let object = value
        .as_object_mut()
        .ok_or_else(|| "Full-ROM report did not serialize as an object.".to_string())?;
    object.insert(
        "bootVerification".into(),
        serde_json::to_value(&verification)
            .map_err(|error| format!("Unable to serialize boot verification: {error}"))?,
    );

    if !verification.verified {
        object.insert("success".into(), json!(false));
        object.insert(
            "diagnostic".into(),
            json!(format!(
                "All guarded partition writes completed, but Android boot verification failed: {}",
                verification.diagnostic
            )),
        );
    } else {
        object.insert(
            "diagnostic".into(),
            json!(format!(
                "All guarded partition writes completed and Android boot was verified: {}",
                verification.diagnostic
            )),
        );
    }

    Ok(value)
}
