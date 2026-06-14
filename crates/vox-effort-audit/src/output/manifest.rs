//! Run manifest writer for `manifest.json`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: String,
    pub run_id: String,
    pub run_started: chrono::DateTime<chrono::Utc>,
    pub run_completed: chrono::DateTime<chrono::Utc>,
    pub vox_version: String,
    pub effort_audit_crate_version: String,
    pub range: RangeManifest,
    pub commits_in_range: u64,
    pub commits_judged: u64,
    pub commits_skipped: u64,
    pub judge_model_id_resolved: String,
    pub judge_total_input_tokens: u64,
    pub judge_total_output_tokens: u64,
    /// Real USD cost = judge token totals × the resolved model's registry
    /// pricing. `None` (serialized as JSON `null`) when the model's price is
    /// unknown — an honest "unknown", never a fabricated $0.00. A *known* rate
    /// applied to zero tokens is `Some(0.0)`, which is a real (not fake) zero.
    pub judge_total_cost_usd: Option<f64>,
    pub hybrid_coverage_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeManifest {
    pub since: String,
    pub until: String,
    pub resolved_since_sha: Option<String>,
    pub resolved_until_sha: Option<String>,
}

pub fn write(path: &std::path::Path, m: &Manifest) -> std::io::Result<()> {
    if let Some(p) = path.parent()
        && !p.as_os_str().is_empty()
    {
        std::fs::create_dir_all(p)?;
    }
    let j = serde_json::to_string_pretty(m)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, j)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_round_trips() {
        let m = Manifest {
            schema_version: "1.0".into(),
            run_id: "01HW7".into(),
            run_started: chrono::Utc::now(),
            run_completed: chrono::Utc::now(),
            vox_version: env!("CARGO_PKG_VERSION").into(),
            effort_audit_crate_version: "0.1.0".into(),
            range: RangeManifest {
                since: "30 days ago".into(),
                until: "HEAD".into(),
                resolved_since_sha: None,
                resolved_until_sha: None,
            },
            commits_in_range: 10,
            commits_judged: 10,
            commits_skipped: 0,
            judge_model_id_resolved: "mock".into(),
            judge_total_input_tokens: 100,
            judge_total_output_tokens: 50,
            judge_total_cost_usd: Some(0.05),
            hybrid_coverage_percent: 30.0,
        };
        let tmp = tempfile::NamedTempFile::new().unwrap();
        write(tmp.path(), &m).unwrap();
        let back: Manifest =
            serde_json::from_str(&std::fs::read_to_string(tmp.path()).unwrap()).unwrap();
        assert_eq!(back.run_id, "01HW7");
    }
}
