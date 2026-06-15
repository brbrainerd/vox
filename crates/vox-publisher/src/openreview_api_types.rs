//! Typed JSON for OpenReview API v2 (subset used by the scholarly adapter).

use serde::{Deserialize, Serialize};

/// Optional `metadata_json.openreview` overlay for invitations/signatures/readers.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct OpenReviewMetadataOverlay {
    pub invitation: Option<String>,
    pub signature: Option<String>,
    pub readers: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct ManifestMetadataOpenReviewRoot {
    #[serde(default)]
    pub openreview: Option<OpenReviewMetadataOverlay>,
}

/// Login `POST /login`.
#[derive(Debug, Clone, Serialize)]
pub struct OpenReviewLoginRequest {
    pub id: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenReviewLoginResponse {
    #[serde(rename = "mfaPending", default)]
    pub mfa_pending: bool,
    #[serde(default)]
    pub token: Option<String>,
}

/// OpenReview v2 field wrapper (`{ "value": ... }`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenReviewField<T> {
    pub value: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenReviewAuthorName {
    pub name: String,
}

/// `content` object for a new note / edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenReviewNoteContent {
    pub title: OpenReviewField<String>,
    #[serde(rename = "abstract")]
    pub abstract_: OpenReviewField<String>,
    pub authors: OpenReviewField<Vec<OpenReviewAuthorName>>,
}

/// `POST /notes/edits` body for scholarly submit.
#[derive(Debug, Clone, Serialize)]
pub struct OpenReviewNoteEditRequest {
    pub invitation: String,
    pub signatures: Vec<String>,
    pub readers: Vec<String>,
    pub writers: Vec<String>,
    pub note: serde_json::Value,
    pub content: OpenReviewNoteContent,
}

impl OpenReviewNoteEditRequest {
    #[must_use]
    pub fn scholarly_submit(
        invitation: String,
        signatures: Vec<String>,
        readers: Vec<String>,
        writers: Vec<String>,
        content: OpenReviewNoteContent,
    ) -> Self {
        Self {
            invitation,
            signatures,
            readers,
            writers,
            note: serde_json::json!({}),
            content,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenReviewNoteIdOnly {
    pub id: String,
}

/// Response from `POST /notes/edits` (shape varies; we only need a note id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenReviewNoteEditResponse {
    #[serde(default)]
    pub note: Option<OpenReviewNoteIdOnly>,
    #[serde(default)]
    pub id: Option<String>,
}

impl OpenReviewNoteEditResponse {
    #[must_use]
    pub fn extract_note_id(&self) -> Option<&str> {
        self.note
            .as_ref()
            .map(|n| n.id.as_str())
            .or(self.id.as_deref())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenReviewNote {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

/// `GET /notes?id=...` response.
#[derive(Debug, Clone, Deserialize)]
pub struct OpenReviewNotesListResponse {
    #[serde(default)]
    pub notes: Vec<OpenReviewNote>,
}

impl OpenReviewNotesListResponse {
    #[must_use]
    pub fn first_status(&self) -> String {
        self.notes
            .first()
            .and_then(|n| {
                n.state
                    .as_deref()
                    .filter(|s| !s.trim().is_empty())
                    .or(n.status.as_deref().filter(|s| !s.trim().is_empty()))
            })
            .map(std::string::ToString::to_string)
            .unwrap_or_else(|| "unknown".to_string())
    }

    #[must_use]
    pub fn first_note_json(&self) -> Option<String> {
        self.notes
            .first()
            .and_then(|n| serde_json::to_string(n).ok())
    }
}
