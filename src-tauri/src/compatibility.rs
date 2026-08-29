use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::partition::{getvar, require_fastboot_serial};

const MAX_METADATA_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RomProductEvidence {
    pub(crate) product: String,
    pub(crate) source: String,
    pub(crate) key: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RomCompatibility {
    pub(crate) device_product: Option<String>,
    pub(crate) rom_products: Vec<String>,
    pub(crate) evidence: Vec<RomProductEvidence>,
    pub(crate) status: String,
    pub(crate) safe_to_auto_flash: bool,
    pub(crate) diagnostic: String,
}

fn normalize_product(value: &str) -> Option<String> {
    let value = value
        .trim()
        .trim_matches(|character: char| {
            matches!(character, '"' | '\'' | '[' | ']' | '(' | ')')
        })
        .to_lowercase();

    if value.is_empty()
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
    {
        return None;
    }

    Some(value)
}

fn split_products(value: &str) -> Vec<String> {
    value
        .split(|character: char| {
            matches!(character, '|' | ',' | ';') || character.is_whitespace()
        })
        .filter_map(normalize_product)
        .collect()
}

fn assignment_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let trimmed = line.trim();
    let (left, right) = trimmed.split_once('=')?;
    if left.trim().eq_ignore_ascii_case(key) {
        Some(right.trim())
    } else {
        None
    }
}

fn parse_metadata_text(contents: &str, source: &str) -> Vec<RomProductEvidence> {
    let mut result = Vec::new();

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let require_line = line
            .strip_prefix("require ")
            .or_else(|| line.strip_prefix("require\t"))
            .unwrap_or(line)
            .trim();

        for key in [
            "product",
            "board",
            "device",
            "pre-device",
            "post-device",
            "ro.product.device",
        ] {
            let Some(value) = assignment_value(require_line, key) else {
                continue;
            };

            for product in split_products(value) {
                result.push(RomProductEvidence {
                    product,
                    source: source.to_string(),
                    key: key.to_string(),
                });
            }
        }
    }

    result
}

fn metadata_root(input: &Path) -> Option<PathBuf> {
    if input.is_dir() {
        Some(input.to_path_buf())
    } else {
        input.parent().map(Path::to_path_buf)
    }
}

fn metadata_candidates(input: &Path) -> Vec<PathBuf> {
    let Some(root) = metadata_root(input) else {
        return Vec::new();
    };

    let relative_paths = [
        "android-info.txt",
        "metadata",
        "images/android-info.txt",
        "images/metadata",
        "META-INF/com/android/metadata",
    ];

    relative_paths
        .into_iter()
        .map(|relative| root.join(relative))
        .filter(|path| path.is_file())
        .collect()
}

fn read_metadata(path: &Path) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_METADATA_BYTES {
        return None;
    }
    fs::read_to_string(path).ok()
}

fn collect_evidence(path: &Path) -> Vec<RomProductEvidence> {
    let mut evidence = Vec::new();

    for candidate in metadata_candidates(path) {
        let Some(contents) = read_metadata(&candidate) else {
            continue;
        };
        evidence.extend(parse_metadata_text(
            &contents,
            &candidate.to_string_lossy(),
        ));
    }

    evidence.sort_by(|left, right| {
        left.product
            .cmp(&right.product)
            .then(left.source.cmp(&right.source))
            .then(left.key.cmp(&right.key))
    });
    evidence.dedup_by(|left, right| {
        left.product == right.product && left.source == right.source && left.key == right.key
    });
    evidence
}

pub(crate) fn inspect_compatibility_inner(
    path: &str,
    serial: &str,
) -> Result<RomCompatibility, String> {
    if serial.trim().is_empty() {
        return Err(
            "A detected Fastboot device serial is required for ROM compatibility validation."
                .into(),
        );
    }

    require_fastboot_serial(serial)?;

    let input = Path::new(path);
    if !input.exists() {
        return Err("ROM input does not exist.".into());
    }

    let device_product = getvar(serial, "product").and_then(|value| normalize_product(&value));
    let evidence = collect_evidence(input);
    let rom_products = evidence
        .iter()
        .map(|item| item.product.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let (status, safe_to_auto_flash, diagnostic) =
        match (&device_product, rom_products.is_empty()) {
            (None, _) => (
                "unknown",
                false,
                "Fastboot did not report a usable device product. Automatic ROM flashing remains blocked."
                    .to_string(),
            ),
            (Some(device), true) => (
                "unknown",
                false,
                format!(
                    "Device product is {device}, but no trusted ROM product/codename metadata was found. Automatic ROM flashing remains blocked."
                ),
            ),
            (Some(device), false) if rom_products.iter().any(|product| product == device) => (
                "matched",
                true,
                format!(
                    "ROM metadata matches Fastboot product {device}. Compatibility validation passed."
                ),
            ),
            (Some(device), false) => (
                "mismatch",
                false,
                format!(
                    "ROM metadata targets [{}], but the connected device reports product {device}. Flashing is blocked.",
                    rom_products.join(", ")
                ),
            ),
        };

    Ok(RomCompatibility {
        device_product,
        rom_products,
        evidence,
        status: status.into(),
        safe_to_auto_flash,
        diagnostic,
    })
}

#[tauri::command]
pub fn inspect_rom_compatibility(
    path: String,
    serial: String,
) -> Result<RomCompatibility, String> {
    inspect_compatibility_inner(&path, &serial)
}

#[cfg(test)]
mod tests {
    use super::{normalize_product, parse_metadata_text, split_products};

    #[test]
    fn parses_android_info_product_requirements() {
        let evidence = parse_metadata_text(
            "require product=sunstone|moonstone\nrequire version-bootloader=1\n",
            "android-info.txt",
        );
        assert_eq!(evidence.len(), 2);
        assert_eq!(evidence[0].product, "sunstone");
        assert_eq!(evidence[1].product, "moonstone");
    }

    #[test]
    fn parses_ota_device_metadata() {
        let evidence = parse_metadata_text(
            "ota-type=AB\npre-device=sunstone,moonstone\n",
            "metadata",
        );
        assert_eq!(evidence.len(), 2);
    }

    #[test]
    fn rejects_unsafe_product_tokens() {
        assert_eq!(normalize_product("sunstone"), Some("sunstone".into()));
        assert_eq!(normalize_product("sunstone && erase"), None);
        assert_eq!(split_products("sunstone|moonstone").len(), 2);
    }
}
