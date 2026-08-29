use std::{fs, path::Path};

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RomArtifact {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) kind: String,
    pub(crate) size: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RomInspection {
    pub(crate) path: String,
    pub(crate) kind: String,
    pub(crate) size: u64,
    pub(crate) artifacts: Vec<RomArtifact>,
    pub(crate) diagnostic: String,
}

fn file_kind(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();

    if name == "payload.bin" {
        "payload".into()
    } else if name == "super.img" {
        "super_image".into()
    } else if extension == "img" {
        "image".into()
    } else if extension == "zip" {
        "zip".into()
    } else if name.starts_with("flash_all") || name.starts_with("flash-all") {
        "flash_script".into()
    } else {
        "file".into()
    }
}

fn artifact(path: &Path) -> Option<RomArtifact> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }

    Some(RomArtifact {
        name: path.file_name()?.to_string_lossy().into_owned(),
        path: path.to_string_lossy().into_owned(),
        kind: file_kind(path),
        size: metadata.len(),
    })
}

fn collect_directory(directory: &Path) -> Result<Vec<RomArtifact>, String> {
    let mut artifacts = Vec::new();
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("Unable to read ROM directory: {error}"))?;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_file() {
            if let Some(value) = artifact(&path) {
                artifacts.push(value);
            }
            continue;
        }

        let is_images_directory = path
            .file_name()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("images"))
            .unwrap_or(false);

        if is_images_directory {
            if let Ok(image_entries) = fs::read_dir(&path) {
                for image_entry in image_entries.flatten() {
                    if let Some(value) = artifact(&image_entry.path()) {
                        artifacts.push(value);
                    }
                }
            }
        }
    }

    artifacts.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    Ok(artifacts)
}

fn classify_directory(artifacts: &[RomArtifact]) -> String {
    let has_payload = artifacts.iter().any(|item| item.kind == "payload");
    let has_flash_script = artifacts.iter().any(|item| item.kind == "flash_script");
    let image_count = artifacts
        .iter()
        .filter(|item| item.kind == "image" || item.kind == "super_image")
        .count();

    if has_payload {
        "payload_package".into()
    } else if has_flash_script || image_count >= 2 {
        "fastboot_rom".into()
    } else if image_count == 1 {
        "image_folder".into()
    } else {
        "directory".into()
    }
}

pub(crate) fn inspect_rom_inner(path: &str) -> Result<RomInspection, String> {
    let input = Path::new(path);
    let metadata = fs::metadata(input)
        .map_err(|error| format!("ROM input does not exist or cannot be accessed: {error}"))?;

    if metadata.is_file() {
        let kind = match file_kind(input).as_str() {
            "zip" => "recovery_zip",
            "payload" => "payload_bin",
            "super_image" => "super_image",
            "image" => "image",
            _ => "file",
        }
        .to_string();

        let value = artifact(input)
            .ok_or_else(|| "Unable to inspect the selected ROM file.".to_string())?;

        return Ok(RomInspection {
            path: path.to_string(),
            kind: kind.clone(),
            size: metadata.len(),
            artifacts: vec![value],
            diagnostic: format!("Detected ROM input type: {kind}."),
        });
    }

    if !metadata.is_dir() {
        return Err("ROM input must be a regular file or directory.".into());
    }

    let artifacts = collect_directory(input)?;
    let size = artifacts.iter().map(|item| item.size).sum();
    let kind = classify_directory(&artifacts);
    let image_count = artifacts
        .iter()
        .filter(|item| item.kind == "image" || item.kind == "super_image")
        .count();

    Ok(RomInspection {
        path: path.to_string(),
        kind: kind.clone(),
        size,
        artifacts,
        diagnostic: format!(
            "Detected {kind} with {image_count} image file(s) in the inspected level."
        ),
    })
}

#[tauri::command]
pub fn inspect_rom(path: String) -> Result<RomInspection, String> {
    inspect_rom_inner(&path)
}

#[cfg(test)]
mod tests {
    use super::{classify_directory, RomArtifact};

    fn item(name: &str, kind: &str) -> RomArtifact {
        RomArtifact {
            name: name.into(),
            path: name.into(),
            kind: kind.into(),
            size: 1,
        }
    }

    #[test]
    fn classifies_fastboot_rom_from_multiple_images() {
        let artifacts = vec![item("boot.img", "image"), item("vendor_boot.img", "image")];
        assert_eq!(classify_directory(&artifacts), "fastboot_rom");
    }

    #[test]
    fn payload_takes_precedence() {
        let artifacts = vec![item("payload.bin", "payload"), item("boot.img", "image")];
        assert_eq!(classify_directory(&artifacts), "payload_package");
    }
}
