use serde::Serialize;

use crate::final_plan::{resolve_final_flash_plan_inner, FinalFlashPlan};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPreviewAction {
    index: usize,
    kind: String,
    mode: Option<String>,
    partition: Option<String>,
    image: Option<String>,
    command_preview: Option<String>,
    description: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPreview {
    final_plan: FinalFlashPlan,
    actions: Vec<ExecutionPreviewAction>,
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
    command_preview: Option<String>,
    description: impl Into<String>,
) {
    actions.push(ExecutionPreviewAction {
        index: actions.len() + 1,
        kind: kind.into(),
        mode: mode.map(str::to_string),
        partition: partition.map(str::to_string),
        image: image.map(str::to_string),
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

    push_action(
        &mut actions,
        "preflight",
        Some(&final_plan.current_mode),
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
            automatic_execution_enabled: false,
            diagnostic:
                "Dry run stopped at preflight because the validated Final Flash Plan is blocked."
                    .into(),
        });
    }

    let mut expected_mode = final_plan.current_mode.clone();

    for step in &final_plan.steps {
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
                Some(command),
                format!(
                    "Transition from {expected_mode} to {} and wait for the same serial to reappear before continuing.",
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
            None,
            format!(
                "Re-check product, unlocked=yes, snapshot status, target size and logical/physical metadata for {}.",
                step.partition
            ),
        );

        push_action(
            &mut actions,
            "flash_preview",
            Some(&expected_mode),
            Some(&step.partition),
            Some(&step.image),
            Some(step.command_preview.clone()),
            format!(
                "Preview the guarded write of {} to {}. This dry run does not execute the command.",
                step.image, step.partition
            ),
        );

        push_action(
            &mut actions,
            "post_write_check",
            Some(&expected_mode),
            Some(&step.partition),
            Some(&step.image),
            Some(format!(
                "fastboot -s {serial} getvar partition-size:{}",
                step.partition
            )),
            "Future executor must confirm the device remains connected in the expected mode after the write. This is a state check, not cryptographic image verification.",
        );
    }

    push_action(
        &mut actions,
        "finish",
        Some(&expected_mode),
        None,
        None,
        None,
        "Stop after the validated write sequence. Reboot and Clean Data remain explicit separate user choices.",
    );

    Ok(ExecutionPreview {
        final_plan,
        actions,
        blocked_reason: None,
        automatic_execution_enabled: false,
        diagnostic: "Execution sequence was generated as a dry run only. Automatic full-ROM writes remain disabled until partition ordering and stronger post-write verification rules are finalized.".into(),
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
