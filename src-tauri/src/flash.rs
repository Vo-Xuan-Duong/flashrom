use std::{fs, path::Path};

use serde::Serialize;

use crate::process::{run, run_streaming, AndroidTool, CommandOutput};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashExecutionResult {
    command: String,
    success: bool,
    status: i32,
    output: String,
    partition: String,
    image_size: u64,
    partition_size: u64,
    required_mode: String,
    product: Option<String>,
}

fn fastboot_var(output: &CommandOutput, key: &str) -> Option<String> {
    let marker = format!("{key}:");
    output.combined_output().lines().find_map(|line| {
        let position = line.find(&marker)?;
        let value = line[position + marker.len()..].trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn getvar(serial: &str, key: &str) -> Option<String> {
    run(AndroidTool::Fastboot, &["-s", serial, "getvar", key])
        .ok()
        .and_then(|output| fastboot_var(&output, key))
}

fn parse_yes_no(value: Option<String>) -> Option<bool> {
    match value?.trim().to_lowercase().as_str() {
        "yes" | "true" | "1" => Some(true),
        "no" | "false" | "0" => Some(false),
        _ => None,
    }
}

fn parse_size(value: Option<String>) -> Option<u64> {
    let value = value?;
    let trimmed = value.trim();

    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u64::from_str_radix(hex, 16).ok();
    }

    trimmed.parse::<u64>().ok()
}

fn fastboot_serial_available(serial: &str) -> Result<bool, String> {
    let output = run(AndroidTool::Fastboot, &["devices"])?;
    Ok(output.stdout.lines().any(|line| {
        line.split_whitespace()
            .next()
            .map(|value| value == serial)
            .unwrap_or(false)
    }))
}

fn validate_partition_name(partition: &str) -> Result<(), String> {
    if partition.is_empty()
        || !partition
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(format!("Invalid partition name: {partition}"));
    }
    Ok(())
}

#[tauri::command]
pub async fn flash_image(
    app: tauri::AppHandle,
    serial: String,
    partition: String,
    image_path: String,
    confirmation: String,
) -> Result<FlashExecutionResult, String> {
    if confirmation != "FLASH" {
        return Err("Manual image flashing requires the exact confirmation value FLASH.".into());
    }

    if serial.trim().is_empty() {
        return Err("A detected Fastboot device serial is required.".into());
    }

    validate_partition_name(&partition)?;

    if !fastboot_serial_available(&serial)? {
        return Err(format!(
            "Device {serial} is not currently available through Fastboot."
        ));
    }

    let image = Path::new(&image_path);
    let metadata = fs::metadata(image)
        .map_err(|error| format!("Unable to access selected image: {error}"))?;
    if !metadata.is_file() {
        return Err("Selected image path is not a regular file.".into());
    }

    let is_img = image
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("img"))
        .unwrap_or(false);
    if !is_img {
        return Err("Manual Fastboot flashing only accepts .img files.".into());
    }

    if parse_yes_no(getvar(&serial, "unlocked")) != Some(true) {
        return Err(
            "Bootloader unlock state could not be confirmed as unlocked=yes. Flashing is blocked."
                .into(),
        );
    }

    if let Some(snapshot_state) = getvar(&serial, "snapshot-update-status") {
        let state = snapshot_state.trim().to_lowercase();
        if !state.is_empty() && state != "none" {
            return Err(format!(
                "Snapshot update status is {snapshot_state}. Flashing is blocked until snapshot operations finish."
            ));
        }
    }

    let partition_size = parse_size(getvar(
        &serial,
        &format!("partition-size:{partition}"),
    ))
    .ok_or_else(|| format!("Partition {partition} does not report a usable partition size."))?;

    let image_size = metadata.len();
    if image_size > partition_size {
        return Err(format!(
            "Image is larger than partition {partition}: image={image_size} bytes, partition={partition_size} bytes."
        ));
    }

    let logical = parse_yes_no(getvar(&serial, &format!("is-logical:{partition}")))
        .ok_or_else(|| format!("Logical/physical status for {partition} could not be confirmed."))?;
    let userspace = parse_yes_no(getvar(&serial, "is-userspace")).unwrap_or(false);

    let required_mode = if logical { "FastbootD" } else { "Fastboot" };
    if logical && !userspace {
        return Err(format!(
            "Partition {partition} is logical and must be flashed from FastbootD."
        ));
    }
    if !logical && userspace {
        return Err(format!(
            "Partition {partition} is physical. Reboot to classic Bootloader/Fastboot before flashing."
        ));
    }

    let product = getvar(&serial, "product");
    let operation_id = format!("fastboot-flash-{partition}");
    let serial_for_run = serial.clone();
    let partition_for_run = partition.clone();
    let image_path_for_run = image_path.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        run_streaming(
            &app,
            &operation_id,
            AndroidTool::Fastboot,
            &[
                "-s",
                &serial_for_run,
                "flash",
                &partition_for_run,
                &image_path_for_run,
            ],
        )
    })
    .await
    .map_err(|error| format!("Fastboot flash worker failed: {error}"))??;

    Ok(FlashExecutionResult {
        command: result.command,
        success: result.success(),
        status: result.status,
        output: result.combined_output(),
        partition,
        image_size,
        partition_size,
        required_mode: required_mode.into(),
        product,
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_size, parse_yes_no, validate_partition_name};

    #[test]
    fn validates_partition_names() {
        assert!(validate_partition_name("boot_a").is_ok());
        assert!(validate_partition_name("vendor_boot").is_ok());
        assert!(validate_partition_name("boot;erase").is_err());
        assert!(validate_partition_name("").is_err());
    }

    #[test]
    fn parses_fastboot_values() {
        assert_eq!(parse_yes_no(Some("yes".into())), Some(true));
        assert_eq!(parse_size(Some("0x4000000".into())), Some(67_108_864));
    }
}
