use std::{
    env, fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

use crate::{
    execution_guard::{build_execution_guard_inner, sha256_file, ExecutionGuardStep},
    final_plan::FinalFlashPlanStep,
    operation::OperationManager,
    ordering::order_final_steps,
    partition::{getvar, require_fastboot_serial},
    process::{run_streaming, AndroidTool},
};

const MODE_WAIT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullRomStepResult {
    index: usize,
    image: String,
    partition: String,
    required_mode: String,
    status: String,
    command: Option<String>,
    exit_code: Option<i32>,
    diagnostic: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullRomExecutionReport {
    operation_id: String,
    success: bool,
    journal_path: String,
    steps: Vec<FullRomStepResult>,
    clean_data_performed: bool,
    reboot_requested: bool,
    diagnostic: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecutionJournal {
    version: u8,
    operation_id: String,
    serial: String,
    product: Option<String>,
    rom_path: String,
    slot_strategy: String,
    status: String,
    started_unix_ms: u64,
    updated_unix_ms: u64,
    clean_data_requested: bool,
    reboot_requested: bool,
    steps: Vec<FullRomStepResult>,
    diagnostic: String,
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn serial_fragment(serial: &str) -> String {
    serial
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn journal_directory() -> PathBuf {
    let root = env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir);
    root.join("FlashROM").join("journals")
}

fn write_journal(path: &Path, journal: &mut ExecutionJournal) -> Result<(), String> {
    journal.updated_unix_ms = now_unix_ms();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Unable to create operation journal directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let bytes = serde_json::to_vec_pretty(journal)
        .map_err(|error| format!("Unable to serialize operation journal: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes).map_err(|error| {
        format!(
            "Unable to write operation journal {}: {error}",
            temporary.display()
        )
    })?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| {
            format!(
                "Unable to replace operation journal {}: {error}",
                path.display()
            )
        })?;
    }
    fs::rename(&temporary, path).map_err(|error| {
        format!(
            "Unable to finalize operation journal {}: {error}",
            path.display()
        )
    })
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

fn normalize_slot(value: Option<String>) -> Option<String> {
    let value = value?.trim().trim_start_matches('_').to_lowercase();
    matches!(value.as_str(), "a" | "b").then_some(value)
}

fn current_mode(serial: &str) -> Result<String, String> {
    require_fastboot_serial(serial)?;
    match parse_yes_no(getvar(serial, "is-userspace")) {
        Some(true) => Ok("FastbootD".into()),
        Some(false) => Ok("Fastboot".into()),
        None => Err("Fastboot did not report is-userspace=yes/no; execution is blocked.".into()),
    }
}

fn wait_for_mode(serial: &str, expected_mode: &str) -> Result<(), String> {
    let started = std::time::Instant::now();
    while started.elapsed() < MODE_WAIT_TIMEOUT {
        if current_mode(serial).as_deref() == Ok(expected_mode) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(750));
    }
    Err(format!(
        "Timed out waiting for device {serial} to reappear in {expected_mode}."
    ))
}

fn transition_mode(
    app: &tauri::AppHandle,
    serial: &str,
    current: &str,
    target: &str,
    operation_id: &str,
) -> Result<(), String> {
    if current == target {
        return Ok(());
    }

    let reboot_target = match target {
        "Fastboot" => "bootloader",
        "FastbootD" => "fastboot",
        _ => return Err(format!("Unsupported executor mode transition target: {target}")),
    };
    let stream_id = format!("{operation_id}-mode-{}", target.to_lowercase());
    let output = run_streaming(
        app,
        &stream_id,
        AndroidTool::Fastboot,
        &["-s", serial, "reboot", reboot_target],
    )?;
    if !output.success() {
        return Err(format!(
            "Unable to transition from {current} to {target}: {}",
            output.combined_output()
        ));
    }
    wait_for_mode(serial, target)
}

fn revalidate_step(
    serial: &str,
    expected_product: Option<&str>,
    expected_slot: Option<&str>,
    expected: &FinalFlashPlanStep,
    guarded: &ExecutionGuardStep,
) -> Result<(), String> {
    require_fastboot_serial(serial)?;

    if parse_yes_no(getvar(serial, "unlocked")) != Some(true) {
        return Err("Bootloader no longer reports unlocked=yes.".into());
    }

    if let Some(snapshot) = getvar(serial, "snapshot-update-status") {
        let snapshot = snapshot.trim().to_lowercase();
        if !snapshot.is_empty() && snapshot != "none" {
            return Err(format!(
                "Snapshot update status changed to {snapshot}; executor stopped."
            ));
        }
    }

    if let Some(product) = expected_product {
        let live_product = getvar(serial, "product")
            .ok_or_else(|| "Fastboot product could not be revalidated.".to_string())?;
        if !live_product.eq_ignore_ascii_case(product) {
            return Err(format!(
                "Connected product changed: expected {product}, found {live_product}."
            ));
        }
    }

    if let Some(slot) = expected_slot {
        let live_slot = normalize_slot(getvar(serial, "current-slot"));
        if live_slot.as_deref() != Some(slot) {
            return Err(format!(
                "Active slot changed: expected {slot}, found {}.",
                live_slot.as_deref().unwrap_or("unknown")
            ));
        }
    }

    let live_mode = current_mode(serial)?;
    if live_mode != guarded.required_mode {
        return Err(format!(
            "Device mode changed before {}: expected {}, found {live_mode}.",
            guarded.partition, guarded.required_mode
        ));
    }

    let live_size = parse_size(getvar(
        serial,
        &format!("partition-size:{}", guarded.partition),
    ));
    if live_size != expected.partition_size {
        return Err(format!(
            "Partition size changed for {}: expected {:?}, found {:?}.",
            guarded.partition, expected.partition_size, live_size
        ));
    }

    let live_logical = parse_yes_no(getvar(
        serial,
        &format!("is-logical:{}", guarded.partition),
    ));
    if live_logical != expected.logical {
        return Err(format!(
            "Logical/physical metadata changed for {}.",
            guarded.partition
        ));
    }

    let live_hash = sha256_file(&guarded.image_path, guarded.image_size)?;
    if live_hash != guarded.sha256 {
        return Err(format!(
            "SHA-256 changed for {} after Execution Guard creation.",
            guarded.image
        ));
    }

    Ok(())
}

fn post_write_check(
    serial: &str,
    expected: &FinalFlashPlanStep,
    expected_mode: &str,
) -> Result<(), String> {
    wait_for_mode(serial, expected_mode)?;
    let live_size = parse_size(getvar(
        serial,
        &format!("partition-size:{}", expected.partition),
    ));
    if live_size != expected.partition_size {
        return Err(format!(
            "Post-write partition metadata changed for {}.",
            expected.partition
        ));
    }
    Ok(())
}

fn failed_report(
    journal_path: &Path,
    journal: &mut ExecutionJournal,
    steps: Vec<FullRomStepResult>,
    clean_data_performed: bool,
    reboot_requested: bool,
    diagnostic: String,
) -> FullRomExecutionReport {
    journal.status = "failed".into();
    journal.steps = steps.clone();
    journal.diagnostic = diagnostic.clone();
    let _ = write_journal(journal_path, journal);

    FullRomExecutionReport {
        operation_id: journal.operation_id.clone(),
        success: false,
        journal_path: journal_path.to_string_lossy().to_string(),
        steps,
        clean_data_performed,
        reboot_requested,
        diagnostic,
    }
}

fn execute_inner(
    app: tauri::AppHandle,
    path: String,
    serial: String,
    slot_strategy: String,
    confirmation: String,
    clean_data_after: bool,
    reboot_after: bool,
) -> Result<FullRomExecutionReport, String> {
    let expected_confirmation = if clean_data_after {
        "FLASH ROM WIPE"
    } else {
        "FLASH ROM"
    };
    if confirmation != expected_confirmation {
        return Err(format!(
            "Full-ROM execution requires the exact confirmation phrase {expected_confirmation}."
        ));
    }

    let guard = build_execution_guard_inner(&path, &serial, &slot_strategy)?;
    if !guard.ready_for_executor {
        return Err(format!(
            "Execution Guard did not pass: {}",
            guard.diagnostic
        ));
    }

    let expected_steps = order_final_steps(&guard.final_plan.steps)?;
    if expected_steps.len() != guard.steps.len() || expected_steps.is_empty() {
        return Err("Execution Guard step count does not match the ordered Final Flash Plan.".into());
    }

    let started = now_unix_ms();
    let operation_id = format!("full-rom-{}-{started}", serial_fragment(&serial));
    let journal_path = journal_directory().join(format!("{operation_id}.json"));
    let product = guard.final_plan.compatibility.device_product.clone();
    let active_slot = guard.final_plan.active_slot.clone();
    let mut results = expected_steps
        .iter()
        .enumerate()
        .map(|(index, step)| FullRomStepResult {
            index: index + 1,
            image: step.image.clone(),
            partition: step.partition.clone(),
            required_mode: step.required_mode.clone(),
            status: "pending".into(),
            command: None,
            exit_code: None,
            diagnostic: "Waiting for execution.".into(),
        })
        .collect::<Vec<_>>();

    let mut journal = ExecutionJournal {
        version: 1,
        operation_id: operation_id.clone(),
        serial: serial.clone(),
        product: product.clone(),
        rom_path: path.clone(),
        slot_strategy: slot_strategy.clone(),
        status: "running".into(),
        started_unix_ms: started,
        updated_unix_ms: started,
        clean_data_requested: clean_data_after,
        reboot_requested: reboot_after,
        steps: results.clone(),
        diagnostic: "Full-ROM execution started after Final Plan and SHA-256 Guard validation.".into(),
    };
    write_journal(&journal_path, &mut journal)?;

    let mut mode = current_mode(&serial)?;
    let mut clean_data_performed = false;

    for (index, (expected, guarded)) in expected_steps.iter().zip(guard.steps.iter()).enumerate() {
        if mode != guarded.required_mode {
            if let Err(error) = transition_mode(
                &app,
                &serial,
                &mode,
                &guarded.required_mode,
                &operation_id,
            ) {
                results[index].status = "failed".into();
                results[index].diagnostic = error.clone();
                return Ok(failed_report(
                    &journal_path,
                    &mut journal,
                    results,
                    clean_data_performed,
                    false,
                    error,
                ));
            }
            mode = guarded.required_mode.clone();
        }

        if let Err(error) = revalidate_step(
            &serial,
            product.as_deref(),
            active_slot.as_deref(),
            expected,
            guarded,
        ) {
            results[index].status = "failed".into();
            results[index].diagnostic = format!("Pre-write revalidation failed: {error}");
            return Ok(failed_report(
                &journal_path,
                &mut journal,
                results,
                clean_data_performed,
                false,
                format!("Execution stopped before {}: {error}", guarded.partition),
            ));
        }

        results[index].status = "running".into();
        results[index].diagnostic = "Pre-write validation passed; fastboot flash is running.".into();
        journal.steps = results.clone();
        journal.diagnostic = format!("Flashing {}.", guarded.partition);
        write_journal(&journal_path, &mut journal)?;

        let stream_id = format!("{operation_id}-flash-{}", index + 1);
        let output = match run_streaming(
            &app,
            &stream_id,
            AndroidTool::Fastboot,
            &[
                "-s",
                &serial,
                "flash",
                &guarded.partition,
                &guarded.image_path,
            ],
        ) {
            Ok(output) => output,
            Err(error) => {
                results[index].status = "failed".into();
                results[index].diagnostic = error.clone();
                return Ok(failed_report(
                    &journal_path,
                    &mut journal,
                    results,
                    clean_data_performed,
                    false,
                    format!("Unable to start flash for {}: {error}", guarded.partition),
                ));
            }
        };

        results[index].command = Some(output.command.clone());
        results[index].exit_code = Some(output.status);
        if !output.success() {
            let diagnostic = format!(
                "Fastboot flash failed for {}: {}",
                guarded.partition,
                output.combined_output()
            );
            results[index].status = "failed".into();
            results[index].diagnostic = diagnostic.clone();
            return Ok(failed_report(
                &journal_path,
                &mut journal,
                results,
                clean_data_performed,
                false,
                diagnostic,
            ));
        }

        if let Err(error) = post_write_check(&serial, expected, &guarded.required_mode) {
            results[index].status = "failed".into();
            results[index].diagnostic = format!("Post-write state verification failed: {error}");
            return Ok(failed_report(
                &journal_path,
                &mut journal,
                results,
                clean_data_performed,
                false,
                format!(
                    "Flash command succeeded for {}, but post-write state verification failed: {error}",
                    guarded.partition
                ),
            ));
        }

        results[index].status = "success".into();
        results[index].diagnostic =
            "Fastboot reported success and the expected device/partition state remained available.".into();
        journal.steps = results.clone();
        journal.diagnostic = format!("Completed {}.", guarded.partition);
        write_journal(&journal_path, &mut journal)?;
    }

    if clean_data_after {
        if mode != "Fastboot" {
            if let Err(error) = transition_mode(&app, &serial, &mode, "Fastboot", &operation_id) {
                return Ok(failed_report(
                    &journal_path,
                    &mut journal,
                    results,
                    clean_data_performed,
                    false,
                    format!("ROM flashing completed, but Clean Data mode transition failed: {error}"),
                ));
            }
            mode = "Fastboot".into();
        }

        let wipe_id = format!("{operation_id}-wipe");
        let output = run_streaming(
            &app,
            &wipe_id,
            AndroidTool::Fastboot,
            &["-s", &serial, "-w"],
        )?;
        if !output.success() {
            return Ok(failed_report(
                &journal_path,
                &mut journal,
                results,
                clean_data_performed,
                false,
                format!(
                    "ROM flashing completed, but Clean Data failed: {}",
                    output.combined_output()
                ),
            ));
        }
        clean_data_performed = true;
    }

    let mut reboot_requested = false;
    if reboot_after {
        require_fastboot_serial(&serial)?;
        let reboot_id = format!("{operation_id}-reboot");
        let output = run_streaming(
            &app,
            &reboot_id,
            AndroidTool::Fastboot,
            &["-s", &serial, "reboot"],
        )?;
        if !output.success() {
            return Ok(failed_report(
                &journal_path,
                &mut journal,
                results,
                clean_data_performed,
                false,
                format!(
                    "All partition writes completed, but reboot failed: {}",
                    output.combined_output()
                ),
            ));
        }
        reboot_requested = true;
    } else {
        require_fastboot_serial(&serial)?;
        if let Some(expected_product) = product.as_deref() {
            let live_product = getvar(&serial, "product")
                .ok_or_else(|| "Final Fastboot product check did not return a value.".to_string())?;
            if !live_product.eq_ignore_ascii_case(expected_product) {
                return Ok(failed_report(
                    &journal_path,
                    &mut journal,
                    results,
                    clean_data_performed,
                    false,
                    "Final Fastboot product changed after execution.".into(),
                ));
            }
        }
    }

    journal.status = "completed".into();
    journal.steps = results.clone();
    journal.diagnostic = if reboot_requested {
        "All guarded partition writes completed and Android reboot was requested.".into()
    } else {
        format!(
            "All guarded partition writes completed. Device remains available in {mode}; reboot is left to the user."
        )
    };
    write_journal(&journal_path, &mut journal)?;

    Ok(FullRomExecutionReport {
        operation_id,
        success: true,
        journal_path: journal_path.to_string_lossy().to_string(),
        steps: results,
        clean_data_performed,
        reboot_requested,
        diagnostic: journal.diagnostic,
    })
}

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
) -> Result<FullRomExecutionReport, String> {
    if serial.trim().is_empty() {
        return Err("A selected Fastboot serial is required for Full-ROM execution.".into());
    }

    let permit = manager
        .inner()
        .clone()
        .acquire("full-rom-executor", &serial)?;

    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        execute_inner(
            app,
            path,
            serial,
            slot_strategy,
            confirmation,
            clean_data_after,
            reboot_after,
        )
    })
    .await
    .map_err(|error| format!("Full-ROM executor worker failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::{normalize_slot, parse_size, parse_yes_no, serial_fragment};

    #[test]
    fn parses_fastboot_guard_values() {
        assert_eq!(parse_yes_no(Some("yes".into())), Some(true));
        assert_eq!(parse_size(Some("0x4000".into())), Some(16_384));
        assert_eq!(normalize_slot(Some("_B".into())), Some("b".into()));
    }

    #[test]
    fn sanitizes_serial_for_journal_filename() {
        assert_eq!(serial_fragment("ABC:123/XYZ"), "ABC_123_XYZ");
    }
}
