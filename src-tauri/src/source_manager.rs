use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::process::{run, run_streaming, AndroidTool};

const MANIFEST_NAME: &str = "source-manager-manifest.json";
const MAX_CONFIG_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
struct ManagerSpec {
    id: &'static str,
    package_name: &'static str,
    label: &'static str,
    import_hint: &'static str,
}

const MANAGERS: &[ManagerSpec] = &[
    ManagerSpec {
        id: "obtainium",
        package_name: "dev.imranr.obtainium",
        label: "Obtainium",
        import_hint: "Open Obtainium and use its Import/Restore function with the staged file from Download/FlashROM/obtainium.",
    },
    ManagerSpec {
        id: "fdroid",
        package_name: "org.fdroid.fdroid",
        label: "F-Droid",
        import_hint: "Open F-Droid and import/restore the staged repository or backup file from Download/FlashROM/fdroid when supported by the installed F-Droid build.",
    },
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceManagerConfigEntry {
    manager: String,
    package_name: String,
    label: String,
    file_name: String,
    local_path: String,
    size: u64,
    sha256: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceManagerManifest {
    version: u8,
    entries: Vec<SourceManagerConfigEntry>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceManagerBackupResult {
    manifest_path: String,
    entry: SourceManagerConfigEntry,
    diagnostic: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceManagerStageResult {
    manager: String,
    package_name: String,
    manager_installed: bool,
    remote_path: String,
    sha256: String,
    manual_import_required: bool,
    import_hint: String,
    diagnostic: String,
}

fn manager_spec(id: &str) -> Option<ManagerSpec> {
    MANAGERS
        .iter()
        .copied()
        .find(|manager| manager.id.eq_ignore_ascii_case(id.trim()))
}

fn require_workspace(path: &str) -> Result<PathBuf, String> {
    let workspace = PathBuf::from(path.trim());
    if workspace.as_os_str().is_empty() {
        return Err("A restore workspace directory is required.".into());
    }
    fs::create_dir_all(&workspace)
        .map_err(|error| format!("Unable to create restore workspace: {error}"))?;
    fs::canonicalize(&workspace)
        .map_err(|error| format!("Unable to resolve restore workspace: {error}"))
}

fn manifest_path(workspace: &Path) -> PathBuf {
    workspace.join(MANIFEST_NAME)
}

fn read_manifest(workspace: &Path) -> Result<SourceManagerManifest, String> {
    let path = manifest_path(workspace);
    if !path.is_file() {
        return Ok(SourceManagerManifest {
            version: 1,
            entries: Vec::new(),
        });
    }
    let bytes = fs::read(&path)
        .map_err(|error| format!("Unable to read source-manager manifest: {error}"))?;
    if bytes.len() > 1024 * 1024 {
        return Err("Source-manager manifest exceeds the 1 MiB safety limit.".into());
    }
    let mut manifest: SourceManagerManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Unable to parse source-manager manifest: {error}"))?;
    if manifest.version == 0 {
        manifest.version = 1;
    }
    Ok(manifest)
}

fn write_manifest(workspace: &Path, manifest: &SourceManagerManifest) -> Result<(), String> {
    let path = manifest_path(workspace);
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|error| format!("Unable to serialize source-manager manifest: {error}"))?;
    fs::write(&path, bytes)
        .map_err(|error| format!("Unable to write source-manager manifest: {error}"))
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("Unable to open source-manager config for hashing: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("Unable to hash source-manager config: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn safe_extension(path: &Path) -> Option<String> {
    let value = path.extension()?.to_str()?.to_ascii_lowercase();
    (!value.is_empty()
        && value.len() <= 12
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric()))
    .then_some(value)
}

fn stored_file_name(source: &Path) -> String {
    safe_extension(source)
        .map(|extension| format!("config.{extension}"))
        .unwrap_or_else(|| "config.bin".into())
}

fn require_adb_device(serial: &str) -> Result<(), String> {
    if serial.trim().is_empty() {
        return Err("A selected ADB serial is required.".into());
    }
    let output = run(AndroidTool::Adb, &["devices"])?;
    let state = output.stdout.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let found_serial = parts.next()?;
        let state = parts.next()?;
        (found_serial == serial).then_some(state)
    });
    match state {
        Some("device") => Ok(()),
        Some(other) => Err(format!(
            "ADB device {serial} is in state {other}; source-manager staging requires normal Android state 'device'."
        )),
        None => Err(format!("ADB device {serial} is not connected.")),
    }
}

fn package_is_installed(serial: &str, package: &str) -> bool {
    run(
        AndroidTool::Adb,
        &["-s", serial, "shell", "pm", "path", package],
    )
    .map(|output| output.success() && output.stdout.contains("package:"))
    .unwrap_or(false)
}

#[tauri::command]
pub fn inspect_source_manager_vault(workspace: String) -> Result<SourceManagerManifest, String> {
    let workspace = require_workspace(&workspace)?;
    read_manifest(&workspace)
}

#[tauri::command]
pub fn backup_source_manager_config(
    workspace: String,
    manager: String,
    source_path: String,
) -> Result<SourceManagerBackupResult, String> {
    let workspace = require_workspace(&workspace)?;
    let spec = manager_spec(&manager)
        .ok_or_else(|| "Source manager must be obtainium or fdroid.".to_string())?;
    let source = PathBuf::from(source_path.trim());
    if !source.is_file() {
        return Err("Selected source-manager export does not exist or is not a file.".into());
    }
    let metadata = fs::metadata(&source)
        .map_err(|error| format!("Unable to inspect source-manager export: {error}"))?;
    if metadata.len() == 0 || metadata.len() > MAX_CONFIG_BYTES {
        return Err("Source-manager export must be between 1 byte and 64 MiB.".into());
    }

    let directory = workspace.join("source-manager-vault").join(spec.id);
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Unable to create source-manager vault directory: {error}"))?;
    let file_name = stored_file_name(&source);
    let destination = directory.join(&file_name);
    fs::copy(&source, &destination)
        .map_err(|error| format!("Unable to copy source-manager export into vault: {error}"))?;
    let sha256 = sha256_file(&destination)?;

    let entry = SourceManagerConfigEntry {
        manager: spec.id.into(),
        package_name: spec.package_name.into(),
        label: spec.label.into(),
        file_name,
        local_path: destination.to_string_lossy().to_string(),
        size: metadata.len(),
        sha256,
    };
    let mut manifest = read_manifest(&workspace)?;
    manifest.version = 1;
    manifest
        .entries
        .retain(|value| !value.manager.eq_ignore_ascii_case(spec.id));
    manifest.entries.push(entry.clone());
    manifest
        .entries
        .sort_by(|left, right| left.manager.cmp(&right.manager));
    write_manifest(&workspace, &manifest)?;

    Ok(SourceManagerBackupResult {
        manifest_path: manifest_path(&workspace).to_string_lossy().to_string(),
        entry,
        diagnostic: format!(
            "{} configuration export was copied into the restore vault and SHA-256 pinned.",
            spec.label
        ),
    })
}

#[tauri::command]
pub async fn stage_source_manager_config(
    app: tauri::AppHandle,
    serial: String,
    workspace: String,
    manager: String,
    confirmation: String,
) -> Result<SourceManagerStageResult, String> {
    if confirmation != "STAGE CONFIG" {
        return Err("Source-manager staging requires the exact confirmation STAGE CONFIG.".into());
    }

    tauri::async_runtime::spawn_blocking(move || {
        require_adb_device(&serial)?;
        let workspace = require_workspace(&workspace)?;
        let spec = manager_spec(&manager)
            .ok_or_else(|| "Source manager must be obtainium or fdroid.".to_string())?;
        let manifest = read_manifest(&workspace)?;
        let entry = manifest
            .entries
            .iter()
            .find(|entry| entry.manager.eq_ignore_ascii_case(spec.id))
            .cloned()
            .ok_or_else(|| format!("No {} configuration is stored in this vault.", spec.label))?;

        let local = fs::canonicalize(&entry.local_path)
            .map_err(|error| format!("Unable to resolve stored source-manager config: {error}"))?;
        if !local.starts_with(&workspace) || !local.is_file() {
            return Err("Stored source-manager config escaped the restore workspace or no longer exists.".into());
        }
        let current_hash = sha256_file(&local)?;
        if current_hash != entry.sha256 {
            return Err("Stored source-manager config SHA-256 changed after backup; staging is blocked.".into());
        }

        let remote_directory = format!("/sdcard/Download/FlashROM/{}", spec.id);
        let remote_path = format!("{remote_directory}/{}", entry.file_name);
        let mkdir = run(
            AndroidTool::Adb,
            &[
                "-s",
                &serial,
                "shell",
                "mkdir",
                "-p",
                &remote_directory,
            ],
        )?;
        if !mkdir.success() {
            return Err(format!(
                "Unable to create device staging directory: {}",
                mkdir.combined_output()
            ));
        }

        let local_text = local.to_string_lossy().to_string();
        let push = run_streaming(
            &app,
            &format!("stage-source-manager-{}", spec.id),
            AndroidTool::Adb,
            &["-s", &serial, "push", &local_text, &remote_path],
        )?;
        if !push.success() {
            return Err(format!(
                "Unable to stage {} configuration: {}",
                spec.label,
                push.combined_output()
            ));
        }

        let manager_installed = package_is_installed(&serial, spec.package_name);
        Ok(SourceManagerStageResult {
            manager: spec.id.into(),
            package_name: spec.package_name.into(),
            manager_installed,
            remote_path: remote_path.clone(),
            sha256: entry.sha256,
            manual_import_required: true,
            import_hint: spec.import_hint.into(),
            diagnostic: format!(
                "{} configuration was SHA-256 verified and staged at {remote_path}. {}",
                spec.label,
                if manager_installed {
                    "The manager package is installed; complete the in-app import now."
                } else {
                    "The manager package is not installed yet; restore its APK first, then complete the in-app import."
                }
            ),
        })
    })
    .await
    .map_err(|error| format!("Source-manager staging worker failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::{manager_spec, stored_file_name};
    use std::path::Path;

    #[test]
    fn resolves_supported_managers() {
        assert_eq!(
            manager_spec("obtainium").map(|value| value.package_name),
            Some("dev.imranr.obtainium")
        );
        assert!(manager_spec("unknown").is_none());
    }

    #[test]
    fn normalizes_vault_file_names() {
        assert_eq!(stored_file_name(Path::new("backup.JSON")), "config.json");
        assert_eq!(
            stored_file_name(Path::new("backup.weird-ext!")),
            "config.bin"
        );
    }
}
