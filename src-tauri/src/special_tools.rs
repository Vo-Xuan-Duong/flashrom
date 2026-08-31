use std::{
    env, fs,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use serde::Serialize;
use zip::ZipArchive;

use crate::process::{run_executable, run_executable_streaming};

const MAX_PREPARED_IMAGES: usize = 128;
const MAX_PREPARED_BYTES: u64 = 64 * 1024 * 1024 * 1024;
const MAX_IDENTITY_METADATA_BYTES: u64 = 1024 * 1024;
const SAFE_PARTITIONS: &[&str] = &[
    "boot",
    "init_boot",
    "vendor_boot",
    "vendor_kernel_boot",
    "dtbo",
    "vbmeta",
    "vbmeta_system",
    "vbmeta_vendor",
    "recovery",
    "system",
    "system_ext",
    "product",
    "vendor",
    "odm",
    "system_dlkm",
    "vendor_dlkm",
    "odm_dlkm",
];

#[derive(Clone, Copy, Debug)]
enum SpecialTool {
    PayloadDumper,
    LpUnpack,
    Simg2Img,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecialToolStatus {
    name: String,
    source: String,
    path: String,
    available: bool,
    version: Option<String>,
    diagnostic: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecialToolsStatus {
    payload_dumper: SpecialToolStatus,
    lpunpack: SpecialToolStatus,
    simg2img: SpecialToolStatus,
    payload_ready: bool,
    super_ready: bool,
    diagnostic: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedArtifact {
    name: String,
    path: String,
    size: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedRomInput {
    source: String,
    destination: String,
    kind: String,
    artifacts: Vec<PreparedArtifact>,
    total_bytes: u64,
    ignored_image_count: usize,
    diagnostic: String,
}

fn executable_name(tool: SpecialTool) -> &'static str {
    match (tool, cfg!(windows)) {
        (SpecialTool::PayloadDumper, true) => "payload-dumper-go.exe",
        (SpecialTool::PayloadDumper, false) => "payload-dumper-go",
        (SpecialTool::LpUnpack, true) => "lpunpack.exe",
        (SpecialTool::LpUnpack, false) => "lpunpack",
        (SpecialTool::Simg2Img, true) => "simg2img.exe",
        (SpecialTool::Simg2Img, false) => "simg2img",
    }
}

fn environment_name(tool: SpecialTool) -> &'static str {
    match tool {
        SpecialTool::PayloadDumper => "FLASHROM_PAYLOAD_DUMPER",
        SpecialTool::LpUnpack => "FLASHROM_LPUNPACK",
        SpecialTool::Simg2Img => "FLASHROM_SIMG2IMG",
    }
}

fn local_path(tool: SpecialTool) -> PathBuf {
    match tool {
        SpecialTool::PayloadDumper => PathBuf::from("tools")
            .join("payload-dumper-go")
            .join(executable_name(tool)),
        SpecialTool::LpUnpack => PathBuf::from("tools")
            .join("dynamic-partitions")
            .join(executable_name(tool)),
        SpecialTool::Simg2Img => PathBuf::from("tools")
            .join("dynamic-partitions")
            .join(executable_name(tool)),
    }
}

fn configured_candidate(raw: &str, tool: SpecialTool) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_dir() {
        path.join(executable_name(tool))
    } else {
        path
    }
}

fn resolve_tool(tool: SpecialTool) -> (String, PathBuf) {
    if let Ok(raw) = env::var(environment_name(tool)) {
        let candidate = configured_candidate(&raw, tool);
        if candidate.is_file() {
            return (environment_name(tool).into(), candidate);
        }
    }

    let local = local_path(tool);
    if local.is_file() {
        return ("FlashROM tools directory".into(), local);
    }

    ("system PATH".into(), PathBuf::from(executable_name(tool)))
}

fn first_nonempty_line(value: &str) -> Option<String> {
    value
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

fn probe_tool(tool: SpecialTool) -> SpecialToolStatus {
    let (source, path) = resolve_tool(tool);
    let name = executable_name(tool).trim_end_matches(".exe").to_string();
    let args: &[&str] = match tool {
        SpecialTool::PayloadDumper | SpecialTool::LpUnpack => &["-h"],
        SpecialTool::Simg2Img => &[],
    };

    match run_executable(&path, args) {
        Ok(output) => {
            let text = output.combined_output();
            SpecialToolStatus {
                name: name.clone(),
                source,
                path: path.to_string_lossy().to_string(),
                available: true,
                version: first_nonempty_line(&text),
                diagnostic: format!(
                    "{name} started successfully{}.",
                    if output.success() {
                        String::new()
                    } else {
                        format!(" and returned its usage/status code {}", output.status)
                    }
                ),
            }
        }
        Err(error) => SpecialToolStatus {
            name,
            source,
            path: path.to_string_lossy().to_string(),
            available: false,
            version: None,
            diagnostic: error,
        },
    }
}

#[tauri::command]
pub fn inspect_special_tools() -> Result<SpecialToolsStatus, String> {
    let payload_dumper = probe_tool(SpecialTool::PayloadDumper);
    let lpunpack = probe_tool(SpecialTool::LpUnpack);
    let simg2img = probe_tool(SpecialTool::Simg2Img);
    let payload_ready = payload_dumper.available;
    let super_ready = lpunpack.available;

    Ok(SpecialToolsStatus {
        payload_dumper,
        lpunpack,
        simg2img,
        payload_ready,
        super_ready,
        diagnostic: if payload_ready && super_ready {
            "Payload and dynamic-partition preparation tools are available. Sparse super.img additionally requires simg2img."
                .into()
        } else {
            "Specialized ROM preparation is partially unavailable. Configure FLASHROM_PAYLOAD_DUMPER / FLASHROM_LPUNPACK / FLASHROM_SIMG2IMG or place the executables under FlashROM's tools directory."
                .into()
        },
    })
}

fn require_regular_file(path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path.trim());
    if !path.is_file() {
        return Err(format!(
            "Specialized ROM input does not exist or is not a regular file: {}",
            path.display()
        ));
    }
    Ok(path)
}

fn prepare_empty_workspace(destination: &str) -> Result<PathBuf, String> {
    let destination = PathBuf::from(destination.trim());
    if destination.as_os_str().is_empty() {
        return Err("A preparation workspace directory is required.".into());
    }

    if destination.exists() {
        if !destination.is_dir() {
            return Err("Preparation workspace path exists but is not a directory.".into());
        }
        let mut entries = fs::read_dir(&destination)
            .map_err(|error| format!("Unable to inspect preparation workspace: {error}"))?;
        if entries.next().is_some() {
            return Err(
                "Preparation workspace must be empty so stale partition images cannot enter the flash plan."
                    .into(),
            );
        }
    } else {
        fs::create_dir_all(&destination)
            .map_err(|error| format!("Unable to create preparation workspace: {error}"))?;
    }

    fs::canonicalize(&destination)
        .map_err(|error| format!("Unable to resolve preparation workspace: {error}"))
}

fn safe_partition_name(value: &str) -> bool {
    if SAFE_PARTITIONS.contains(&value) {
        return true;
    }
    for base in SAFE_PARTITIONS {
        if value == format!("{base}_a") || value == format!("{base}_b") {
            return true;
        }
    }
    false
}

fn safe_image_name(path: &Path) -> bool {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(|value| safe_partition_name(&value.to_ascii_lowercase()))
        .unwrap_or(false)
}

fn collect_prepared_images(destination: &Path) -> Result<(Vec<PreparedArtifact>, u64), String> {
    let mut artifacts = Vec::new();
    let mut total_bytes = 0_u64;

    for entry in fs::read_dir(destination)
        .map_err(|error| format!("Unable to inspect prepared ROM images: {error}"))?
    {
        let entry = entry.map_err(|error| format!("Unable to inspect prepared entry: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Unable to inspect prepared entry type: {error}"))?;
        if file_type.is_symlink() || !file_type.is_file() {
            continue;
        }
        let path = entry.path();
        let is_image = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("img"))
            .unwrap_or(false);
        if !is_image || !safe_image_name(&path) {
            continue;
        }

        if artifacts.len() >= MAX_PREPARED_IMAGES {
            return Err(format!(
                "Prepared ROM exceeds the {MAX_PREPARED_IMAGES}-image safety limit."
            ));
        }
        let size = entry
            .metadata()
            .map_err(|error| format!("Unable to read prepared image metadata: {error}"))?
            .len();
        total_bytes = total_bytes
            .checked_add(size)
            .ok_or_else(|| "Prepared ROM image size overflow.".to_string())?;
        if total_bytes > MAX_PREPARED_BYTES {
            return Err("Prepared ROM exceeds FlashROM's 64 GiB image safety limit.".into());
        }

        artifacts.push(PreparedArtifact {
            name: entry.file_name().to_string_lossy().to_string(),
            path: path.to_string_lossy().to_string(),
            size,
        });
    }

    artifacts.sort_by(|left, right| left.name.cmp(&right.name));
    if artifacts.is_empty() {
        return Err(
            "Preparation completed without producing any allowlisted .img partition files.".into(),
        );
    }
    Ok((artifacts, total_bytes))
}

fn quarantine_unsupported_images(destination: &Path) -> Result<usize, String> {
    let entries = fs::read_dir(destination)
        .map_err(|error| format!("Unable to filter prepared images: {error}"))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(|value| value.eq_ignore_ascii_case("img"))
                    .unwrap_or(false)
                && !safe_image_name(path)
        })
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return Ok(0);
    }

    let ignored = destination.join("_ignored_partitions");
    fs::create_dir_all(&ignored)
        .map_err(|error| format!("Unable to create ignored-partition directory: {error}"))?;
    let count = entries.len();
    for source in entries {
        let Some(name) = source.file_name() else {
            continue;
        };
        fs::rename(&source, ignored.join(name)).map_err(|error| {
            format!(
                "Unable to quarantine unsupported prepared image {}: {error}",
                source.display()
            )
        })?;
    }
    Ok(count)
}

fn android_sparse_image(path: &Path) -> Result<bool, String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("Unable to inspect image header {}: {error}", path.display()))?;
    let mut magic = [0_u8; 4];
    let read = file
        .read(&mut magic)
        .map_err(|error| format!("Unable to read image header {}: {error}", path.display()))?;
    Ok(read == 4 && magic == [0x3a, 0xff, 0x26, 0xed])
}

fn copy_metadata_file(source: &Path, destination: &Path) -> Result<bool, String> {
    if !source.is_file() {
        return Ok(false);
    }
    let metadata = fs::metadata(source)
        .map_err(|error| format!("Unable to inspect ROM identity metadata: {error}"))?;
    if metadata.len() > MAX_IDENTITY_METADATA_BYTES {
        return Err(format!(
            "ROM identity metadata {} exceeds the 1 MiB safety limit.",
            source.display()
        ));
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Unable to create metadata directory: {error}"))?;
    }
    fs::copy(source, destination)
        .map_err(|error| format!("Unable to preserve ROM identity metadata: {error}"))?;
    Ok(true)
}

fn preserve_sibling_identity_metadata(source: &Path, destination: &Path) -> Result<usize, String> {
    let Some(root) = source.parent() else {
        return Ok(0);
    };
    let candidates = [
        ("android-info.txt", "android-info.txt"),
        ("metadata", "metadata"),
        (
            "META-INF/com/android/metadata",
            "META-INF/com/android/metadata",
        ),
    ];
    let mut copied = 0;
    for (relative_source, relative_destination) in candidates {
        if copy_metadata_file(
            &root.join(relative_source),
            &destination.join(relative_destination),
        )? {
            copied += 1;
        }
    }
    Ok(copied)
}

fn preserve_zip_identity_metadata(source: &Path, destination: &Path) -> Result<usize, String> {
    let file = File::open(source)
        .map_err(|error| format!("Unable to open OTA ZIP for metadata preservation: {error}"))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| format!("Unable to inspect OTA ZIP metadata: {error}"))?;
    let mut copied = 0;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Unable to inspect OTA ZIP entry: {error}"))?;
        if entry.is_dir() || entry.is_symlink() || entry.size() > MAX_IDENTITY_METADATA_BYTES {
            continue;
        }
        let normalized = entry.name().replace('\\', "/").to_ascii_lowercase();
        let relative = match normalized.as_str() {
            "android-info.txt" => Some("android-info.txt"),
            "metadata" => Some("metadata"),
            "meta-inf/com/android/metadata" => Some("META-INF/com/android/metadata"),
            _ => None,
        };
        let Some(relative) = relative else {
            continue;
        };
        if entry.enclosed_name().is_none() {
            return Err(format!(
                "Unsafe OTA ZIP metadata path rejected: {}",
                entry.name()
            ));
        }
        let output = destination.join(relative);
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Unable to create metadata directory: {error}"))?;
        }
        let mut target = File::create(&output)
            .map_err(|error| format!("Unable to create preserved metadata file: {error}"))?;
        std::io::copy(&mut entry, &mut target)
            .map_err(|error| format!("Unable to preserve OTA ZIP metadata: {error}"))?;
        copied += 1;
    }
    Ok(copied)
}

fn preserve_identity_metadata(source: &Path, destination: &Path) -> Result<usize, String> {
    let zip = source
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("zip"))
        .unwrap_or(false);
    if zip {
        preserve_zip_identity_metadata(source, destination)
    } else {
        preserve_sibling_identity_metadata(source, destination)
    }
}

fn cleanup_failed_workspace(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

fn payload_partitions(executable: &Path, source: &str) -> Result<Vec<String>, String> {
    let output = run_executable(executable, &["-l", "-m", source])?;
    if !output.success() {
        return Err(format!(
            "Unable to list payload partitions: {}",
            output.combined_output()
        ));
    }
    let mut selected = output
        .stdout
        .lines()
        .filter_map(|line| line.trim().split_once(':').map(|(name, _)| name.trim()))
        .filter(|name| safe_partition_name(name))
        .map(str::to_string)
        .collect::<Vec<_>>();
    selected.sort();
    selected.dedup();
    if selected.is_empty() {
        Err("Payload contains no partitions covered by FlashROM's beta safety policy.".into())
    } else {
        Ok(selected)
    }
}

fn prepare_payload_inner(
    app: &tauri::AppHandle,
    source: &str,
    destination: &str,
) -> Result<PreparedRomInput, String> {
    let source = require_regular_file(source)?;
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case("bin") && !extension.eq_ignore_ascii_case("zip") {
        return Err("Payload preparation accepts payload.bin or an OTA .zip input.".into());
    }

    let destination = prepare_empty_workspace(destination)?;
    let identity_count = preserve_identity_metadata(&source, &destination)?;
    let (_, executable) = resolve_tool(SpecialTool::PayloadDumper);
    let source_text = source.to_string_lossy().to_string();
    let destination_text = destination.to_string_lossy().to_string();
    let selected = match payload_partitions(&executable, &source_text) {
        Ok(value) => value,
        Err(error) => {
            cleanup_failed_workspace(&destination);
            return Err(error);
        }
    };
    let selected_text = selected.join(",");
    let output = match run_executable_streaming(
        app,
        "prepare-payload",
        &executable,
        &["-o", &destination_text, "-p", &selected_text, &source_text],
    ) {
        Ok(value) => value,
        Err(error) => {
            cleanup_failed_workspace(&destination);
            return Err(format!(
                "payload-dumper-go is unavailable. Configure FLASHROM_PAYLOAD_DUMPER or tools/payload-dumper-go before preparing payload ROMs: {error}"
            ));
        }
    };

    if !output.success() {
        let diagnostic = output.combined_output();
        cleanup_failed_workspace(&destination);
        return Err(format!(
            "Payload extraction failed with exit code {}. Incremental OTAs may require previous/base images and are intentionally not guessed automatically. {diagnostic}",
            output.status
        ));
    }

    let ignored_image_count = quarantine_unsupported_images(&destination)?;
    let (artifacts, total_bytes) = collect_prepared_images(&destination)?;
    Ok(PreparedRomInput {
        source: source_text,
        destination: destination_text,
        kind: "payload_images".into(),
        diagnostic: format!(
            "payload-dumper-go verified and extracted {} allowlisted partition image(s); {identity_count} ROM identity metadata file(s) were preserved. Re-select the prepared directory so FlashROM rebuilds compatibility, partition metadata, ordering and SHA-256 Guard.",
            artifacts.len()
        ),
        artifacts,
        total_bytes,
        ignored_image_count,
    })
}

fn prepare_super_inner(
    app: &tauri::AppHandle,
    source: &str,
    destination: &str,
) -> Result<PreparedRomInput, String> {
    let source = require_regular_file(source)?;
    if source
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("super.img"))
        != Some(true)
    {
        return Err("Dynamic-partition preparation requires a file named super.img.".into());
    }

    let destination = prepare_empty_workspace(destination)?;
    let identity_count = preserve_identity_metadata(&source, &destination)?;
    let source_text = source.to_string_lossy().to_string();
    let destination_text = destination.to_string_lossy().to_string();
    let (_, lpunpack) = resolve_tool(SpecialTool::LpUnpack);
    let sparse = android_sparse_image(&source)?;

    let raw_path = destination.join("_flashrom_raw_super.img");
    let unpack_source = if sparse {
        let (_, simg2img) = resolve_tool(SpecialTool::Simg2Img);
        let raw_text = raw_path.to_string_lossy().to_string();
        let conversion = match run_executable_streaming(
            app,
            "prepare-super-unsparse",
            &simg2img,
            &[&source_text, &raw_text],
        ) {
            Ok(value) => value,
            Err(error) => {
                cleanup_failed_workspace(&destination);
                return Err(format!(
                    "super.img is Android sparse and requires simg2img. Configure FLASHROM_SIMG2IMG or tools/dynamic-partitions: {error}"
                ));
            }
        };
        if !conversion.success() || !raw_path.is_file() {
            let diagnostic = conversion.combined_output();
            cleanup_failed_workspace(&destination);
            return Err(format!(
                "simg2img failed with exit code {}: {diagnostic}",
                conversion.status
            ));
        }
        raw_text
    } else {
        source_text.clone()
    };

    let unpack = match run_executable_streaming(
        app,
        "prepare-super-unpack",
        &lpunpack,
        &[&unpack_source, &destination_text],
    ) {
        Ok(value) => value,
        Err(error) => {
            cleanup_failed_workspace(&destination);
            return Err(format!(
                "lpunpack is unavailable. Configure FLASHROM_LPUNPACK or tools/dynamic-partitions before unpacking super.img: {error}"
            ));
        }
    };
    let _ = fs::remove_file(&raw_path);

    if !unpack.success() {
        let diagnostic = unpack.combined_output();
        cleanup_failed_workspace(&destination);
        return Err(format!(
            "lpunpack failed with exit code {}: {diagnostic}",
            unpack.status
        ));
    }

    let ignored_image_count = quarantine_unsupported_images(&destination)?;
    let (artifacts, total_bytes) = collect_prepared_images(&destination)?;
    Ok(PreparedRomInput {
        source: source_text,
        destination: destination_text,
        kind: "super_partition_images".into(),
        diagnostic: format!(
            "super.img was unpacked into {} allowlisted logical partition image(s); {ignored_image_count} unsupported image(s) were quarantined and {identity_count} identity metadata file(s) were preserved. Slot-qualified filenames remain explicit and are selected only after live device metadata confirms the slot layout.",
            artifacts.len()
        ),
        artifacts,
        total_bytes,
        ignored_image_count,
    })
}

#[tauri::command]
pub async fn prepare_payload_input(
    app: tauri::AppHandle,
    source: String,
    destination: String,
) -> Result<PreparedRomInput, String> {
    tauri::async_runtime::spawn_blocking(move || prepare_payload_inner(&app, &source, &destination))
        .await
        .map_err(|error| format!("Payload preparation worker failed: {error}"))?
}

#[tauri::command]
pub async fn prepare_super_input(
    app: tauri::AppHandle,
    source: String,
    destination: String,
) -> Result<PreparedRomInput, String> {
    tauri::async_runtime::spawn_blocking(move || prepare_super_inner(&app, &source, &destination))
        .await
        .map_err(|error| format!("super.img preparation worker failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::{android_sparse_image, executable_name, safe_partition_name, SpecialTool};
    use std::{fs, time::SystemTime};

    #[test]
    fn uses_platform_specific_tool_names() {
        let payload = executable_name(SpecialTool::PayloadDumper);
        assert!(payload.starts_with("payload-dumper-go"));
    }

    #[test]
    fn accepts_only_beta_policy_partitions() {
        assert!(safe_partition_name("system"));
        assert!(safe_partition_name("system_a"));
        assert!(safe_partition_name("boot_b"));
        assert!(!safe_partition_name("modem"));
        assert!(!safe_partition_name("abl_a"));
    }

    #[test]
    fn detects_android_sparse_magic() {
        let path =
            std::env::temp_dir().join(format!("flashrom-sparse-test-{:?}.img", SystemTime::now()));
        fs::write(&path, [0x3a, 0xff, 0x26, 0xed, 0, 0, 0, 0]).expect("write test image");
        assert!(android_sparse_image(&path).expect("inspect sparse image"));
        let _ = fs::remove_file(path);
    }
}
