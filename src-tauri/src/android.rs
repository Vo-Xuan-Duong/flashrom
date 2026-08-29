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
    boot_layout: String,
    boot_partitions: Vec<String>,
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

fn boot_partition_info(is_ab: Option<bool>) -> (String, Vec<String>) {
    match is_ab {
        Some(true) => (
            "ab".into(),
            vec!["boot_a".to_string(), "boot_b".to_string()],
        ),
        Some(false) => ("single".into(), vec!["boot".to_string()]),
        None => ("unknown".into(), Vec::new()),
    }
}

fn normalize_slot(value: &str) -> Option<String> {
    let slot = value.trim().trim_start_matches('_').to_lowercase();
    match slot.as_str() {
        "a" | "b" => Some(slot),
        _ => None,
    }
}

fn disconnected(diagnostic: impl Into<String>) -> DeviceSnapshot {
    let (boot_layout, boot_partitions) = boot_partition_info(None);
    DeviceSnapshot {
        connected: false,
        serial: None,
        mode: "Disconnected".into(),
        slot: None,
        product: None,
        tool: None,
        boot_layout,
        boot_partitions,
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

fn adb_prop(serial: &str, property: &str) -> Option<String> {
    run(
        AndroidTool::Adb,
        &["-s", serial, "shell", "getprop", property],
    )
    .ok()
    .and_then(|output| {
        let value = output.stdout.trim().to_string();
        (!value.is_empty()).then_some(value)
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
                    let (boot_layout, boot_partitions) = boot_partition_info(None);
                    return DeviceSnapshot {
                        connected: true,
                        serial: Some(serial),
                        mode: mode.into(),
                        slot: None,
                        product: None,
                        tool: Some("adb".into()),
                        boot_layout,
                        boot_partitions,
                        diagnostic: format!("ADB reports device state: {state}."),
                    };
                }

                let boot_mode = adb_prop(&serial, "ro.bootmode")
                    .unwrap_or_default()
                    .to_lowercase();
                let product = adb_prop(&serial, "ro.product.device");
                let slot = adb_prop(&serial, "ro.boot.slot_suffix")
                    .and_then(|value| normalize_slot(&value))
                    .or_else(|| {
                        adb_prop(&serial, "ro.boot.slot").and_then(|value| normalize_slot(&value))
                    });
                let ab_update = adb_prop(&serial, "ro.build.ab_update")
                    .map(|value| value.eq_ignore_ascii_case("true"));
                let is_ab = slot.as_ref().map(|_| true).or(ab_update);
                let (boot_layout, boot_partitions) = boot_partition_info(is_ab);

                return DeviceSnapshot {
                    connected: true,
                    serial: Some(serial),
                    mode: if boot_mode.contains("recovery") {
                        "Recovery".into()
                    } else {
                        "Android".into()
                    },
                    slot,
                    product,
                    tool: Some("adb".into()),
                    boot_layout,
                    boot_partitions,
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
                .and_then(|value| fastboot_var(&value, "current-slot"))
                .and_then(|value| normalize_slot(&value));
                let product = run(AndroidTool::Fastboot, &["-s", &serial, "getvar", "product"])
                    .ok()
                    .and_then(|value| fastboot_var(&value, "product"));
                let has_boot_slot = run(
                    AndroidTool::Fastboot,
                    &["-s", &serial, "getvar", "has-slot:boot"],
                )
                .ok()
                .and_then(|value| fastboot_var(&value, "has-slot:boot"))
                .and_then(|value| match value.to_lowercase().as_str() {
                    "yes" => Some(true),
                    "no" => Some(false),
                    _ => None,
                });
                let is_ab = has_boot_slot.or_else(|| slot.as_ref().map(|_| true));
                let (boot_layout, boot_partitions) = boot_partition_info(is_ab);

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
                    boot_layout,
                    boot_partitions,
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
    use super::{
        boot_partition_info, fastboot_var, normalize_slot, parse_adb_device, parse_fastboot_device,
    };
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

    #[test]
    fn normalizes_slot_suffix() {
        assert_eq!(normalize_slot("_a\n"), Some("a".into()));
        assert_eq!(normalize_slot("b"), Some("b".into()));
        assert_eq!(normalize_slot(""), None);
    }

    #[test]
    fn maps_boot_partition_layout() {
        assert_eq!(
            boot_partition_info(Some(false)),
            ("single".into(), vec!["boot".to_string()])
        );
        assert_eq!(
            boot_partition_info(Some(true)),
            (
                "ab".into(),
                vec!["boot_a".to_string(), "boot_b".to_string()]
            )
        );
    }
}
