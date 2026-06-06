use sha3::{Digest, Sha3_256};

use crate::types::UnifiedNewsItem;

/// Canonical content manifest reused across community and scholarly publishing flows.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PublicationManifest {
    pub publication_id: String,
    pub content_type: String,
    pub source_ref: Option<String>,
    pub title: String,
    pub author: String,
    pub abstract_text: Option<String>,
    pub body_markdown: String,
    pub citations_json: Option<String>,
    pub metadata_json: Option<String>,
}

impl PublicationManifest {
    #[must_use]
    pub fn content_sha3_256(&self) -> String {
        let canonical = serde_json::json!({
            "publication_id": self.publication_id,
            "content_type": self.content_type,
            "source_ref": self.source_ref,
            "title": self.title,
            "author": self.author,
            "abstract_text": self.abstract_text,
            "body_markdown": self.body_markdown,
            "citations_json": self.citations_json,
            "metadata_json": self.metadata_json,
        });
        let mut hasher = Sha3_256::new();
        hasher.update(canonical.to_string().as_bytes());
        hex::encode(hasher.finalize())
    }
}

impl From<UnifiedNewsItem> for PublicationManifest {
    fn from(value: UnifiedNewsItem) -> Self {
        Self {
            publication_id: value.id,
            content_type: "news".to_string(),
            source_ref: None,
            title: value.title,
            author: value.author,
            abstract_text: None,
            body_markdown: value.content_markdown,
            citations_json: None,
            metadata_json: Some(
                serde_json::json!({
                    "tags": value.tags,
                    "syndication": value.syndication,
                    "topic_pack": value.topic_pack,
                })
                .to_string(),
            ),
        }
    }
}
