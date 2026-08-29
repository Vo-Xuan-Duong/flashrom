use std::path::Path;

use serde::Serialize;

use crate::rom::{inspect_rom_inner, RomArtifact};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashPlanStep {
    image: String,
    image_path: String,
    partition: String,
    required_mode: String,
    command_preview: String,
    state: String,
    warning: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashPlan {
    rom_kind: String,
    boot_layout: String,
    slot_strategy: String,
    active_slot: Option<String>,
    steps: Vec<FlashPlanStep>,
    warnings: Vec<String>,
    ready_for_validation: bool,
}

#[derive(Clone, Copy)]
enum PartitionClass {
    Boot,
    Physical,
    Dynamic,
    Super,
}

fn image_partition(name: &str) -> Option<(&'static str, PartitionClass)> {
    match name.to_lowercase().as_str() {
        "boot.img" => Some(("boot", PartitionClass::Boot)),
        "init_boot.img" => Some(("init_boot", PartitionClass::Physical)),
        "vendor_boot.img" => Some(("vendor_boot", PartitionClass::Physical)),
        "vendor_kernel_boot.img" => Some(("vendor_kernel_boot", PartitionClass::Physical)),
        "dtbo.img" => Some(("dtbo", PartitionClass::Physical)),
        "vbmeta.img" => Some(("vbmeta", PartitionClass::Physical)),
        "vbmeta_system.img" => Some(("vbmeta_system", PartitionClass::Physical)),
        "vbmeta_vendor.img" => Some(("vbmeta_vendor", PartitionClass::Physical)),
        "recovery.img" => Some(("recovery", PartitionClass::Physical)),
        "super.img" => Some(("super", PartitionClass::Super)),
        "system.img" => Some(("system", PartitionClass::Dynamic)),
        "system_ext.img" => Some(("system_ext", PartitionClass::Dynamic)),
        "product.img" => Some(("product", PartitionClass::Dynamic)),
        "vendor.img" => Some(("vendor", PartitionClass::Dynamic)),
        "odm.img" => Some(("odm", PartitionClass::Dynamic)),
        "system_dlkm.img" => Some(("system_dlkm", PartitionClass::Dynamic)),
        "vendor_dlkm.img" => Some(("vendor_dlkm", PartitionClass::Dynamic)),
        "odm_dlkm.img" => Some(("odm_dlkm", PartitionClass::Dynamic)),
        _ => None,
    }
}

fn normalized_slot(slot: Option<&str>) -> Option<&'static str> {
    match slot.map(|value| value.trim().trim_start_matches('_').to_lowercase()) {
        Some(value) if value == "a" => Some("a"),
        Some(value) if value == "b" => Some("b"),
        _ => None,
    }
}

fn quote_path(path: &str) -> String {
    format!("\"{}\"", path.replace('"', "\\\""))
}

fn command(serial: Option<&str>, partition: &str, image_path: &str) -> String {
    let serial = serial.unwrap_or("<serial>");
    format!(
        "fastboot -s {serial} flash {partition} {}",
        quote_path(image_path)
    )
}

fn boot_steps(
    artifact: &RomArtifact,
    boot_layout: &str,
    active_slot: Option<&str>,
    slot_strategy: &str,
    serial: Option<&str>,
) -> (Vec<FlashPlanStep>, Vec<String>) {
    let mut warnings = Vec::new();

    match boot_layout {
        "single" => (
            vec![FlashPlanStep {
                image: artifact.name.clone(),
                image_path: artifact.path.clone(),
                partition: "boot".into(),
                required_mode: "Fastboot".into(),
                command_preview: command(serial, "boot", &artifact.path),
                state: "resolved".into(),
                warning: None,
            }],
            warnings,
        ),
        "ab" if slot_strategy == "both" => (
            ["boot_a", "boot_b"]
                .into_iter()
                .map(|partition| FlashPlanStep {
                    image: artifact.name.clone(),
                    image_path: artifact.path.clone(),
                    partition: partition.into(),
                    required_mode: "Fastboot".into(),
                    command_preview: command(serial, partition, &artifact.path),
                    state: "resolved".into(),
                    warning: Some("Both boot slots are selected. Verify that this is intended for the ROM.".into()),
                })
                .collect(),
            warnings,
        ),
        "ab" => {
            if let Some(slot) = normalized_slot(active_slot) {
                let partition = format!("boot_{slot}");
                (
                    vec![FlashPlanStep {
                        image: artifact.name.clone(),
                        image_path: artifact.path.clone(),
                        partition: partition.clone(),
                        required_mode: "Fastboot".into(),
                        command_preview: command(serial, &partition, &artifact.path),
                        state: "resolved".into(),
                        warning: None,
                    }],
                    warnings,
                )
            } else {
                warnings.push(
                    "A/B boot layout is selected, but the active slot is unknown. Choose both slots or reconnect in Fastboot so the active slot can be detected."
                        .into(),
                );
                (
                    vec![FlashPlanStep {
                        image: artifact.name.clone(),
                        image_path: artifact.path.clone(),
                        partition: "boot_<slot>".into(),
                        required_mode: "Fastboot".into(),
                        command_preview: command(serial, "boot_<slot>", &artifact.path),
                        state: "blocked".into(),
                        warning: Some("Active A/B slot is unresolved.".into()),
                    }],
                    warnings,
                )
            }
        }
        _ => {
            warnings.push(
                "Boot layout is unknown. Select 1 partition or 2 partitions (A/B) before boot.img can be resolved."
                    .into(),
            );
            (
                vec![FlashPlanStep {
                    image: artifact.name.clone(),
                    image_path: artifact.path.clone(),
                    partition: "boot<?>".into(),
                    required_mode: "Fastboot".into(),
                    command_preview: command(serial, "boot<?>", &artifact.path),
                    state: "blocked".into(),
                    warning: Some("Boot partition layout is unresolved.".into()),
                }],
                warnings,
            )
        }
    }
}

fn unresolved_step(
    artifact: &RomArtifact,
    partition: &str,
    required_mode: &str,
    serial: Option<&str>,
    warning: &str,
) -> FlashPlanStep {
    FlashPlanStep {
        image: artifact.name.clone(),
        image_path: artifact.path.clone(),
        partition: format!("{partition}<?>"),
        required_mode: required_mode.into(),
        command_preview: command(serial, &format!("{partition}<?>"), &artifact.path),
        state: "needs_partition_metadata".into(),
        warning: Some(warning.into()),
    }
}

fn artifact_steps(
    artifact: &RomArtifact,
    boot_layout: &str,
    active_slot: Option<&str>,
    slot_strategy: &str,
    serial: Option<&str>,
) -> (Vec<FlashPlanStep>, Vec<String>) {
    if artifact.kind != "image" && artifact.kind != "super_image" {
        return (Vec::new(), Vec::new());
    }

    let Some((partition, class)) = image_partition(&artifact.name) else {
        return (
            vec![FlashPlanStep {
                image: artifact.name.clone(),
                image_path: artifact.path.clone(),
                partition: "unknown".into(),
                required_mode: "Unknown".into(),
                command_preview: "No command generated".into(),
                state: "unsupported".into(),
                warning: Some("Image filename is not in the safe partition mapping allowlist.".into()),
            }],
            vec![format!(
                "{} is not mapped to a known partition and will not be auto-flashed.",
                artifact.name
            )],
        );
    };

    match class {
        PartitionClass::Boot => boot_steps(
            artifact,
            boot_layout,
            active_slot,
            slot_strategy,
            serial,
        ),
        PartitionClass::Super => (
            vec![FlashPlanStep {
                image: artifact.name.clone(),
                image_path: artifact.path.clone(),
                partition: partition.into(),
                required_mode: "Fastboot".into(),
                command_preview: command(serial, partition, &artifact.path),
                state: "needs_compatibility_check".into(),
                warning: Some(
                    "super.img is a large physical image. Product/codename and partition-size validation are required before flashing."
                        .into(),
                ),
            }],
            vec!["super.img requires explicit device compatibility validation.".into()],
        ),
        PartitionClass::Physical => (
            vec![unresolved_step(
                artifact,
                partition,
                "Fastboot",
                serial,
                "The device must report whether this partition has A/B slots before the target can be finalized.",
            )],
            Vec::new(),
        ),
        PartitionClass::Dynamic => (
            vec![unresolved_step(
                artifact,
                partition,
                "FastbootD",
                serial,
                "Dynamic/logical partition metadata must be checked in FastbootD before the target can be finalized.",
            )],
            Vec::new(),
        ),
    }
}

#[tauri::command]
pub fn generate_flash_plan(
    path: String,
    boot_layout: String,
    active_slot: Option<String>,
    slot_strategy: String,
    serial: Option<String>,
) -> Result<FlashPlan, String> {
    if !matches!(boot_layout.as_str(), "single" | "ab" | "unknown") {
        return Err("Unsupported boot layout.".into());
    }

    if !matches!(slot_strategy.as_str(), "active" | "both") {
        return Err("Slot strategy must be active or both.".into());
    }

    let inspection = inspect_rom_inner(&path)?;
    let mut steps = Vec::new();
    let mut warnings = Vec::new();

    if inspection.kind == "recovery_zip" {
        warnings.push(
            "Recovery ZIP detected. It needs an ADB sideload/recovery installation flow, not direct partition flashing."
                .into(),
        );
    }

    if inspection.kind == "payload_bin" || inspection.kind == "payload_package" {
        warnings.push(
            "payload.bin detected. Payload extraction or update-engine handling is required before a partition flash plan can be generated."
                .into(),
        );
    }

    for artifact in &inspection.artifacts {
        let (mut artifact_steps, artifact_warnings) = artifact_steps(
            artifact,
            &boot_layout,
            active_slot.as_deref(),
            &slot_strategy,
            serial.as_deref(),
        );
        steps.append(&mut artifact_steps);
        warnings.extend(artifact_warnings);
    }

    if steps.is_empty() && warnings.is_empty() {
        warnings.push("No recognized flashable image artifacts were found in this ROM input.".into());
    }

    let ready_for_validation = !steps.is_empty()
        && steps
            .iter()
            .all(|step| step.state == "resolved" || step.state == "needs_compatibility_check");

    Ok(FlashPlan {
        rom_kind: inspection.kind,
        boot_layout,
        slot_strategy,
        active_slot: active_slot.and_then(|value| {
            normalized_slot(Some(&value)).map(|slot| slot.to_string())
        }),
        steps,
        warnings,
        ready_for_validation,
    })
}

#[cfg(test)]
mod tests {
    use super::{artifact_steps, normalized_slot};
    use crate::rom::RomArtifact;

    fn image(name: &str) -> RomArtifact {
        RomArtifact {
            name: name.into(),
            path: format!("C:/rom/{name}"),
            kind: "image".into(),
            size: 1,
        }
    }

    #[test]
    fn resolves_single_slot_boot() {
        let (steps, warnings) = artifact_steps(
            &image("boot.img"),
            "single",
            None,
            "active",
            Some("ABC"),
        );
        assert!(warnings.is_empty());
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].partition, "boot");
        assert_eq!(steps[0].state, "resolved");
    }

    #[test]
    fn resolves_both_ab_boot_targets() {
        let (steps, _) = artifact_steps(
            &image("boot.img"),
            "ab",
            Some("b"),
            "both",
            Some("ABC"),
        );
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].partition, "boot_a");
        assert_eq!(steps[1].partition, "boot_b");
    }

    #[test]
    fn dynamic_partition_requires_metadata() {
        let (steps, _) = artifact_steps(
            &image("system.img"),
            "ab",
            Some("a"),
            "active",
            Some("ABC"),
        );
        assert_eq!(steps[0].required_mode, "FastbootD");
        assert_eq!(steps[0].state, "needs_partition_metadata");
    }

    #[test]
    fn normalizes_slots() {
        assert_eq!(normalized_slot(Some("_A")), Some("a"));
        assert_eq!(normalized_slot(Some("b")), Some("b"));
        assert_eq!(normalized_slot(Some("x")), None);
    }

    #[test]
    fn path_filename_helper_remains_standard() {
        assert_eq!(
            Path::new("C:/rom/boot.img")
                .file_name()
                .and_then(|value| value.to_str()),
            Some("boot.img")
        );
    }
}
