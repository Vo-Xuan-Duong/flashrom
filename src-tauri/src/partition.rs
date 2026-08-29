use serde::Serialize;

use crate::process::{run, AndroidTool, CommandOutput};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionTargetMetadata {
    pub(crate) name: String,
    pub(crate) logical: Option<bool>,
    pub(crate) size_bytes: Option<u64>,
    pub(crate) partition_type: Option<String>,
    pub(crate) recommended_mode: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionMetadata {
    pub(crate) base_partition: String,
    pub(crate) has_slot: Option<bool>,
    pub(crate) targets: Vec<PartitionTargetMetadata>,
    pub(crate) diagnostic: String,
}

fn fastboot_var(output: &CommandOutput, key: &str) -> Option<String> {
    let marker = format!("{key}:");
    output.combined_output().lines().find_map(|line| {
        let position = line.find(&marker)?;
        let value = line[position + marker.len()..].trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

pub(crate) fn getvar(serial: &str, key: &str) -> Option<String> {
    run(AndroidTool::Fastboot, &["-s", serial, "getvar", key])
        .ok()
        .and_then(|output| fastboot_var(&output, key))
}

fn parse_yes_no(value: Option<String>) -> Option<bool> {
    match value?.trim().to_lowercase().as_str() {
        "yes" | "true" | "1" => Some(true),
        "no" | "false" | "0" => Some(false),
        _ => None,
    }
}

fn parse_size(value: Option<String>) -> Option<u64> {
    let value = value?;
    let trimmed = value.trim();

    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u64::from_str_radix(hex, 16).ok();
    }

    trimmed.parse::<u64>().ok()
}

fn connected_fastboot_serials() -> Result<Vec<String>, String> {
    let output = run(AndroidTool::Fastboot, &["devices"])?;
    Ok(output
        .stdout
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|serial| !serial.is_empty())
        .map(str::to_string)
        .collect())
}

pub(crate) fn require_fastboot_serial(serial: &str) -> Result<(), String> {
    let connected = connected_fastboot_serials()?;
    if connected.iter().any(|value| value == serial) {
        Ok(())
    } else {
        Err(format!(
            "Device {serial} is not currently available through Fastboot. Reboot it to Bootloader or FastbootD and refresh detection."
        ))
    }
}

fn target_metadata(serial: &str, target: &str) -> PartitionTargetMetadata {
    let logical = parse_yes_no(getvar(serial, &format!("is-logical:{target}")));
    let size_bytes = parse_size(getvar(serial, &format!("partition-size:{target}")));
    let partition_type = getvar(serial, &format!("partition-type:{target}"));
    let recommended_mode = match logical {
        Some(true) => "FastbootD",
        Some(false) => "Fastboot",
        None => "Unknown",
    }
    .to_string();

    PartitionTargetMetadata {
        name: target.into(),
        logical,
        size_bytes,
        partition_type,
        recommended_mode,
    }
}

fn infer_slot_state(serial: &str, base: &str) -> Option<bool> {
    if let Some(value) = parse_yes_no(getvar(serial, &format!("has-slot:{base}"))) {
        return Some(value);
    }

    let slot_a = parse_size(getvar(serial, &format!("partition-size:{base}_a")));
    let slot_b = parse_size(getvar(serial, &format!("partition-size:{base}_b")));
    if slot_a.is_some() || slot_b.is_some() {
        return Some(true);
    }

    parse_size(getvar(serial, &format!("partition-size:{base}"))).map(|_| false)
}

pub(crate) fn inspect_partitions_inner(
    serial: &str,
    partitions: Vec<String>,
) -> Result<Vec<PartitionMetadata>, String> {
    if serial.trim().is_empty() {
        return Err("A detected device serial is required for partition probing.".into());
    }

    require_fastboot_serial(serial)?;

    let mut bases = partitions
        .into_iter()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    bases.sort();
    bases.dedup();

    if bases.len() > 32 {
        return Err("Partition probe is limited to 32 unique partitions per request.".into());
    }

    let mut result = Vec::with_capacity(bases.len());

    for base in bases {
        if !base
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return Err(format!("Invalid partition name: {base}"));
        }

        let has_slot = infer_slot_state(serial, &base);
        let target_names = match has_slot {
            Some(true) => vec![format!("{base}_a"), format!("{base}_b")],
            Some(false) => vec![base.clone()],
            None => vec![base.clone()],
        };
        let targets = target_names
            .iter()
            .map(|target| target_metadata(serial, target))
            .collect::<Vec<_>>();

        let diagnostic = match has_slot {
            Some(true) => format!("{base} uses A/B partition slots."),
            Some(false) => format!("{base} is a single target partition."),
            None => format!("Slot layout for {base} could not be confirmed."),
        };

        result.push(PartitionMetadata {
            base_partition: base,
            has_slot,
            targets,
            diagnostic,
        });
    }

    Ok(result)
}

#[tauri::command]
pub fn inspect_partitions(
    serial: String,
    partitions: Vec<String>,
) -> Result<Vec<PartitionMetadata>, String> {
    inspect_partitions_inner(&serial, partitions)
}

#[cfg(test)]
mod tests {
    use super::{parse_size, parse_yes_no};

    #[test]
    fn parses_fastboot_sizes() {
        assert_eq!(
            parse_size(Some("0x0000000004000000".into())),
            Some(67_108_864)
        );
        assert_eq!(parse_size(Some("4096".into())), Some(4096));
        assert_eq!(parse_size(Some("invalid".into())), None);
    }

    #[test]
    fn parses_yes_no_values() {
        assert_eq!(parse_yes_no(Some("yes".into())), Some(true));
        assert_eq!(parse_yes_no(Some("no".into())), Some(false));
        assert_eq!(parse_yes_no(Some("unknown".into())), None);
    }
}
