use serde::Serialize;

use crate::process::{run, AndroidTool, CommandOutput};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSnapshot {
    connected: bool,
    serial: Option<String>,
    mode: String,
    slot: Option<String>,
    product: Option<String>,
    tool: Option<String>,
    diagnostic: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResult {
    command: String,
    success: bool,
    status: i32,
    output: String,
}

fn disconnected(diagnostic: impl Into<String>) -> DeviceSnapshot {
    DeviceSnapshot {
        connected: false,
        serial: None,
        mode: "Disconnected".into(),
        slot: None,
        product: None,
        tool: None,
        diagnostic: diagnostic.into(),
    }
}

fn parse_adb_device(output: &str) -> Option<(String, String)> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("List of devices"))
        .find_map(|line| {
            let mut parts = line.split_whitespace();
            let serial = parts.next()?;
            let state = parts.next()?;
            Some((serial.to_string(), state.to_string()))
        })
}

fn parse_fastboot_device(output: &str) -> Option<String> {
    output.lines().map(str::trim).find_map(|line| {
        if line.is_empty() {
            return None;
        }
        line.split_whitespace().next().map(str::to_string)
    })
}

fn fastboot_var(output: &CommandOutput, key: &str) -> Option<String> {
    let marker = format!("{key}:");
    output.combined_output().lines().find_map(|line| {
        let position = line.find(&marker)?;
        let value = line[position + marker.len()..].trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn detect_inner() -> DeviceSnapshot {
    let mut diagnostics = Vec::new();

    match run(AndroidTool::Adb, &["devices"]) {
        Ok(output) => {
            if let Some((serial, state)) = parse_adb_device(&output.stdout) {
                if state != "device" {
                    let mode = match state.as_str() {
                        "unauthorized" => "ADB Unauthorized",
                        "offline" => "ADB Offline",
                        _ => "Android",
                    };
                    return DeviceSnapshot {
                        connected: true,
                        serial: Some(serial),
                        mode: mode.into(),
                        slot: None,
                        product: None,
                        tool: Some("adb".into()),
                        diagnostic: format!("ADB reports device state: {state}."),
                    };
                }

                let boot_mode = run(
                    AndroidTool::Adb,
                    &["-s", &serial, "shell", "getprop", "ro.bootmode"],
                )
                .ok()
                .map(|value| value.stdout.trim().to_lowercase())
                .unwrap_or_default();
                let product = run(
                    AndroidTool::Adb,
                    &["-s", &serial, "shell", "getprop", "ro.product.device"],
                )
                .ok()
                .and_then(|value| {
                    let product = value.stdout.trim().to_string();
                    (!product.is_empty()).then_some(product)
                });

                return DeviceSnapshot {
                    connected: true,
                    serial: Some(serial),
                    mode: if boot_mode.contains("recovery") {
                        "Recovery".into()
                    } else {
                        "Android".into()
                    },
                    slot: None,
                    product,
                    tool: Some("adb".into()),
                    diagnostic: "Device detected through ADB.".into(),
                };
            }
        }
        Err(error) => diagnostics.push(error),
    }

    match run(AndroidTool::Fastboot, &["devices"]) {
        Ok(output) => {
            if let Some(serial) = parse_fastboot_device(&output.stdout) {
                let userspace = run(
                    AndroidTool::Fastboot,
                    &["-s", &serial, "getvar", "is-userspace"],
                )
                .ok()
                .and_then(|value| fastboot_var(&value, "is-userspace"));
                let slot = run(
                    AndroidTool::Fastboot,
                    &["-s", &serial, "getvar", "current-slot"],
                )
                .ok()
                .and_then(|value| fastboot_var(&value, "current-slot"));
                let product = run(AndroidTool::Fastboot, &["-s", &serial, "getvar", "product"])
                    .ok()
                    .and_then(|value| fastboot_var(&value, "product"));

                let mode = if userspace.as_deref() == Some("yes") {
                    "FastbootD"
                } else {
                    "Fastboot"
                };

                return DeviceSnapshot {
                    connected: true,
                    serial: Some(serial),
                    mode: mode.into(),
                    slot,
                    product,
                    tool: Some("fastboot".into()),
                    diagnostic: "Device detected through Fastboot.".into(),
                };
            }
        }
        Err(error) => diagnostics.push(error),
    }

    if diagnostics.is_empty() {
        disconnected("ADB and Fastboot are available, but no device was detected.")
    } else {
        disconnected(diagnostics.join(" | "))
    }
}

#[tauri::command]
pub fn detect_device() -> Result<DeviceSnapshot, String> {
    Ok(detect_inner())
}

fn action_result(output: CommandOutput) -> ActionResult {
    ActionResult {
        command: output.command,
        success: output.success(),
        status: output.status,
        output: output.combined_output(),
    }
}

#[tauri::command]
pub fn reboot_device(target: String) -> Result<ActionResult, String> {
    let device = detect_inner();
    if !device.connected {
        return Err(device.diagnostic);
    }

    let transport = device.tool.as_deref().unwrap_or_default();
    let output = match (transport, target.as_str()) {
        ("adb", "android") => run(AndroidTool::Adb, &["reboot"]),
        ("adb", "bootloader") => run(AndroidTool::Adb, &["reboot", "bootloader"]),
        ("adb", "fastbootd") => run(AndroidTool::Adb, &["reboot", "fastboot"]),
        ("adb", "recovery") => run(AndroidTool::Adb, &["reboot", "recovery"]),
        ("fastboot", "android") => run(AndroidTool::Fastboot, &["reboot"]),
        ("fastboot", "bootloader") => run(AndroidTool::Fastboot, &["reboot", "bootloader"]),
        ("fastboot", "fastbootd") => run(AndroidTool::Fastboot, &["reboot", "fastboot"]),
        ("fastboot", "recovery") => run(AndroidTool::Fastboot, &["reboot", "recovery"]),
        (_, _) => return Err(format!("Unsupported reboot target: {target}")),
    }?;

    Ok(action_result(output))
}

#[cfg(test)]
mod tests {
    use super::{fastboot_var, parse_adb_device, parse_fastboot_device};
    use crate::process::CommandOutput;

    #[test]
    fn parses_adb_devices() {
        let value = "List of devices attached\nABC123\tdevice\n";
        assert_eq!(
            parse_adb_device(value),
            Some(("ABC123".into(), "device".into()))
        );
    }

    #[test]
    fn parses_fastboot_devices() {
        assert_eq!(
            parse_fastboot_device("ABC123\tfastboot\n"),
            Some("ABC123".into())
        );
    }

    #[test]
    fn parses_fastboot_getvar_from_stderr() {
        let output = CommandOutput {
            command: "fastboot getvar current-slot".into(),
            status: 0,
            stdout: String::new(),
            stderr: "current-slot: b\nFinished. Total time: 0.001s\n".into(),
        };
        assert_eq!(fastboot_var(&output, "current-slot"), Some("b".into()));
    }
}
