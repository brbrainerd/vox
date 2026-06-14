use crate::task::TaskKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshDirectoryEntry {
    pub scope_id: String,
    pub control_url: String,
    pub region_label: Option<String>,
    pub task_kinds: Vec<TaskKind>,
    pub public: bool,
    pub current_queue_depth: Option<usize>,
    pub supported_priorities: Option<Vec<u8>>,
    /// Optional Ed25519 signature of the canonical entry representation.
    pub signature: Option<Vec<u8>>,
    /// Ed25519 public key used to verify the signature.
    pub public_key: Option<[u8; 32]>,
}

impl MeshDirectoryEntry {
    /// Returns the canonical JSON representation for signing.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut clone = self.clone();
        clone.signature = None;
        clone.public_key = None;
        serde_json::to_vec(&clone).unwrap_or_default()
    }
}

#[cfg(test)]
mod semcov_wave25_tests {
    use super::*;
    use crate::task::TaskKind;

    fn make_entry(public: bool) -> MeshDirectoryEntry {
        MeshDirectoryEntry {
            scope_id: "scope-1".to_string(),
            control_url: "https://mesh.example.com/control".to_string(),
            region_label: Some("us-east".to_string()),
            task_kinds: vec![TaskKind::TextInfer],
            public,
            current_queue_depth: Some(3),
            supported_priorities: Some(vec![1, 2, 3]),
            signature: None,
            public_key: None,
        }
    }

    // Catches: canonical_bytes including signature/public_key in the signed payload,
    // meaning a node that signs the entry and then sets signature would produce an
    // unverifiable signature (circular dependency).
    #[test]
    fn canonical_bytes_excludes_signature_and_public_key() {
        let mut entry = make_entry(true);
        entry.signature = Some(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        entry.public_key = Some([1u8; 32]);

        let bytes = entry.canonical_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(
            json.get("signature").is_none(),
            "canonical_bytes must not include 'signature'"
        );
        assert!(
            json.get("public_key").is_none(),
            "canonical_bytes must not include 'public_key'"
        );
    }

    // Catches: canonical_bytes being non-deterministic between two calls with the same
    // entry (e.g., HashMap iteration order in the serializer — serde_json object is
    // insertion-ordered so this should be stable, but verify it).
    #[test]
    fn canonical_bytes_is_deterministic() {
        let entry = make_entry(true);
        let b1 = entry.canonical_bytes();
        let b2 = entry.canonical_bytes();
        assert_eq!(b1, b2, "canonical_bytes must be deterministic");
    }

    // Catches: MeshDirectoryEntry serde round-trip dropping optional fields like
    // region_label or supported_priorities.
    #[test]
    fn mesh_directory_entry_serde_round_trip() {
        let entry = make_entry(false);
        let json = serde_json::to_string(&entry).unwrap();
        let back: MeshDirectoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.scope_id, "scope-1");
        assert_eq!(back.region_label.as_deref(), Some("us-east"));
        assert_eq!(back.supported_priorities, Some(vec![1, 2, 3]));
        assert!(!back.public);
    }

    // Catches: canonical_bytes changing when only signature is set (entry content is the
    // same), which would invalidate previously issued signatures unnecessarily.
    #[test]
    fn canonical_bytes_stable_across_different_signature_values() {
        let base = make_entry(true);
        let base_bytes = base.canonical_bytes();

        let mut with_sig = base.clone();
        with_sig.signature = Some(vec![0xAB, 0xCD]);
        with_sig.public_key = Some([42u8; 32]);

        assert_eq!(
            with_sig.canonical_bytes(),
            base_bytes,
            "canonical_bytes must not change when only signature/public_key differ"
        );
    }

    // Catches: task_kinds being silently dropped during serde (e.g., untagged enum
    // failing to round-trip TaskKind variants).
    #[test]
    fn task_kinds_preserved_in_round_trip() {
        let mut entry = make_entry(true);
        entry.task_kinds = vec![TaskKind::TextInfer, TaskKind::TrainQLoRA];
        let json = serde_json::to_string(&entry).unwrap();
        let back: MeshDirectoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.task_kinds.len(), 2);
    }

    // Catches: current_queue_depth being required (not Option) in some code path,
    // causing deserialization failure for entries without it.
    #[test]
    fn entry_deserializes_without_optional_fields() {
        let json = r#"{
            "scope_id":"s","control_url":"u","task_kinds":[],"public":true
        }"#;
        let entry: MeshDirectoryEntry =
            serde_json::from_str(json).expect("entry must deserialize without optional fields");
        assert!(entry.current_queue_depth.is_none());
        assert!(entry.region_label.is_none());
        assert!(entry.signature.is_none());
    }
}
