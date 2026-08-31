use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

use serde::Serialize;
use zip::ZipArchive;

const MAX_ZIP_ENTRIES: usize = 8192;
const MAX_EXTRACTED_FILES: usize = 128;
const MAX_EXTRACTED_BYTES: u64 = 64 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZipEntryInfo {
    name: String,
    size: u64,
    compressed_size: u64,
    kind: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZipInspection {
    path: String,
    kind: String,
    entry_count: usize,
    decompressed_size: Option<u64>,
    has_payload: bool,
    has_recovery_metadata: bool,
    has_fastboot_images: bool,
    entries: Vec<ZipEntryInfo>,
    diagnostic: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZipExtractionResult {
    source: String,
    destination: String,
    extracted_files: Vec<String>,
    extracted_bytes: u64,
    payload_extracted: bool,
    image_count: usize,
    diagnostic: String,
}

fn ensure_zip(path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path);
    if !path.is_file() {
        return Err("Selected ZIP does not exist or is not a regular file.".into());
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("zip"))
        != Some(true)
    {
        return Err("ROM archive inspection requires a .zip file.".into());
    }
    Ok(path)
}

fn entry_kind(name: &str) -> &'static str {
    let normalized = name.replace('\\', "/").to_lowercase();
    let file_name = normalized.rsplit('/').next().unwrap_or_default();
    if file_name == "payload.bin" {
        "payload"
    } else if file_name.ends_with(".img") {
        "image"
    } else if file_name == "metadata" || file_name == "android-info.txt" {
        "metadata"
    } else if file_name.starts_with("flash_all") || file_name.starts_with("flash-all") {
        "flash_script"
    } else if normalized.starts_with("meta-inf/") {
        "recovery_metadata"
    } else {
        "file"
    }
}

fn should_extract(name: &str) -> bool {
    let normalized = name.replace('\\', "/").to_lowercase();
    let file_name = normalized.rsplit('/').next().unwrap_or_default();
    file_name == "payload.bin"
        || file_name == "metadata"
        || file_name == "android-info.txt"
        || file_name.ends_with(".img")
        || normalized == "meta-inf/com/android/metadata"
}

fn classify(entries: &[ZipEntryInfo]) -> String {
    let has_payload = entries.iter().any(|entry| entry.kind == "payload");
    let has_recovery = entries
        .iter()
        .any(|entry| entry.kind == "recovery_metadata");
    let image_count = entries.iter().filter(|entry| entry.kind == "image").count();
    let has_flash_script = entries.iter().any(|entry| entry.kind == "flash_script");

    if has_payload {
        "payload_ota_zip".into()
    } else if image_count >= 2 || has_flash_script {
        "fastboot_zip".into()
    } else if has_recovery {
        "recovery_zip".into()
    } else {
        "unknown_zip".into()
    }
}

fn inspect_zip_inner(path: &str) -> Result<ZipInspection, String> {
    let path = ensure_zip(path)?;
    let file = File::open(&path)
        .map_err(|error| format!("Unable to open ZIP {}: {error}", path.display()))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| format!("Unable to parse ZIP central directory: {error}"))?;

    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(format!(
            "ZIP contains {} entries; FlashROM inspection limit is {MAX_ZIP_ENTRIES}.",
            archive.len()
        ));
    }

    let decompressed_size = archive
        .decompressed_size()
        .and_then(|value| u64::try_from(value).ok());
    let mut entries = Vec::with_capacity(archive.len().min(512));

    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|error| format!("Unable to inspect ZIP entry {index}: {error}"))?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().to_string();
        entries.push(ZipEntryInfo {
            kind: entry_kind(&name).into(),
            name,
            size: file.size(),
            compressed_size: file.compressed_size(),
        });
    }

    let kind = classify(&entries);
    let has_payload = entries.iter().any(|entry| entry.kind == "payload");
    let has_recovery_metadata = entries
        .iter()
        .any(|entry| entry.kind == "recovery_metadata");
    let has_fastboot_images = entries.iter().filter(|entry| entry.kind == "image").count() >= 2;
    let entry_count = entries.len();
    let visible_entries = entries
        .into_iter()
        .filter(|entry| entry.kind != "file")
        .take(256)
        .collect::<Vec<_>>();

    Ok(ZipInspection {
        path: path.to_string_lossy().to_string(),
        kind: kind.clone(),
        entry_count,
        decompressed_size,
        has_payload,
        has_recovery_metadata,
        has_fastboot_images,
        entries: visible_entries,
        diagnostic: format!(
            "ZIP classified as {kind}. Only metadata, payload.bin and image entries are surfaced; no partition write is performed during inspection."
        ),
    })
}

#[tauri::command]
pub fn inspect_rom_zip(path: String) -> Result<ZipInspection, String> {
    inspect_zip_inner(&path)
}

#[tauri::command]
pub fn extract_rom_zip_inputs(
    path: String,
    destination: String,
) -> Result<ZipExtractionResult, String> {
    let source = ensure_zip(&path)?;
    let destination = PathBuf::from(destination);
    if destination.as_os_str().is_empty() {
        return Err("Choose a destination directory for extracted ROM inputs.".into());
    }
    fs::create_dir_all(&destination)
        .map_err(|error| format!("Unable to create extraction directory: {error}"))?;
    let destination_root = fs::canonicalize(&destination)
        .map_err(|error| format!("Unable to resolve extraction directory: {error}"))?;

    let file = File::open(&source)
        .map_err(|error| format!("Unable to open ZIP {}: {error}", source.display()))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| format!("Unable to parse ZIP central directory: {error}"))?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err("ZIP entry count exceeds the extraction safety limit.".into());
    }

    let mut extracted_files = Vec::new();
    let mut extracted_bytes = 0_u64;
    let mut image_count = 0_usize;
    let mut payload_extracted = false;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Unable to read ZIP entry {index}: {error}"))?;
        if entry.is_dir() || entry.is_symlink() || !should_extract(entry.name()) {
            continue;
        }
        if extracted_files.len() >= MAX_EXTRACTED_FILES {
            return Err("ZIP contains too many extractable ROM inputs.".into());
        }
        let safe_relative = entry
            .enclosed_name()
            .ok_or_else(|| format!("Unsafe ZIP entry path rejected: {}", entry.name()))?;
        let output_path = destination_root.join(&safe_relative);
        if !output_path.starts_with(&destination_root) {
            return Err(format!("ZIP entry escaped extraction root: {}", entry.name()));
        }
        extracted_bytes = extracted_bytes
            .checked_add(entry.size())
            .ok_or_else(|| "Extracted size overflow.".to_string())?;
        if extracted_bytes > MAX_EXTRACTED_BYTES {
            return Err("Selected ZIP exceeds FlashROM's 64 GiB safe extraction limit.".into());
        }
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Unable to create extraction directory: {error}"))?;
        }
        let mut output = File::create(&output_path)
            .map_err(|error| format!("Unable to create {}: {error}", output_path.display()))?;
        io::copy(&mut entry, &mut output)
            .map_err(|error| format!("Unable to extract {}: {error}", entry.name()))?;

        let kind = entry_kind(entry.name());
        if kind == "payload" {
            payload_extracted = true;
        }
        if kind == "image" {
            image_count += 1;
        }
        extracted_files.push(output_path.to_string_lossy().to_string());
    }

    Ok(ZipExtractionResult {
        source: source.to_string_lossy().to_string(),
        destination: destination_root.to_string_lossy().to_string(),
        extracted_files,
        extracted_bytes,
        payload_extracted,
        image_count,
        diagnostic: if payload_extracted && image_count == 0 {
            "payload.bin was extracted safely. Partition-image extraction from Android update_engine payloads is still a separate guarded stage and is not executed automatically."
                .into()
        } else {
            format!("Extracted {image_count} image file(s) plus trusted ROM metadata/payload inputs without executing flash commands.")
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{classify, entry_kind, should_extract, ZipEntryInfo};

    fn item(name: &str, kind: &str) -> ZipEntryInfo {
        ZipEntryInfo {
            name: name.into(),
            size: 1,
            compressed_size: 1,
            kind: kind.into(),
        }
    }

    #[test]
    fn detects_payload_zip() {
        assert_eq!(classify(&[item("payload.bin", "payload")]), "payload_ota_zip");
    }

    #[test]
    fn only_extracts_known_rom_inputs() {
        assert!(should_extract("images/boot.img"));
        assert!(should_extract("META-INF/com/android/metadata"));
        assert!(!should_extract("../../evil.exe"));
        assert_eq!(entry_kind("images/system.img"), "image");
    }
}
