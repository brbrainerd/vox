//! A `Fragment` is the universal comparable unit: text + a blake3 content hash +
//! a similarity `Signature` + provenance (`source_ref`).

use serde::{Deserialize, Serialize};

use crate::signature::Signature;

/// What kind of thing a fragment represents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FragmentKind {
    Code,
    Prompt,
    InstalledSkill,
    McpTool,
    ExternalSkill,
}

/// A normalized comparable unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fragment {
    pub id: String,
    pub kind: FragmentKind,
    /// blake3 hex of the raw text (exact-duplicate key).
    pub content_hash: String,
    pub signature: Signature,
    /// Provenance: "path:line", skill id, or registry id.
    pub source_ref: String,
    pub text: String,
}

impl Fragment {
    pub fn new(
        id: impl Into<String>,
        kind: FragmentKind,
        text: impl Into<String>,
        source_ref: impl Into<String>,
        shingle_k: usize,
        num_hashes: usize,
    ) -> Self {
        let text = text.into();
        let content_hash = blake3::hash(text.as_bytes()).to_hex().to_string();
        let signature = Signature::from_text(&text, shingle_k, num_hashes);
        Fragment {
            id: id.into(),
            kind,
            content_hash,
            signature,
            source_ref: source_ref.into(),
            text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragment_hashes_and_signs_text() {
        let f = Fragment::new("f1", FragmentKind::Code, "let y = 2", "a.vox:1", 2, 16);
        assert_eq!(f.content_hash.len(), 64); // blake3 hex
        assert_eq!(f.signature.minhash.len(), 16);
        assert_eq!(f.kind, FragmentKind::Code);
    }

    #[test]
    fn identical_text_same_content_hash() {
        let a = Fragment::new("a", FragmentKind::Code, "same body", "x:1", 2, 8);
        let b = Fragment::new("b", FragmentKind::Code, "same body", "y:9", 2, 8);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
