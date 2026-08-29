use serde::Serialize;

use crate::{
    final_plan::{resolve_final_flash_plan_inner, FinalFlashPlan},
    ordering::{order_final_steps, ordering_class_label},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPreviewAction {
    index: usize,
    kind: String,
    mode: Option<String>,
    partition: Option<String>,
    image: Option<String>,
    policy_class: Option<String>,
    command_preview: Option<String>,
    description: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPreview {
    final_plan: FinalFlashPlan,
    actions: Vec<ExecutionPreviewAction>,
    ordering_policy: String,
    ordering_policy_complete: bool,
    blocked_reason: Option<String>,
    automatic_execution_enabled: bool,
    diagnostic: String,
}

fn transition_command(serial: &str, target_mode: &str) -> Option<String> {
    match target_mode {
        "Fastboot" => Some(format!("fastboot -s {serial} reboot bootloader")),
        "FastbootD" => Some(format!("fastboot -s {serial} reboot fastboot")),
        _ => None,
    }
}

fn push_action(
    actions: &mut Vec<ExecutionPreviewAction>,
    kind: &str,
    mode: Option<&str>,
    partition: Option<&str>,
    image: Option<&str>,
    policy_class: Option<&str>,
    command_preview: Option<String>,
    description: impl Into<String>,
) {
    actions.push(ExecutionPreviewAction {
        index: actions.len() + 1,
        kind: kind.into(),
        mode: mode.map(str::to_string),
        partition: partition.map(str::to_string),
        image: image.map(str::to_string),
        policy_class: policy_class.map(str::to_string),
        command_preview,
        description: description.into(),
    });
}

#[tauri::command]
pub fn build_execution_preview(
    path: String,
    serial: String,
    slot_strategy: String,
) -> Result<ExecutionPreview, String> {
    let final_plan = resolve_final_flash_plan_inner(&path, &serial, &slot_strategy)?;
    let mut actions = Vec::new();
    let ordering_policy = "conservative-v1: boot-chain -> system-payload -> AVB-metadata";

    push_action(
        &mut actions,
        "preflight",
        Some(&final_plan.current_mode),
        None,
        None,
        None,
        None,
        "Re-resolve ROM compatibility, bootloader state, snapshot state, active slot and partition metadata immediately before any future executor is allowed to write.",
    );

    if !final_plan.ready_for_execution {
        return Ok(ExecutionPreview {
            blocked_reason: Some(
                "Final Flash Plan is not fully ready. Resolve all warnings and blocked/manual-only steps first."
                    .into(),
            ),
            final_plan,
            actions,
            ordering_policy: ordering_policy.into(),
            ordering_policy_complete: false,
            automatic_execution_enabled: false,
            diagnostic:
                "Dry run stopped at preflight because the validated Final Flash Plan is blocked."
                    .into(),
        });
    }

    let ordered_steps = match order_final_steps(&final_plan.steps) {
        Ok(steps) => steps,
        Err(error) => {
            return Ok(ExecutionPreview {
                blocked_reason: Some(error),
                final_plan,
                actions,
                ordering_policy: ordering_policy.into(),
                ordering_policy_complete: false,
                automatic_execution_enabled: false,
                diagnostic: "Dry run stopped because at least one resolved partition has no explicit ordering rule."
                    .into(),
            });
        }
    };

    let mut expected_mode = final_plan.current_mode.clone();

    for step in &ordered_steps {
        let policy_class = ordering_class_label(&step.base_partition).ok_or_else(|| {
            format!(
                "Ordering policy class is missing for {}.",
                step.base_partition
            )
        })?;

        if step.required_mode != expected_mode {
            let command = transition_command(&serial, &step.required_mode).ok_or_else(|| {
                format!(
                    "No safe Fastboot mode transition is defined for {}.",
                    step.required_mode
                )
            })?;

            push_action(
                &mut actions,
                "mode_transition",
                Some(&step.required_mode),
                None,
                None,
                Some(policy_class),
                Some(command),
                format!(
                    "Transition from {expected_mode} to {} and wait for the same serial to reappear before continuing with the {policy_class} class.",
                    step.required_mode
                ),
            );
            expected_mode = step.required_mode.clone();
        }

        push_action(
            &mut actions,
            "revalidate_step",
            Some(&expected_mode),
            Some(&step.partition),
            Some(&step.image),
            Some(policy_class),
            None,
            format!(
                "Re-check product, unlocked=yes, snapshot status, target size and logical/physical metadata for {} before the {policy_class} write.",
                step.partition
            ),
        );

        push_action(
            &mut actions,
            "flash_preview",
            Some(&expected_mode),
            Some(&step.partition),
            Some(&step.image),
            Some(policy_class),
            Some(step.command_preview.clone()),
            format!(
                "Preview the guarded {policy_class} write of {} to {}. This dry run does not execute the command.",
                step.image, step.partition
            ),
        );

        push_action(
            &mut actions,
            "post_write_check",
            Some(&expected_mode),
            Some(&step.partition),
            Some(&step.image),
            Some(policy_class),
            Some(format!(
                "fastboot -s {serial} getvar partition-size:{}",
                step.partition
            )),
            "Future executor must confirm the same serial remains connected in the expected mode after the write. This is a state check, not cryptographic image verification.",
        );
    }

    push_action(
        &mut actions,
        "finish",
        Some(&expected_mode),
        None,
        None,
        None,
        None,
        "Stop after the conservatively ordered validated write sequence. Reboot and Clean Data remain explicit separate user choices.",
    );

    Ok(ExecutionPreview {
        final_plan,
        actions,
        ordering_policy: ordering_policy.into(),
        ordering_policy_complete: true,
        blocked_reason: None,
        automatic_execution_enabled: false,
        diagnostic: "Execution sequence was generated with the conservative ordering policy as a dry run only. Automatic full-ROM writes remain disabled until stronger post-write verification and executor guards are finalized.".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::transition_command;

    #[test]
    fn builds_known_mode_transitions() {
        assert_eq!(
            transition_command("ABC", "Fastboot"),
            Some("fastboot -s ABC reboot bootloader".into())
        );
        assert_eq!(
            transition_command("ABC", "FastbootD"),
            Some("fastboot -s ABC reboot fastboot".into())
        );
        assert_eq!(transition_command("ABC", "Recovery"), None);
    }
}
