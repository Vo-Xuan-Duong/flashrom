use std::{
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::process::{run, AndroidTool};

const DEFAULT_BOOT_TIMEOUT: Duration = Duration::from_secs(300);
const POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidBootVerification {
    pub verified: bool,
    pub serial: String,
    pub product: Option<String>,
    pub android_release: Option<String>,
    pub build_fingerprint: Option<String>,
    pub boot_completed: bool,
    pub elapsed_ms: u64,
    pub diagnostic: String,
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

fn adb_prop(serial: &str, key: &str) -> Option<String> {
    run(AndroidTool::Adb, &["-s", serial, "shell", "getprop", key])
        .ok()
        .and_then(|output| {
            let value = output.stdout.trim().to_string();
            (!value.is_empty()).then_some(value)
        })
}

pub(crate) fn wait_for_android_boot_inner(
    serial: &str,
    expected_product: Option<&str>,
    timeout: Duration,
) -> Result<AndroidBootVerification, String> {
    if serial.trim().is_empty() {
        return Err("A selected device serial is required for Android boot verification.".into());
    }

    let started = Instant::now();
    let mut last_state = None;

    while started.elapsed() < timeout {
        match adb_state(serial) {
            Ok(state) => {
                last_state = state.clone();
                if state.as_deref() == Some("device") {
                    let boot_completed = adb_prop(serial, "sys.boot_completed")
                        .map(|value| value.trim() == "1")
                        .unwrap_or(false);
                    if boot_completed {
                        let product = adb_prop(serial, "ro.product.device");
                        if let Some(expected) = expected_product {
                            if product
                                .as_deref()
                                .map(|value| value.eq_ignore_ascii_case(expected))
                                != Some(true)
                            {
                                return Ok(AndroidBootVerification {
                                    verified: false,
                                    serial: serial.to_string(),
                                    product,
                                    android_release: adb_prop(serial, "ro.build.version.release"),
                                    build_fingerprint: adb_prop(serial, "ro.build.fingerprint"),
                                    boot_completed: true,
                                    elapsed_ms: started.elapsed().as_millis() as u64,
                                    diagnostic: format!(
                                        "Android booted, but ro.product.device does not match expected product {expected}."
                                    ),
                                });
                            }
                        }

                        let android_release = adb_prop(serial, "ro.build.version.release");
                        let build_fingerprint = adb_prop(serial, "ro.build.fingerprint");
                        return Ok(AndroidBootVerification {
                            verified: true,
                            serial: serial.to_string(),
                            product,
                            android_release,
                            build_fingerprint,
                            boot_completed: true,
                            elapsed_ms: started.elapsed().as_millis() as u64,
                            diagnostic: "Android reported sys.boot_completed=1 and device identity remained consistent."
                                .into(),
                        });
                    }
                }
            }
            Err(_) => {}
        }
        thread::sleep(POLL_INTERVAL);
    }

    Ok(AndroidBootVerification {
        verified: false,
        serial: serial.to_string(),
        product: None,
        android_release: None,
        build_fingerprint: None,
        boot_completed: false,
        elapsed_ms: started.elapsed().as_millis() as u64,
        diagnostic: format!(
            "Timed out waiting for Android boot completion; last ADB state was {}.",
            last_state.as_deref().unwrap_or("not detected")
        ),
    })
}

#[tauri::command]
pub async fn verify_android_boot(
    serial: String,
    expected_product: Option<String>,
    timeout_seconds: Option<u64>,
) -> Result<AndroidBootVerification, String> {
    let timeout = Duration::from_secs(
        timeout_seconds
            .unwrap_or(DEFAULT_BOOT_TIMEOUT.as_secs())
            .clamp(30, 900),
    );
    tauri::async_runtime::spawn_blocking(move || {
        wait_for_android_boot_inner(&serial, expected_product.as_deref(), timeout)
    })
    .await
    .map_err(|error| format!("Android boot verification worker failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::DEFAULT_BOOT_TIMEOUT;

    #[test]
    fn default_timeout_is_long_enough_for_first_boot() {
        assert!(DEFAULT_BOOT_TIMEOUT.as_secs() >= 300);
    }
}
