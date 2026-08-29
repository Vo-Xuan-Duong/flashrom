use std::path::Path;

use serde::Serialize;

use crate::process::{run, run_streaming, AndroidTool, CommandOutput};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingActionResult {
    command: String,
    success: bool,
    status: i32,
    output: String,
}

fn action_result(output: CommandOutput) -> StreamingActionResult {
    StreamingActionResult {
        command: output.command,
        success: output.success(),
        status: output.status,
        output: output.combined_output(),
    }
}

fn adb_state(serial: &str) -> Result<Option<String>, String> {
    let output = run(AndroidTool::Adb, &["devices"])?;

    Ok(output.stdout.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let found_serial = fields.next()?;
        let state = fields.next()?;
        (found_serial == serial).then(|| state.to_string())
    }))
}

#[tauri::command]
pub async fn adb_sideload(
    app: tauri::AppHandle,
    serial: String,
    zip_path: String,
) -> Result<StreamingActionResult, String> {
    if serial.trim().is_empty() {
        return Err("A detected ADB device serial is required for sideload.".into());
    }

    let zip = Path::new(&zip_path);
    if !zip.is_file() {
        return Err("The selected ROM ZIP does not exist or is not a file.".into());
    }

    let is_zip = zip
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("zip"))
        .unwrap_or(false);

    if !is_zip {
        return Err("ADB sideload requires a .zip package.".into());
    }

    let state = adb_state(&serial)?;
    if state.as_deref() != Some("sideload") {
        return Err(format!(
            "Device {serial} is not in ADB sideload mode (current state: {}). Start ADB Sideload in recovery, then refresh detection.",
            state.as_deref().unwrap_or("not detected")
        ));
    }

    tauri::async_runtime::spawn_blocking(move || {
        let output = run_streaming(
            &app,
            "adb-sideload",
            AndroidTool::Adb,
            &["-s", &serial, "sideload", &zip_path],
        )?;
        Ok(action_result(output))
    })
    .await
    .map_err(|error| format!("ADB sideload worker failed: {error}"))?
}
