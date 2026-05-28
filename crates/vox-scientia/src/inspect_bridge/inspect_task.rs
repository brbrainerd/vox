//! UK AISI Inspect task descriptor builder.
//!
//! Inspect tasks are JSON files consumed by the `inspect` CLI tool.
//! This module generates conformant descriptors from Vox measurement probes.
//! No Python runtime dependency — descriptors are plain JSON.

use serde::{Deserialize, Serialize};

/// A single sample (input/target pair) in an Inspect task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectSample {
    /// The probe or question presented to the model under evaluation.
    pub input: String,
    /// The expected answer or judgment rubric.
    pub target: String,
    /// Arbitrary extra fields (source ref, probe id, etc.).
    pub metadata: serde_json::Value,
}

/// A full UK AISI Inspect task descriptor.
///
/// Serialises to the JSON format expected by `inspect eval`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectTaskDescriptor {
    pub task_id: String,
    pub description: String,
    /// Semver string, e.g. `"1.0.0"`.
    pub version: String,
    pub samples: Vec<InspectSample>,
    /// Scorer id, e.g. `"exact_match"` or `"model_graded_qa"`.
    pub scorer: String,
    pub metadata: serde_json::Value,
}

impl InspectTaskDescriptor {
    /// Create a new descriptor with empty samples and sensible defaults.
    pub fn new(task_id: String, description: String) -> Self {
        Self {
            task_id,
            description,
            version: "1.0.0".to_string(),
            samples: Vec::new(),
            scorer: "model_graded_qa".to_string(),
            metadata: serde_json::Value::Object(Default::default()),
        }
    }

    /// Append a sample to the task.
    pub fn add_sample(&mut self, input: String, target: String, metadata: serde_json::Value) {
        self.samples.push(InspectSample {
            input,
            target,
            metadata,
        });
    }

    /// Serialise to an Inspect-compatible JSON value.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("InspectTaskDescriptor is always serialisable")
    }

    /// Return the number of samples in the task.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }
}

/// Convert a Vox measurement probe into an Inspect sample.
///
/// `probe_text` — the natural-language probe question.
/// `expected_behavior` — the judgment rubric or expected answer.
pub fn vox_probe_to_inspect_sample(probe_text: &str, expected_behavior: &str) -> InspectSample {
    InspectSample {
        input: probe_text.to_string(),
        target: expected_behavior.to_string(),
        metadata: serde_json::Value::Object(Default::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_task_has_no_samples() {
        let task = InspectTaskDescriptor::new("T-001".to_string(), "Test task".to_string());
        assert_eq!(task.sample_count(), 0);
    }

    #[test]
    fn add_sample_increments_count() {
        let mut task = InspectTaskDescriptor::new("T-002".to_string(), "Test task".to_string());
        task.add_sample(
            "What is 2+2?".to_string(),
            "4".to_string(),
            serde_json::Value::Null,
        );
        task.add_sample(
            "What is 3+3?".to_string(),
            "6".to_string(),
            serde_json::Value::Null,
        );
        assert_eq!(task.sample_count(), 2);
    }

    #[test]
    fn to_json_contains_task_id_and_samples() {
        let mut task =
            InspectTaskDescriptor::new("T-003".to_string(), "Novelty probe task".to_string());
        task.add_sample(
            "probe?".to_string(),
            "rubric".to_string(),
            serde_json::json!({}),
        );
        let json = task.to_json();
        assert_eq!(json["task_id"], "T-003");
        assert_eq!(json["samples"].as_array().unwrap().len(), 1);
        assert_eq!(json["samples"][0]["input"], "probe?");
    }
}
