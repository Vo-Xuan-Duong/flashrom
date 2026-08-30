use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

const PROFILE_FILE_NAME: &str = "flashrom-restore-profile.json";
const MAX_PROFILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PROFILE_APPS: usize = 512;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreProfileConfigApp {
    package_name: String,
    installer_package: Option<String>,
    source_kind: String,
    restore_strategy: String,
    enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreProfileConfig {
    version: u8,
    device_product: Option<String>,
    android_release: Option<String>,
    sdk_level: Option<String>,
    apps: Vec<RestoreProfileConfigApp>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreProfileSaveResult {
    path: String,
    app_count: usize,
    diagnostic: String,
}

fn safe_package_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

fn safe_optional_package(value: Option<&str>) -> bool {
    value.map(safe_package_name).unwrap_or(true)
}

fn valid_strategy(value: &str) -> bool {
    matches!(
        value,
        "google_play" | "source_manager" | "local_apk_backup" | "manual" | "skip"
    )
}

fn validate_profile(profile: &RestoreProfileConfig) -> Result<(), String> {
    if profile.version != 1 {
        return Err(format!(
            "Unsupported restore profile version {}. Expected version 1.",
            profile.version
        ));
    }
    if profile.apps.len() > MAX_PROFILE_APPS {
        return Err(format!(
            "Restore profile contains {} apps; maximum supported is {MAX_PROFILE_APPS}.",
            profile.apps.len()
        ));
    }

    for app in &profile.apps {
        if !safe_package_name(&app.package_name) {
            return Err(format!("Unsafe package name in restore profile: {}", app.package_name));
        }
        if !safe_optional_package(app.installer_package.as_deref()) {
            return Err(format!(
                "Unsafe installer package in restore profile for {}.",
                app.package_name
            ));
        }
        if !valid_strategy(&app.restore_strategy) {
            return Err(format!(
                "Unsupported restore strategy '{}' for {}.",
                app.restore_strategy, app.package_name
            ));
        }
    }
    Ok(())
}

fn profile_path(directory: &str) -> Result<PathBuf, String> {
    let directory = directory.trim();
    if directory.is_empty() {
        return Err("Restore profile directory is required.".into());
    }
    Ok(PathBuf::from(directory).join(PROFILE_FILE_NAME))
}

#[tauri::command]
pub fn save_restore_profile(
    directory: String,
    profile: RestoreProfileConfig,
) -> Result<RestoreProfileSaveResult, String> {
    validate_profile(&profile)?;
    let path = profile_path(&directory)?;
    let parent = path
        .parent()
        .ok_or_else(|| "Restore profile path has no parent directory.".to_string())?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Unable to create restore profile directory {}: {error}",
            parent.display()
        )
    })?;

    let json = serde_json::to_vec_pretty(&profile)
        .map_err(|error| format!("Unable to serialize restore profile: {error}"))?;
    if json.len() as u64 > MAX_PROFILE_BYTES {
        return Err("Restore profile is unexpectedly large and was not saved.".into());
    }
    fs::write(&path, json)
        .map_err(|error| format!("Unable to save restore profile {}: {error}", path.display()))?;

    Ok(RestoreProfileSaveResult {
        path: path.to_string_lossy().to_string(),
        app_count: profile.apps.len(),
        diagnostic: format!(
            "Saved restore profile with {} app(s) to {}.",
            profile.apps.len(),
            path.display()
        ),
    })
}

#[tauri::command]
pub fn load_restore_profile(directory: String) -> Result<RestoreProfileConfig, String> {
    let path = profile_path(&directory)?;
    let metadata = fs::metadata(&path)
        .map_err(|error| format!("Restore profile not found at {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("Restore profile path is not a file: {}", path.display()));
    }
    if metadata.len() > MAX_PROFILE_BYTES {
        return Err("Restore profile exceeds the 2 MiB safety limit.".into());
    }

    let bytes = fs::read(&path)
        .map_err(|error| format!("Unable to read restore profile {}: {error}", path.display()))?;
    let profile: RestoreProfileConfig = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Invalid restore profile JSON: {error}"))?;
    validate_profile(&profile)?;
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::{valid_strategy, validate_profile, RestoreProfileConfig, RestoreProfileConfigApp};

    fn profile(strategy: &str) -> RestoreProfileConfig {
        RestoreProfileConfig {
            version: 1,
            device_product: Some("sunstone".into()),
            android_release: Some("16".into()),
            sdk_level: Some("36".into()),
            apps: vec![RestoreProfileConfigApp {
                package_name: "com.example.app".into(),
                installer_package: Some("com.android.vending".into()),
                source_kind: "google_play".into(),
                restore_strategy: strategy.into(),
                enabled: true,
            }],
        }
    }

    #[test]
    fn accepts_known_restore_strategies() {
        assert!(valid_strategy("google_play"));
        assert!(valid_strategy("source_manager"));
        assert!(valid_strategy("local_apk_backup"));
        assert!(!valid_strategy("shell_command"));
    }

    #[test]
    fn validates_profile_entries() {
        assert!(validate_profile(&profile("google_play")).is_ok());
        assert!(validate_profile(&profile("shell_command")).is_err());
    }
}
