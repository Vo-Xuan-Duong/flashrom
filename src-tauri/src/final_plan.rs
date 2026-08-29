use std::collections::BTreeMap;

use serde::Serialize;

use crate::{
    compatibility::{inspect_compatibility_inner, RomCompatibility},
    partition::{getvar, inspect_partitions_inner, PartitionMetadata, PartitionTargetMetadata},
    rom::{inspect_rom_inner, RomArtifact},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalFlashPlanStep {
    pub(crate) image: String,
    pub(crate) image_path: String,
    pub(crate) image_size: u64,
    pub(crate) base_partition: String,
    pub(crate) partition: String,
    pub(crate) partition_size: Option<u64>,
    pub(crate) logical: Option<bool>,
    pub(crate) required_mode: String,
    pub(crate) phase: u8,
    pub(crate) state: String,
    pub(crate) command_preview: String,
    pub(crate) warning: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalFlashPlan {
    pub(crate) compatibility: RomCompatibility,
    pub(crate) active_slot: Option<String>,
    pub(crate) slot_strategy: String,
    pub(crate) bootloader_unlocked: Option<bool>,
    pub(crate) snapshot_update_status: Option<String>,
    pub(crate) current_mode: String,
    pub(crate) steps: Vec<FinalFlashPlanStep>,
    pub(crate) warnings: Vec<String>,
    pub(crate) requires_mode_switch: bool,
    pub(crate) ready_for_execution: bool,
}

fn normalize_slot(value: Option<String>) -> Option<String> {
    let value = value?.trim().trim_start_matches('_').to_lowercase();
    match value.as_str() {
        "a" | "b" => Some(value),
        _ => None,
    }
}

fn parse_yes_no(value: Option<String>) -> Option<bool> {
    match value?.trim().to_lowercase().as_str() {
        "yes" | "true" | "1" => Some(true),
        "no" | "false" | "0" => Some(false),
        _ => None,
    }
}

fn base_partition_for_image(name: &str) -> Option<&'static str> {
    match name.to_lowercase().as_str() {
        "boot.img" => Some("boot"),
        "init_boot.img" => Some("init_boot"),
        "vendor_boot.img" => Some("vendor_boot"),
        "vendor_kernel_boot.img" => Some("vendor_kernel_boot"),
        "dtbo.img" => Some("dtbo"),
        "vbmeta.img" => Some("vbmeta"),
        "vbmeta_system.img" => Some("vbmeta_system"),
        "vbmeta_vendor.img" => Some("vbmeta_vendor"),
        "recovery.img" => Some("recovery"),
        "super.img" => Some("super"),
        "system.img" => Some("system"),
        "system_ext.img" => Some("system_ext"),
        "product.img" => Some("product"),
        "vendor.img" => Some("vendor"),
        "odm.img" => Some("odm"),
        "system_dlkm.img" => Some("system_dlkm"),
        "vendor_dlkm.img" => Some("vendor_dlkm"),
        "odm_dlkm.img" => Some("odm_dlkm"),
        _ => None,
    }
}

fn quote_path(path: &str) -> String {
    format!("\"{}\"", path.replace('"', "\\\""))
}

fn command(serial: &str, partition: &str, image_path: &str) -> String {
    format!(
        "fastboot -s {serial} flash {partition} {}",
        quote_path(image_path)
    )
}

fn blocked_step(
    artifact: &RomArtifact,
    base_partition: &str,
    partition: &str,
    serial: &str,
    warning: impl Into<String>,
) -> FinalFlashPlanStep {
    FinalFlashPlanStep {
        image: artifact.name.clone(),
        image_path: artifact.path.clone(),
        image_size: artifact.size,
        base_partition: base_partition.into(),
        partition: partition.into(),
        partition_size: None,
        logical: None,
        required_mode: "Unknown".into(),
        phase: 0,
        state: "blocked".into(),
        command_preview: command(serial, partition, &artifact.path),
        warning: Some(warning.into()),
    }
}

fn selected_targets(
    metadata: &PartitionMetadata,
    slot_strategy: &str,
    active_slot: Option<&str>,
) -> Result<Vec<PartitionTargetMetadata>, String> {
    match metadata.has_slot {
        Some(false) => Ok(metadata.targets.clone()),
        Some(true) if slot_strategy == "both" => Ok(metadata.targets.clone()),
        Some(true) => {
            let slot = active_slot.ok_or_else(|| {
                format!(
                    "{} is A/B, but Fastboot did not report a usable active slot.",
                    metadata.base_partition
                )
            })?;
            let suffix = format!("_{slot}");
            let target = metadata
                .targets
                .iter()
                .find(|target| target.name.ends_with(&suffix))
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "{} does not expose a target for active slot {slot}.",
                        metadata.base_partition
                    )
                })?;
            Ok(vec![target])
        }
        None => Err(format!(
            "Slot layout for {} could not be confirmed from Fastboot metadata.",
            metadata.base_partition
        )),
    }
}

fn resolved_step(
    artifact: &RomArtifact,
    base_partition: &str,
    target: &PartitionTargetMetadata,
    serial: &str,
) -> FinalFlashPlanStep {
    let partition_size = target.size_bytes;
    let logical = target.logical;
    let (required_mode, phase) = match logical {
        Some(false) => ("Fastboot", 1),
        Some(true) => ("FastbootD", 2),
        None => ("Unknown", 0),
    };

    let (state, warning) = if target.size_bytes.is_none() {
        (
            "blocked",
            Some(format!(
                "Partition {} does not report a usable size.",
                target.name
            )),
        )
    } else if target.logical.is_none() {
        (
            "blocked",
            Some(format!(
                "Partition {} does not report logical/physical status.",
                target.name
            )),
        )
    } else if artifact.size > target.size_bytes.unwrap_or(0) {
        (
            "blocked",
            Some(format!(
                "{} is {} bytes but partition {} is only {} bytes.",
                artifact.name,
                artifact.size,
                target.name,
                target.size_bytes.unwrap_or(0)
            )),
        )
    } else if artifact.name.eq_ignore_ascii_case("super.img") {
        (
            "manual_only",
            Some(
                "super.img remains manual-only because it can replace the complete dynamic partition container."
                    .into(),
            ),
        )
    } else {
        ("ready", None)
    };

    FinalFlashPlanStep {
        image: artifact.name.clone(),
        image_path: artifact.path.clone(),
        image_size: artifact.size,
        base_partition: base_partition.into(),
        partition: target.name.clone(),
        partition_size,
        logical,
        required_mode: required_mode.into(),
        phase,
        state: state.into(),
        command_preview: command(serial, &target.name, &artifact.path),
        warning,
    }
}

fn artifact_steps(
    artifact: &RomArtifact,
    metadata: &BTreeMap<String, PartitionMetadata>,
    slot_strategy: &str,
    active_slot: Option<&str>,
    serial: &str,
) -> Vec<FinalFlashPlanStep> {
    if artifact.kind != "image" && artifact.kind != "super_image" {
        return Vec::new();
    }

    let Some(base_partition) = base_partition_for_image(&artifact.name) else {
        return vec![blocked_step(
            artifact,
            "unknown",
            "unknown",
            serial,
            "Image filename is outside the safe partition allowlist.",
        )];
    };

    let Some(partition_metadata) = metadata.get(base_partition) else {
        return vec![blocked_step(
            artifact,
            base_partition,
            base_partition,
            serial,
            "No device partition metadata was returned for this image.",
        )];
    };

    match selected_targets(partition_metadata, slot_strategy, active_slot) {
        Ok(targets) => targets
            .iter()
            .map(|target| resolved_step(artifact, base_partition, target, serial))
            .collect(),
        Err(error) => vec![blocked_step(
            artifact,
            base_partition,
            base_partition,
            serial,
            error,
        )],
    }
}

pub(crate) fn resolve_final_flash_plan_inner(
    path: &str,
    serial: &str,
    slot_strategy: &str,
) -> Result<FinalFlashPlan, String> {
    if !matches!(slot_strategy, "active" | "both") {
        return Err("Slot strategy must be active or both.".into());
    }

    let compatibility = inspect_compatibility_inner(path, serial)?;
    let inspection = inspect_rom_inner(path)?;
    let active_slot = normalize_slot(getvar(serial, "current-slot"));
    let bootloader_unlocked = parse_yes_no(getvar(serial, "unlocked"));
    let snapshot_update_status = getvar(serial, "snapshot-update-status")
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());
    let current_mode = if parse_yes_no(getvar(serial, "is-userspace")) == Some(true) {
        "FastbootD"
    } else {
        "Fastboot"
    }
    .to_string();

    let mut bases = inspection
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == "image" || artifact.kind == "super_image")
        .filter_map(|artifact| base_partition_for_image(&artifact.name).map(str::to_string))
        .collect::<Vec<_>>();
    bases.sort();
    bases.dedup();

    let partition_metadata = inspect_partitions_inner(serial, bases)?
        .into_iter()
        .map(|metadata| (metadata.base_partition.clone(), metadata))
        .collect::<BTreeMap<_, _>>();

    let mut steps = Vec::new();
    for artifact in &inspection.artifacts {
        steps.extend(artifact_steps(
            artifact,
            &partition_metadata,
            slot_strategy,
            active_slot.as_deref(),
            serial,
        ));
    }

    let mut warnings = Vec::new();
    if !compatibility.safe_to_auto_flash {
        warnings.push(compatibility.diagnostic.clone());
    }
    if bootloader_unlocked != Some(true) {
        warnings.push(
            "Fastboot did not confirm unlocked=yes. Final-plan execution remains blocked.".into(),
        );
    }
    if let Some(snapshot) = snapshot_update_status.as_deref() {
        if snapshot != "none" {
            warnings.push(format!(
                "Snapshot update status is {snapshot}. Partition writes must wait until snapshot operations finish."
            ));
        }
    }
    if steps.is_empty() {
        warnings.push("No direct image flash steps were resolved from this ROM input.".into());
    }

    let has_fastboot = steps
        .iter()
        .any(|step| step.state == "ready" && step.required_mode == "Fastboot");
    let has_fastbootd = steps
        .iter()
        .any(|step| step.state == "ready" && step.required_mode == "FastbootD");
    let requires_mode_switch = has_fastboot && has_fastbootd;
    if requires_mode_switch {
        warnings.push(
            "The resolved plan contains both physical and logical partitions. Execution must be serialized into Fastboot and FastbootD phases."
                .into(),
        );
    }

    let snapshot_safe = snapshot_update_status
        .as_deref()
        .map(|value| value == "none")
        .unwrap_or(true);
    let ready_for_execution = compatibility.safe_to_auto_flash
        && bootloader_unlocked == Some(true)
        && snapshot_safe
        && !steps.is_empty()
        && steps.iter().all(|step| step.state == "ready");

    Ok(FinalFlashPlan {
        compatibility,
        active_slot,
        slot_strategy: slot_strategy.to_string(),
        bootloader_unlocked,
        snapshot_update_status,
        current_mode,
        steps,
        warnings,
        requires_mode_switch,
        ready_for_execution,
    })
}

#[tauri::command]
pub fn resolve_final_flash_plan(
    path: String,
    serial: String,
    slot_strategy: String,
) -> Result<FinalFlashPlan, String> {
    resolve_final_flash_plan_inner(&path, &serial, &slot_strategy)
}

#[cfg(test)]
mod tests {
    use super::{base_partition_for_image, normalize_slot};

    #[test]
    fn maps_supported_images() {
        assert_eq!(base_partition_for_image("boot.img"), Some("boot"));
        assert_eq!(base_partition_for_image("system.img"), Some("system"));
        assert_eq!(base_partition_for_image("unknown.img"), None);
    }

    #[test]
    fn normalizes_active_slot() {
        assert_eq!(normalize_slot(Some("_a".into())), Some("a".into()));
        assert_eq!(normalize_slot(Some("B".into())), Some("b".into()));
        assert_eq!(normalize_slot(Some("none".into())), None);
    }
}
