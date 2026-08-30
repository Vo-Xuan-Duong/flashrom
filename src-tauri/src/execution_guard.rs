use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    final_plan::{resolve_final_flash_plan_inner, FinalFlashPlan},
    ordering::{order_final_steps, ordering_class_label},
};

const HASH_BUFFER_BYTES: usize = 1024 * 1024;
const ORDERING_POLICY: &str = "conservative-v1: boot-chain -> system-payload -> AVB-metadata";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionGuardStep {
    pub(crate) index: usize,
    pub(crate) image: String,
    pub(crate) image_path: String,
    pub(crate) partition: String,
    pub(crate) required_mode: String,
    pub(crate) policy_class: String,
    pub(crate) image_size: u64,
    pub(crate) sha256: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionGuardReport {
    pub(crate) final_plan: FinalFlashPlan,
    pub(crate) ordering_policy: String,
    pub(crate) steps: Vec<ExecutionGuardStep>,
    pub(crate) state_stable_during_hashing: bool,
    pub(crate) ready_for_executor: bool,
    pub(crate) diagnostic: String,
}

#[derive(Debug, PartialEq, Eq)]
struct PlanSignature {
    product: Option<String>,
    active_slot: Option<String>,
    current_mode: String,
    bootloader_unlocked: Option<bool>,
    snapshot_update_status: Option<String>,
    steps: Vec<StepSignature>,
}

#[derive(Debug, PartialEq, Eq)]
struct StepSignature {
    image_path: String,
    image_size: u64,
    partition: String,
    partition_size: Option<u64>,
    logical: Option<bool>,
    required_mode: String,
    state: String,
}

fn plan_signature(plan: &FinalFlashPlan) -> PlanSignature {
    PlanSignature {
        product: plan.compatibility.device_product.clone(),
        active_slot: plan.active_slot.clone(),
        current_mode: plan.current_mode.clone(),
        bootloader_unlocked: plan.bootloader_unlocked,
        snapshot_update_status: plan.snapshot_update_status.clone(),
        steps: plan
            .steps
            .iter()
            .map(|step| StepSignature {
                image_path: step.image_path.clone(),
                image_size: step.image_size,
                partition: step.partition.clone(),
                partition_size: step.partition_size,
                logical: step.logical,
                required_mode: step.required_mode.clone(),
                state: step.state.clone(),
            })
            .collect(),
    }
}

pub(crate) fn sha256_file(path: &str, expected_size: u64) -> Result<String, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("Unable to read image metadata for {path}: {error}"))?;
    if !metadata.is_file() {
        return Err(format!(
            "Execution guard image is not a regular file: {path}"
        ));
    }
    if metadata.len() != expected_size {
        return Err(format!(
            "Image size changed since Final Flash Plan resolution: {path}; expected {expected_size}, found {}.",
            metadata.len()
        ));
    }

    let file = File::open(Path::new(path))
        .map_err(|error| format!("Unable to open image for SHA-256 hashing: {path}: {error}"))?;
    let mut reader = BufReader::with_capacity(HASH_BUFFER_BYTES, file);
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    let mut hasher = Sha256::new();

    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("Unable to hash image {path}: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn build_execution_guard_inner(
    path: &str,
    serial: &str,
    slot_strategy: &str,
) -> Result<ExecutionGuardReport, String> {
    let before = resolve_final_flash_plan_inner(path, serial, slot_strategy)?;

    if !before.ready_for_execution {
        return Ok(ExecutionGuardReport {
            final_plan: before,
            ordering_policy: ORDERING_POLICY.into(),
            steps: Vec::new(),
            state_stable_during_hashing: false,
            ready_for_executor: false,
            diagnostic:
                "Execution guard stopped because the Final Flash Plan is not ready for execution."
                    .into(),
        });
    }

    let ordered = order_final_steps(&before.steps)?;
    let before_signature = plan_signature(&before);
    let mut guarded_steps = Vec::with_capacity(ordered.len());

    for (index, step) in ordered.iter().enumerate() {
        let policy_class = ordering_class_label(&step.base_partition).ok_or_else(|| {
            format!(
                "Ordering policy class is missing for {}.",
                step.base_partition
            )
        })?;
        let sha256 = sha256_file(&step.image_path, step.image_size)?;

        guarded_steps.push(ExecutionGuardStep {
            index: index + 1,
            image: step.image.clone(),
            image_path: step.image_path.clone(),
            partition: step.partition.clone(),
            required_mode: step.required_mode.clone(),
            policy_class: policy_class.into(),
            image_size: step.image_size,
            sha256,
        });
    }

    let after = resolve_final_flash_plan_inner(path, serial, slot_strategy)?;
    let state_stable_during_hashing = before_signature == plan_signature(&after);
    let ready_for_executor = after.ready_for_execution && state_stable_during_hashing;

    let diagnostic = if ready_for_executor {
        format!(
            "Execution guard passed for {} ordered image(s). Device state remained stable while SHA-256 fingerprints were generated.",
            guarded_steps.len()
        )
    } else if !after.ready_for_execution {
        "Device or ROM preflight changed while image fingerprints were being generated. Executor remains blocked."
            .into()
    } else {
        "Device state or resolved target metadata changed while image fingerprints were being generated. Executor remains blocked."
            .into()
    };

    Ok(ExecutionGuardReport {
        final_plan: after,
        ordering_policy: ORDERING_POLICY.into(),
        steps: guarded_steps,
        state_stable_during_hashing,
        ready_for_executor,
        diagnostic,
    })
}

#[tauri::command]
pub async fn build_execution_guard(
    path: String,
    serial: String,
    slot_strategy: String,
) -> Result<ExecutionGuardReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        build_execution_guard_inner(&path, &serial, &slot_strategy)
    })
    .await
    .map_err(|error| format!("Execution guard worker failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::{plan_signature, sha256_file};
    use crate::{compatibility::RomCompatibility, final_plan::FinalFlashPlan};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn hashes_file_with_sha256() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("flashrom-hash-{unique}.img"));
        fs::write(&path, b"abc").expect("test image should be written");
        let digest =
            sha256_file(path.to_str().expect("utf8 temp path"), 3).expect("hash should succeed");
        fs::remove_file(path).ok();
        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn plan_signature_tracks_live_preflight_fields() {
        let compatibility = RomCompatibility {
            device_product: Some("sunstone".into()),
            rom_products: vec!["sunstone".into()],
            evidence: Vec::new(),
            status: "matched".into(),
            safe_to_auto_flash: true,
            diagnostic: String::new(),
        };
        let plan = FinalFlashPlan {
            compatibility,
            active_slot: Some("a".into()),
            slot_strategy: "active".into(),
            bootloader_unlocked: Some(true),
            snapshot_update_status: Some("none".into()),
            current_mode: "Fastboot".into(),
            steps: Vec::new(),
            warnings: Vec::new(),
            requires_mode_switch: false,
            ready_for_execution: true,
        };
        let signature = plan_signature(&plan);
        assert_eq!(signature.product.as_deref(), Some("sunstone"));
        assert_eq!(signature.active_slot.as_deref(), Some("a"));
        assert_eq!(signature.current_mode, "Fastboot");
    }
}
