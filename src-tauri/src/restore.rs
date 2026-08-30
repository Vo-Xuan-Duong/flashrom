use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::AppHandle;

use crate::process::{run, run_streaming, AndroidTool};

const MAX_PROFILE_APPS: usize = 512;
const MAX_BACKUP_PACKAGES: usize = 256;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreApp {
    package_name: String,
    installer_package: Option<String>,
    source_kind: String,
    restore_strategy: String,
    enabled_by_default: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreProfileCounts {
    total: usize,
    google_play: usize,
    source_manager: usize,
    local_apk_backup: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreProfile {
    version: u8,
    serial: String,
    device_product: Option<String>,
    android_release: Option<String>,
    sdk_level: Option<String>,
    apps: Vec<RestoreApp>,
    counts: RestoreProfileCounts,
    diagnostic: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApkBackupFile {
    remote_path: String,
    local_path: String,
    size: u64,
    sha256: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApkBackupPackageResult {
    package_name: String,
    success: bool,
    files: Vec<ApkBackupFile>,
    diagnostic: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApkBackupReport {
    destination: String,
    packages: Vec<ApkBackupPackageResult>,
    success_count: usize,
    failure_count: usize,
    total_files: usize,
    diagnostic: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalRestorePackageResult {
    package_name: String,
    success: bool,
    apk_count: usize,
    command: Option<String>,
    diagnostic: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalRestoreReport {
    source_directory: String,
    packages: Vec<LocalRestorePackageResult>,
    success_count: usize,
    failure_count: usize,
    diagnostic: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreVerification {
    expected_count: usize,
    installed_count: usize,
    missing_count: usize,
    installed: Vec<String>,
    missing: Vec<String>,
    diagnostic: String,
}

fn safe_package_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

fn require_package_list(packages: &[String]) -> Result<(), String> {
    if packages.len() > MAX_BACKUP_PACKAGES {
        return Err(format!(
            "A restore operation is limited to {MAX_BACKUP_PACKAGES} packages per request."
        ));
    }

    for package in packages {
        if !safe_package_name(package) {
            return Err(format!("Unsafe Android package name: {package}"));
        }
    }
    Ok(())
}

fn require_adb_device(serial: &str) -> Result<(), String> {
    if serial.trim().is_empty() {
        return Err("A detected ADB serial is required.".into());
    }

    let output = run(AndroidTool::Adb, &["devices"])?;
    if !output.success() {
        return Err(format!(
            "Unable to query ADB devices: {}",
            output.combined_output()
        ));
    }

    let state = output.stdout.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let found_serial = parts.next()?;
        let state = parts.next()?;
        (found_serial == serial).then_some(state)
    });

    match state {
        Some("device") => Ok(()),
        Some(other) => Err(format!(
            "ADB device {serial} is in state {other}. Restore operations require normal Android ADB state 'device'."
        )),
        None => Err(format!("ADB device {serial} is not currently connected.")),
    }
}

fn adb_prop(serial: &str, property: &str) -> Option<String> {
    run(
        AndroidTool::Adb,
        &["-s", serial, "shell", "getprop", property],
    )
    .ok()
    .filter(|output| output.success())
    .and_then(|output| {
        let value = output.stdout.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn normalize_installer(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty()
        || value.eq_ignore_ascii_case("null")
        || value.eq_ignore_ascii_case("none")
        || value.eq_ignore_ascii_case("unknown")
    {
        None
    } else if safe_package_name(value) {
        Some(value.to_string())
    } else {
        None
    }
}

fn classify_installer(installer: Option<&str>) -> (&'static str, &'static str) {
    match installer {
        Some("com.android.vending") => ("google_play", "google_play"),
        Some("dev.imranr.obtainium") => ("obtainium", "source_manager"),
        Some("org.fdroid.fdroid") | Some("org.fdroid.basic") => ("fdroid", "source_manager"),
        Some("com.aurora.store") => ("aurora_store", "source_manager"),
        Some("com.amazon.venezia") => ("amazon_appstore", "source_manager"),
        Some("com.sec.android.app.samsungapps") => ("galaxy_store", "source_manager"),
        Some("com.huawei.appmarket") => ("huawei_appgallery", "source_manager"),
        Some("com.android.packageinstaller")
        | Some("com.google.android.packageinstaller")
        | Some("com.android.shell") => ("local_or_adb", "local_apk_backup"),
        Some(_) => ("external_or_unknown", "local_apk_backup"),
        None => ("unknown", "local_apk_backup"),
    }
}

fn parse_package_with_installer(line: &str) -> Option<RestoreApp> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("package:")?;
    let mut parts = rest.split_whitespace();
    let package_name = parts.next()?.trim();
    if !safe_package_name(package_name) {
        return None;
    }

    let installer = parts.find_map(|part| part.strip_prefix("installer="));
    let installer_package = normalize_installer(installer);
    let (source_kind, restore_strategy) = classify_installer(installer_package.as_deref());

    Some(RestoreApp {
        package_name: package_name.to_string(),
        installer_package,
        source_kind: source_kind.into(),
        restore_strategy: restore_strategy.into(),
        enabled_by_default: true,
    })
}

fn parse_package_name(line: &str) -> Option<String> {
    let package = line.trim().strip_prefix("package:")?.trim();
    safe_package_name(package).then(|| package.to_string())
}

fn scan_packages(serial: &str) -> Result<Vec<RestoreApp>, String> {
    let output = run(
        AndroidTool::Adb,
        &["-s", serial, "shell", "pm", "list", "packages", "-3", "-i"],
    )?;

    let mut apps = if output.success() {
        output
            .stdout
            .lines()
            .filter_map(parse_package_with_installer)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    if apps.is_empty() {
        let fallback = run(
            AndroidTool::Adb,
            &["-s", serial, "shell", "pm", "list", "packages", "-3"],
        )?;
        if !fallback.success() {
            return Err(format!(
                "Unable to list installed applications: {}",
                fallback.combined_output()
            ));
        }

        apps = fallback
            .stdout
            .lines()
            .filter_map(parse_package_name)
            .map(|package_name| RestoreApp {
                package_name,
                installer_package: None,
                source_kind: "unknown".into(),
                restore_strategy: "local_apk_backup".into(),
                enabled_by_default: true,
            })
            .collect();
    }

    apps.sort_by(|left, right| left.package_name.cmp(&right.package_name));
    apps.dedup_by(|left, right| left.package_name == right.package_name);
    if apps.len() > MAX_PROFILE_APPS {
        apps.truncate(MAX_PROFILE_APPS);
    }
    Ok(apps)
}

fn profile_counts(apps: &[RestoreApp]) -> RestoreProfileCounts {
    RestoreProfileCounts {
        total: apps.len(),
        google_play: apps
            .iter()
            .filter(|app| app.restore_strategy == "google_play")
            .count(),
        source_manager: apps
            .iter()
            .filter(|app| app.restore_strategy == "source_manager")
            .count(),
        local_apk_backup: apps
            .iter()
            .filter(|app| app.restore_strategy == "local_apk_backup")
            .count(),
    }
}

#[tauri::command]
pub fn scan_restore_profile(serial: String) -> Result<RestoreProfile, String> {
    require_adb_device(&serial)?;
    let apps = scan_packages(&serial)?;
    let counts = profile_counts(&apps);

    Ok(RestoreProfile {
        version: 1,
        serial: serial.clone(),
        device_product: adb_prop(&serial, "ro.product.device"),
        android_release: adb_prop(&serial, "ro.build.version.release"),
        sdk_level: adb_prop(&serial, "ro.build.version.sdk"),
        diagnostic: format!(
            "Scanned {} third-party app(s): {} Google Play, {} source-manager, {} local APK backup candidate(s).",
            counts.total, counts.google_play, counts.source_manager, counts.local_apk_backup
        ),
        apps,
        counts,
    })
}

fn package_apk_paths(serial: &str, package: &str) -> Result<Vec<String>, String> {
    let output = run(
        AndroidTool::Adb,
        &["-s", serial, "shell", "pm", "path", package],
    )?;
    if !output.success() {
        return Err(format!(
            "Unable to read APK paths for {package}: {}",
            output.combined_output()
        ));
    }

    let mut paths = output
        .stdout
        .lines()
        .filter_map(|line| line.trim().strip_prefix("package:"))
        .map(str::trim)
        .filter(|path| path.starts_with('/') && !path.contains(['\r', '\n']))
        .map(str::to_string)
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();

    if paths.is_empty() {
        Err(format!("Package {package} did not expose any APK paths."))
    } else {
        Ok(paths)
    }
}

fn sha256_local(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("Unable to hash backup file {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn backup_package(
    app: &AppHandle,
    serial: &str,
    destination: &Path,
    package: &str,
    package_index: usize,
) -> ApkBackupPackageResult {
    let remote_paths = match package_apk_paths(serial, package) {
        Ok(paths) => paths,
        Err(error) => {
            return ApkBackupPackageResult {
                package_name: package.into(),
                success: false,
                files: Vec::new(),
                diagnostic: error,
            }
        }
    };

    let package_dir = destination.join(package);
    if let Err(error) = fs::create_dir_all(&package_dir) {
        return ApkBackupPackageResult {
            package_name: package.into(),
            success: false,
            files: Vec::new(),
            diagnostic: format!("Unable to create {}: {error}", package_dir.display()),
        };
    }

    let mut files = Vec::new();
    for (file_index, remote_path) in remote_paths.iter().enumerate() {
        let name = Path::new(remote_path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("package.apk");
        let local_path = package_dir.join(name);
        let local_text = local_path.to_string_lossy().to_string();
        let operation_id = format!("restore-backup-{package_index}-{file_index}");
        let output = match run_streaming(
            app,
            &operation_id,
            AndroidTool::Adb,
            &["-s", serial, "pull", remote_path, &local_text],
        ) {
            Ok(output) => output,
            Err(error) => {
                return ApkBackupPackageResult {
                    package_name: package.into(),
                    success: false,
                    files,
                    diagnostic: format!("ADB pull failed for {remote_path}: {error}"),
                }
            }
        };

        if !output.success() || !local_path.is_file() {
            return ApkBackupPackageResult {
                package_name: package.into(),
                success: false,
                files,
                diagnostic: format!(
                    "ADB pull failed for {remote_path}: {}",
                    output.combined_output()
                ),
            };
        }

        let size = fs::metadata(&local_path)
            .map(|value| value.len())
            .unwrap_or(0);
        let sha256 = match sha256_local(&local_path) {
            Ok(value) => value,
            Err(error) => {
                return ApkBackupPackageResult {
                    package_name: package.into(),
                    success: false,
                    files,
                    diagnostic: error,
                }
            }
        };

        files.push(ApkBackupFile {
            remote_path: remote_path.clone(),
            local_path: local_text,
            size,
            sha256,
        });
    }

    ApkBackupPackageResult {
        package_name: package.into(),
        success: true,
        diagnostic: format!("Backed up {} APK file(s).", files.len()),
        files,
    }
}

#[tauri::command]
pub async fn backup_restore_apks(
    app: AppHandle,
    serial: String,
    destination: String,
    packages: Vec<String>,
) -> Result<ApkBackupReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        require_adb_device(&serial)?;
        require_package_list(&packages)?;
        if packages.is_empty() {
            return Err("Select at least one package for APK backup.".into());
        }

        let destination = PathBuf::from(destination.trim());
        if destination.as_os_str().is_empty() {
            return Err("APK backup destination is required.".into());
        }
        fs::create_dir_all(&destination).map_err(|error| {
            format!(
                "Unable to create APK backup destination {}: {error}",
                destination.display()
            )
        })?;

        let mut results = Vec::with_capacity(packages.len());
        for (index, package) in packages.iter().enumerate() {
            results.push(backup_package(
                &app,
                &serial,
                &destination,
                package,
                index,
            ));
        }

        let success_count = results.iter().filter(|item| item.success).count();
        let failure_count = results.len().saturating_sub(success_count);
        let total_files = results.iter().map(|item| item.files.len()).sum();

        Ok(ApkBackupReport {
            destination: destination.to_string_lossy().to_string(),
            packages: results,
            success_count,
            failure_count,
            total_files,
            diagnostic: format!(
                "APK backup complete: {success_count} package(s) succeeded, {failure_count} failed, {total_files} APK file(s) saved."
            ),
        })
    })
    .await
    .map_err(|error| format!("APK backup worker failed: {error}"))?
}

fn local_apk_files(source: &Path, package: &str) -> Result<Vec<PathBuf>, String> {
    let directory = source.join(package);
    let entries = fs::read_dir(&directory)
        .map_err(|error| format!("Unable to read {}: {error}", directory.display()))?;
    let mut apks = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(|value| value.eq_ignore_ascii_case("apk"))
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    apks.sort_by(|left, right| {
        let left_base = left.file_name().and_then(|value| value.to_str()) == Some("base.apk");
        let right_base = right.file_name().and_then(|value| value.to_str()) == Some("base.apk");
        right_base
            .cmp(&left_base)
            .then(left.file_name().cmp(&right.file_name()))
    });
    if apks.is_empty() {
        Err(format!("No APK files found for {package}."))
    } else {
        Ok(apks)
    }
}

fn package_is_installed(serial: &str, package: &str) -> bool {
    run(
        AndroidTool::Adb,
        &["-s", serial, "shell", "pm", "path", package],
    )
    .map(|output| {
        output.success()
            && output
                .stdout
                .lines()
                .any(|line| line.starts_with("package:"))
    })
    .unwrap_or(false)
}

#[tauri::command]
pub async fn restore_local_apks(
    app: AppHandle,
    serial: String,
    source_directory: String,
    packages: Vec<String>,
) -> Result<LocalRestoreReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        require_adb_device(&serial)?;
        require_package_list(&packages)?;
        if packages.is_empty() {
            return Err("Select at least one package for local APK restore.".into());
        }

        let source = PathBuf::from(source_directory.trim());
        if !source.is_dir() {
            return Err(format!(
                "Local APK backup directory does not exist: {}",
                source.display()
            ));
        }

        let mut results = Vec::with_capacity(packages.len());
        for (index, package) in packages.iter().enumerate() {
            let apks = match local_apk_files(&source, package) {
                Ok(value) => value,
                Err(error) => {
                    results.push(LocalRestorePackageResult {
                        package_name: package.clone(),
                        success: false,
                        apk_count: 0,
                        command: None,
                        diagnostic: error,
                    });
                    continue;
                }
            };

            let apk_strings = apks
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>();
            let mut args = vec!["-s".to_string(), serial.clone()];
            if apk_strings.len() == 1 {
                args.extend(["install".into(), "-r".into(), apk_strings[0].clone()]);
            } else {
                args.extend(["install-multiple".into(), "-r".into()]);
                args.extend(apk_strings.clone());
            }
            let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
            let operation_id = format!("restore-install-{index}");

            match run_streaming(&app, &operation_id, AndroidTool::Adb, &refs) {
                Ok(output) => {
                    let verified = output.success() && package_is_installed(&serial, package);
                    let diagnostic = if verified {
                        "APK install completed and package presence was verified.".into()
                    } else {
                        format!(
                            "APK install did not verify successfully: {}",
                            output.combined_output()
                        )
                    };
                    results.push(LocalRestorePackageResult {
                        package_name: package.clone(),
                        success: verified,
                        apk_count: apk_strings.len(),
                        command: Some(output.command),
                        diagnostic,
                    });
                }
                Err(error) => results.push(LocalRestorePackageResult {
                    package_name: package.clone(),
                    success: false,
                    apk_count: apk_strings.len(),
                    command: None,
                    diagnostic: error,
                }),
            }
        }

        let success_count = results.iter().filter(|item| item.success).count();
        let failure_count = results.len().saturating_sub(success_count);
        Ok(LocalRestoreReport {
            source_directory: source.to_string_lossy().to_string(),
            packages: results,
            success_count,
            failure_count,
            diagnostic: format!(
                "Local APK restore finished: {success_count} package(s) verified, {failure_count} failed."
            ),
        })
    })
    .await
    .map_err(|error| format!("Local APK restore worker failed: {error}"))?
}

#[tauri::command]
pub fn verify_restore_packages(
    serial: String,
    expected_packages: Vec<String>,
) -> Result<RestoreVerification, String> {
    require_adb_device(&serial)?;
    require_package_list(&expected_packages)?;

    let output = run(
        AndroidTool::Adb,
        &["-s", &serial, "shell", "pm", "list", "packages"],
    )?;
    if !output.success() {
        return Err(format!(
            "Unable to verify installed packages: {}",
            output.combined_output()
        ));
    }

    let installed_set = output
        .stdout
        .lines()
        .filter_map(parse_package_name)
        .collect::<BTreeSet<_>>();
    let expected = expected_packages.into_iter().collect::<BTreeSet<_>>();
    let installed = expected
        .iter()
        .filter(|package| installed_set.contains(*package))
        .cloned()
        .collect::<Vec<_>>();
    let missing = expected
        .iter()
        .filter(|package| !installed_set.contains(*package))
        .cloned()
        .collect::<Vec<_>>();

    Ok(RestoreVerification {
        expected_count: expected.len(),
        installed_count: installed.len(),
        missing_count: missing.len(),
        diagnostic: if missing.is_empty() {
            format!("All {} expected package(s) are installed.", expected.len())
        } else {
            format!(
                "{} of {} expected package(s) are still missing.",
                missing.len(),
                expected.len()
            )
        },
        installed,
        missing,
    })
}

#[cfg(test)]
mod tests {
    use super::{classify_installer, parse_package_with_installer, safe_package_name};

    #[test]
    fn classifies_known_installers() {
        assert_eq!(
            classify_installer(Some("com.android.vending")),
            ("google_play", "google_play")
        );
        assert_eq!(
            classify_installer(Some("dev.imranr.obtainium")),
            ("obtainium", "source_manager")
        );
        assert_eq!(
            classify_installer(Some("com.android.shell")),
            ("local_or_adb", "local_apk_backup")
        );
    }

    #[test]
    fn parses_package_installer_output() {
        let app =
            parse_package_with_installer("package:com.example.app installer=com.android.vending")
                .expect("package should parse");
        assert_eq!(app.package_name, "com.example.app");
        assert_eq!(app.source_kind, "google_play");
        assert_eq!(app.restore_strategy, "google_play");
    }

    #[test]
    fn validates_package_names() {
        assert!(safe_package_name("com.example.app"));
        assert!(!safe_package_name("com.example.app;rm"));
        assert!(!safe_package_name(""));
    }
}
