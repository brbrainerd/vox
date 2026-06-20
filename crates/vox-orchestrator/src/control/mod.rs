//! Drive Console control SSOT parity with `contracts/gui/drive-console.v1.yaml`.

#[cfg(test)]
mod parity_tests {
    use crate::mode::{ApprovalLean, ClutchProfile, QualityLevel, RiskPosture};

    fn quality_str(q: QualityLevel) -> &'static str {
        match q {
            QualityLevel::Flash => "flash",
            QualityLevel::Balanced => "balanced",
            QualityLevel::Premium => "premium",
        }
    }
    fn approval_str(a: ApprovalLean) -> &'static str {
        match a {
            ApprovalLean::AutoApproveMore => "auto_approve_more",
            ApprovalLean::Confirm => "confirm",
            ApprovalLean::Review => "review",
        }
    }

    #[test]
    fn code_matches_contract() {
        let raw = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../contracts/gui/drive-console.v1.yaml"
        ))
        .expect("drive-console.v1.yaml contract must be present");
        let doc: serde_yaml::Value = serde_yaml::from_str(&raw).expect("valid yaml");

        for (i, clutch) in [
            ClutchProfile::Free,
            ClutchProfile::Efficiency,
            ClutchProfile::Balanced,
            ClutchProfile::Genius,
        ]
        .into_iter()
        .enumerate()
        {
            let r = clutch.resolve();
            let row = &doc["clutch"][i];
            assert_eq!(
                row["quality"].as_str().unwrap(),
                quality_str(r.quality),
                "clutch[{i}] quality mismatch"
            );
            assert_eq!(
                row["force_free_pool"].as_bool().unwrap(),
                r.force_free_pool,
                "clutch[{i}] force_free_pool mismatch"
            );
            let axes: Vec<u8> = row["axes"]
                .as_sequence()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            assert_eq!(
                (axes[0], axes[1], axes[2]),
                r.axes,
                "clutch[{i}] axes mismatch"
            );
        }

        for (i, risk) in [RiskPosture::High, RiskPosture::Moderate, RiskPosture::Low]
            .into_iter()
            .enumerate()
        {
            let r = risk.resolve();
            let row = &doc["risk"][i];
            assert_eq!(
                row["approval"].as_str().unwrap(),
                approval_str(r.approval),
                "risk[{i}] approval mismatch"
            );
            assert_eq!(
                row["grounding_enforce"].as_bool().unwrap(),
                r.grounding_enforce,
                "risk[{i}] grounding_enforce mismatch"
            );
            assert_eq!(
                row["socrates_enforce"].as_bool().unwrap(),
                r.socrates_enforce,
                "risk[{i}] socrates_enforce mismatch"
            );
        }
    }
}
