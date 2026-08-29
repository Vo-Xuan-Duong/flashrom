use crate::final_plan::FinalFlashPlanStep;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum OrderingClass {
    BootChain,
    SystemPayload,
    AvbMetadata,
}

fn policy_rank(base_partition: &str) -> Option<(OrderingClass, u16)> {
    match base_partition {
        "init_boot" => Some((OrderingClass::BootChain, 10)),
        "vendor_kernel_boot" => Some((OrderingClass::BootChain, 20)),
        "vendor_boot" => Some((OrderingClass::BootChain, 30)),
        "dtbo" => Some((OrderingClass::BootChain, 40)),
        "boot" => Some((OrderingClass::BootChain, 50)),
        "recovery" => Some((OrderingClass::BootChain, 60)),

        "system" => Some((OrderingClass::SystemPayload, 110)),
        "system_ext" => Some((OrderingClass::SystemPayload, 120)),
        "product" => Some((OrderingClass::SystemPayload, 130)),
        "vendor" => Some((OrderingClass::SystemPayload, 140)),
        "odm" => Some((OrderingClass::SystemPayload, 150)),
        "system_dlkm" => Some((OrderingClass::SystemPayload, 160)),
        "vendor_dlkm" => Some((OrderingClass::SystemPayload, 170)),
        "odm_dlkm" => Some((OrderingClass::SystemPayload, 180)),

        "vbmeta_vendor" => Some((OrderingClass::AvbMetadata, 210)),
        "vbmeta_system" => Some((OrderingClass::AvbMetadata, 220)),
        "vbmeta" => Some((OrderingClass::AvbMetadata, 230)),
        _ => None,
    }
}

pub(crate) fn ordering_class_label(base_partition: &str) -> Option<&'static str> {
    match policy_rank(base_partition)?.0 {
        OrderingClass::BootChain => Some("boot_chain"),
        OrderingClass::SystemPayload => Some("system_payload"),
        OrderingClass::AvbMetadata => Some("avb_metadata"),
    }
}

pub(crate) fn order_final_steps(
    steps: &[FinalFlashPlanStep],
) -> Result<Vec<FinalFlashPlanStep>, String> {
    let mut ranked = steps
        .iter()
        .cloned()
        .map(|step| {
            let (class, rank) = policy_rank(&step.base_partition).ok_or_else(|| {
                format!(
                    "No conservative ordering rule exists for partition {} (base {}).",
                    step.partition, step.base_partition
                )
            })?;
            Ok((class, rank, step))
        })
        .collect::<Result<Vec<_>, String>>()?;

    ranked.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then(left.1.cmp(&right.1))
            .then(left.2.partition.cmp(&right.2.partition))
    });

    Ok(ranked.into_iter().map(|(_, _, step)| step).collect())
}

#[cfg(test)]
mod tests {
    use super::{order_final_steps, ordering_class_label};
    use crate::final_plan::FinalFlashPlanStep;

    fn step(base: &str, partition: &str) -> FinalFlashPlanStep {
        FinalFlashPlanStep {
            image: format!("{base}.img"),
            image_path: format!("C:/rom/{base}.img"),
            image_size: 1,
            base_partition: base.into(),
            partition: partition.into(),
            partition_size: Some(2),
            logical: Some(false),
            required_mode: "Fastboot".into(),
            phase: 1,
            state: "ready".into(),
            command_preview: String::new(),
            warning: None,
        }
    }

    #[test]
    fn places_avb_metadata_after_payloads() {
        let input = vec![
            step("vbmeta", "vbmeta_a"),
            step("system", "system_a"),
            step("boot", "boot_a"),
        ];
        let ordered = order_final_steps(&input).expect("ordering should succeed");
        assert_eq!(ordered[0].base_partition, "boot");
        assert_eq!(ordered[1].base_partition, "system");
        assert_eq!(ordered[2].base_partition, "vbmeta");
    }

    #[test]
    fn orders_ab_targets_deterministically() {
        let input = vec![step("boot", "boot_b"), step("boot", "boot_a")];
        let ordered = order_final_steps(&input).expect("ordering should succeed");
        assert_eq!(ordered[0].partition, "boot_a");
        assert_eq!(ordered[1].partition, "boot_b");
    }

    #[test]
    fn refuses_unknown_partition_ordering() {
        assert!(order_final_steps(&[step("custom", "custom")]).is_err());
    }

    #[test]
    fn exposes_policy_class() {
        assert_eq!(ordering_class_label("boot"), Some("boot_chain"));
        assert_eq!(ordering_class_label("system"), Some("system_payload"));
        assert_eq!(ordering_class_label("vbmeta"), Some("avb_metadata"));
    }
}
