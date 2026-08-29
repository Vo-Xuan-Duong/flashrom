use std::path::Path;

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
                        "sideload" => "ADB Sideload",
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

fn require_serial(device: &DeviceSnapshot) -> Result<&str, String> {
    if !device.connected {
        return Err(device.diagnostic.clone());
    }

    device
        .serial
        .as_deref()
        .ok_or_else(|| "Connected device has no serial number.".to_string())
}

fn require_classic_fastboot(device: &DeviceSnapshot) -> Result<&str, String> {
    let serial = require_serial(device)?;

    if device.tool.as_deref() != Some("fastboot") {
        return Err("This action requires Fastboot. Reboot the device to Bootloader first.".into());
    }

    if device.mode != "Fastboot" {
        return Err(format!(
            "This action requires classic Fastboot, but the device is in {}. Reboot to Bootloader first.",
            device.mode
        ));
    }

    Ok(serial)
}

#[tauri::command]
pub fn reboot_device(target: String) -> Result<ActionResult, String> {
    let device = detect_inner();
    let serial = require_serial(&device)?.to_string();
    let transport = device.tool.as_deref().unwrap_or_default();

    let output = match (transport, target.as_str()) {
        ("adb", "android") => run(AndroidTool::Adb, &["-s", &serial, "reboot"]),
        ("adb", "bootloader") => run(AndroidTool::Adb, &["-s", &serial, "reboot", "bootloader"]),
        ("adb", "fastbootd") => run(AndroidTool::Adb, &["-s", &serial, "reboot", "fastboot"]),
        ("adb", "recovery") => run(AndroidTool::Adb, &["-s", &serial, "reboot", "recovery"]),
        ("fastboot", "android") => run(AndroidTool::Fastboot, &["-s", &serial, "reboot"]),
        ("fastboot", "bootloader") => run(
            AndroidTool::Fastboot,
            &["-s", &serial, "reboot", "bootloader"],
        ),
        ("fastboot", "fastbootd") => run(
            AndroidTool::Fastboot,
            &["-s", &serial, "reboot", "fastboot"],
        ),
        ("fastboot", "recovery") => run(
            AndroidTool::Fastboot,
            &["-s", &serial, "reboot", "recovery"],
        ),
        (_, _) => return Err(format!("Unsupported reboot target: {target}")),
    }?;

    Ok(action_result(output))
}

#[tauri::command]
pub fn boot_twrp(image_path: String) -> Result<ActionResult, String> {
    let device = detect_inner();
    let serial = require_classic_fastboot(&device)?.to_string();
    let image = Path::new(&image_path);

    if !image.is_file() {
        return Err("The selected TWRP image does not exist or is not a file.".into());
    }

    let is_img = image
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("img"))
        .unwrap_or(false);

    if !is_img {
        return Err("TWRP boot requires an .img file.".into());
    }

    let output = run(
        AndroidTool::Fastboot,
        &["-s", &serial, "boot", image_path.as_str()],
    )?;

    Ok(action_result(output))
}

#[tauri::command]
pub fn factory_reset(confirmation: String) -> Result<ActionResult, String> {
    if confirmation != "WIPE" {
        return Err("Factory reset confirmation must exactly match WIPE.".into());
    }

    let device = detect_inner();
    let serial = require_classic_fastboot(&device)?.to_string();
    let output = run(AndroidTool::Fastboot, &["-s", &serial, "-w"])?;

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
