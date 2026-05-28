//! OpenReview API v2 client (Bearer token from `/login`, matching openreview-py).

use std::time::Duration;

use async_trait::async_trait;

use super::ScholarlyRemoteStatus;
use super::ScholarlySubmissionReceipt;
use super::error::{ScholarlyError, classify_scholarly_http};
use super::flags;
use crate::openreview_api_types::{
    ManifestMetadataOpenReviewRoot, OpenReviewAuthorName, OpenReviewField, OpenReviewLoginRequest,
    OpenReviewLoginResponse, OpenReviewNoteContent, OpenReviewNoteEditRequest,
    OpenReviewNoteEditResponse, OpenReviewNotesListResponse,
};
use crate::publication::PublicationManifest;

const DEFAULT_API_BASE: &str = "https://api2.openreview.net";

#[must_use]
fn openreview_http_max_attempts() -> u32 {
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOpenReviewHttpMaxAttempts)
        .expose()
        .and_then(|s| s.trim().parse().ok())
        .filter(|&n| (1..=10).contains(&n))
        .unwrap_or(3)
}

async fn sleep_before_openreview_retry(err: &ScholarlyError, zero_based_attempt: u32) {
    let ms: u64 = match err {
        ScholarlyError::RateLimited {
            retry_after_secs: Some(s),
            ..
        } => (*s).saturating_mul(1000).max(500),
        _ => {
            let base = 350u64.saturating_mul(2u64.pow(zero_based_attempt));
            base.min(8_000)
        }
    };
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

#[derive(Debug, Clone)]
struct OpenReviewConfig {
    invitation: String,
    signature: String,
    readers: Vec<String>,
}

fn merge_openreview_config(
    manifest: &PublicationManifest,
) -> Result<OpenReviewConfig, ScholarlyError> {
    let mut invitation =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOpenReviewInvitation)
            .expose()
            .unwrap_or_default()
            .to_string();
    let mut signature = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOpenReviewSignature)
        .expose()
        .unwrap_or_default()
        .to_string();
    let mut readers: Option<Vec<String>> = None;

    if let Some(meta) = manifest.metadata_json.as_deref()
        && let Ok(root) = serde_json::from_str::<ManifestMetadataOpenReviewRoot>(meta)
        && let Some(or) = root.openreview
    {
        if invitation.is_empty()
            && let Some(s) = or
                .invitation
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
        {
            invitation = s.to_string();
        }
        if signature.is_empty()
            && let Some(s) = or
                .signature
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
        {
            signature = s.to_string();
        }
        if let Some(r) = or.readers.filter(|x| !x.is_empty()) {
            let r: Vec<String> = r
                .into_iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !r.is_empty() {
                readers = Some(r);
            }
        }
    }

    if invitation.is_empty() {
        return Err(ScholarlyError::Config {
            message: "OpenReview submit requires invitation id (VOX_OPENREVIEW_INVITATION or metadata_json.openreview.invitation)"
                .into(),
        });
    }
    if signature.is_empty() {
        return Err(ScholarlyError::Config {
            message: "OpenReview submit requires tilde signature (VOX_OPENREVIEW_SIGNATURE or metadata_json.openreview.signature)"
                .into(),
        });
    }

    let readers = readers.unwrap_or_else(|| vec!["everyone".to_string()]);
    Ok(OpenReviewConfig {
        invitation,
        signature,
        readers,
    })
}

/// Resolved OpenReview `notes/edits` profile (stdout-only helper for operators and CI).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenReviewSubmitProfileExport {
    pub schema_version: i32,
    pub invitation: String,
    pub signature: String,
    pub readers: Vec<String>,
    pub api_base: String,
}

/// Merge `VOX_OPENREVIEW_*` / `OPENREVIEW_*` with `metadata_json.openreview` (same rules as submit).
pub fn export_openreview_submit_profile(
    manifest: &PublicationManifest,
) -> Result<OpenReviewSubmitProfileExport, ScholarlyError> {
    let cfg = merge_openreview_config(manifest)?;
    Ok(OpenReviewSubmitProfileExport {
        schema_version: 1,
        invitation: cfg.invitation,
        signature: cfg.signature,
        readers: cfg.readers,
        api_base: api_base(),
    })
}

fn api_base() -> String {
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOpenReviewApiBase)
        .expose()
        .unwrap_or(DEFAULT_API_BASE)
        .to_string()
}

async fn login_bearer(
    http: &reqwest::Client,
    base: &str,
    username: &str,
    password: &str,
) -> Result<String, ScholarlyError> {
    let url = format!("{}/login", base.trim_end_matches('/'));
    let login_body = OpenReviewLoginRequest {
        id: username.to_string(),
        password: password.to_string(),
    };
    let resp = http
        .post(url)
        .header("Content-Type", "application/json")
        .json(&login_body)
        .send()
        .await?;
    let status = resp.status().as_u16();
    let text = resp.text().await.unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(classify_scholarly_http(status, &text));
    }
    let v: OpenReviewLoginResponse =
        serde_json::from_str(&text).map_err(|e| ScholarlyError::Fatal {
            code: "openreview_json".into(),
            message: format!("login JSON: {e}; body={text}"),
        })?;
    if v.mfa_pending {
        return Err(ScholarlyError::Config {
            message: "OpenReview MFA required; set OPENREVIEW_ACCESS_TOKEN (Bearer JWT from login) instead of password login"
                .into(),
        });
    }
    let token = v.token.ok_or_else(|| ScholarlyError::Fatal {
        code: "openreview_login_token".into(),
        message: format!("login response missing token: {text}"),
    })?;
    Ok(token)
}

async fn resolve_bearer_async(
    http: &reqwest::Client,
    base: &str,
) -> Result<String, ScholarlyError> {
    let token_res = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOpenReviewAccessToken);
    if let Some(t) = token_res.expose() {
        return Ok(t.to_string());
    }
    let email_res = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOpenReviewEmail);
    let email = email_res.expose();
    let password_res = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOpenReviewPassword);
    let password = password_res.expose();
    let (Some(email), Some(password)) = (email, password) else {
        return Err(ScholarlyError::Config {
            message: "OpenReview auth: set OPENREVIEW_ACCESS_TOKEN or OPENREVIEW_EMAIL + OPENREVIEW_PASSWORD (or Clavis equivalents)"
                .into(),
        });
    };
    login_bearer(http, base, email, password).await
}

#[derive(Debug, Clone)]
pub(super) struct OpenReviewHttpClient {
    base: String,
    bearer: String,
    http: reqwest::Client,
}

impl OpenReviewHttpClient {
    pub(super) async fn new_authenticated(base: String) -> Result<Self, ScholarlyError> {
        let http = vox_http_client::client_builder()
            .user_agent("vox-publisher/scholarly-openreview")
            .build()
            .map_err(|e| ScholarlyError::Config {
                message: format!("http client: {e}"),
            })?;
        let bearer = resolve_bearer_async(&http, &base).await?;
        Ok(Self { base, bearer, http })
    }

    fn notes_url(&self) -> String {
        format!("{}/notes", self.base.trim_end_matches('/'))
    }

    fn note_edits_url(&self) -> String {
        format!("{}/notes/edits", self.base.trim_end_matches('/'))
    }

    async fn get_note_once(
        &self,
        note_id: &str,
    ) -> Result<OpenReviewNotesListResponse, ScholarlyError> {
        let url = self.notes_url();
        let resp = self
            .http
            .get(url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", self.bearer),
            )
            .query(&[("id", note_id)])
            .send()
            .await?;
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        if !(200..300).contains(&status) {
            return Err(classify_scholarly_http(status, &text));
        }
        serde_json::from_str(&text).map_err(|e| ScholarlyError::Fatal {
            code: "openreview_json_parse".into(),
            message: format!("notes GET: {e}; body={text}"),
        })
    }

    pub(super) async fn get_note(
        &self,
        note_id: &str,
    ) -> Result<OpenReviewNotesListResponse, ScholarlyError> {
        let max = openreview_http_max_attempts();
        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            match self.get_note_once(note_id).await {
                Ok(v) => return Ok(v),
                Err(e) if e.retryable() && attempt < max => {
                    sleep_before_openreview_retry(&e, attempt.saturating_sub(1)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn post_note_edit_once(
        &self,
        body: &OpenReviewNoteEditRequest,
    ) -> Result<OpenReviewNoteEditResponse, ScholarlyError> {
        let url = self.note_edits_url();
        let resp = self
            .http
            .post(url)
            .header("Content-Type", "application/json")
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", self.bearer),
            )
            .json(body)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        if !(200..300).contains(&status) {
            return Err(classify_scholarly_http(status, &text));
        }
        serde_json::from_str(&text).map_err(|e| ScholarlyError::Fatal {
            code: "openreview_json_parse".into(),
            message: format!("notes/edits POST: {e}; body={text}"),
        })
    }

    pub(super) async fn post_note_edit(
        &self,
        body: &OpenReviewNoteEditRequest,
    ) -> Result<OpenReviewNoteEditResponse, ScholarlyError> {
        let max = openreview_http_max_attempts();
        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            match self.post_note_edit_once(body).await {
                Ok(v) => return Ok(v),
                Err(e) if e.retryable() && attempt < max => {
                    sleep_before_openreview_retry(&e, attempt.saturating_sub(1)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

fn manifest_content_for_openreview(manifest: &PublicationManifest) -> OpenReviewNoteContent {
    let description = manifest
        .abstract_text
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(std::string::ToString::to_string)
        .unwrap_or_else(|| manifest.body_markdown.trim().to_string());
    OpenReviewNoteContent {
        title: OpenReviewField {
            value: manifest.title.clone(),
        },
        abstract_: OpenReviewField { value: description },
        authors: OpenReviewField {
            value: vec![OpenReviewAuthorName {
                name: manifest.author.clone(),
            }],
        },
    }
}

#[derive(Debug, Clone)]
pub(super) struct OpenReviewAdapter {
    client: OpenReviewHttpClient,
}

impl OpenReviewAdapter {
    pub(super) async fn new_from_env() -> Result<Self, ScholarlyError> {
        if flags::adapter_live_disabled("openreview") {
            return Err(ScholarlyError::Disabled {
                reason: "VOX_SCHOLARLY_DISABLE_OPENREVIEW is set".into(),
            });
        }
        let client = OpenReviewHttpClient::new_authenticated(api_base()).await?;
        Ok(Self { client })
    }
}

#[async_trait]
impl super::ScholarlyAdapter for OpenReviewAdapter {
    fn adapter_name(&self) -> &'static str {
        "openreview"
    }

    async fn submit(
        &self,
        manifest: &PublicationManifest,
    ) -> Result<ScholarlySubmissionReceipt, ScholarlyError> {
        let cfg = merge_openreview_config(manifest)?;
        let writers = vec![cfg.signature.clone()];
        let body = OpenReviewNoteEditRequest::scholarly_submit(
            cfg.invitation,
            vec![cfg.signature.clone()],
            cfg.readers,
            writers,
            manifest_content_for_openreview(manifest),
        );
        let v = self.client.post_note_edit(&body).await?;
        let note_id = v
            .extract_note_id()
            .ok_or_else(|| ScholarlyError::Fatal {
                code: "openreview_missing_note_id".into(),
                message: format!(
                    "note edit response missing note id: {}",
                    serde_json::to_string(&v).unwrap_or_default()
                ),
            })?
            .to_string();
        let digest = manifest.content_sha3_256();
        let meta_json = serde_json::to_string(&v).map_err(|e| ScholarlyError::Fatal {
            code: "openreview_receipt_encode".into(),
            message: format!("serialize note edit response: {e}"),
        })?;
        Ok(ScholarlySubmissionReceipt {
            adapter: self.adapter_name().to_string(),
            external_submission_id: note_id,
            status: "submitted".into(),
            response_fingerprint: Some(digest),
            metadata_json: Some(meta_json),
        })
    }

    async fn fetch_status(
        &self,
        external_submission_id: &str,
    ) -> Result<ScholarlyRemoteStatus, ScholarlyError> {
        let list = self.client.get_note(external_submission_id).await?;
        if list.notes.is_empty() {
            return Err(ScholarlyError::Fatal {
                code: "openreview_note_missing".into(),
                message: format!("no note for id {external_submission_id}"),
            });
        }
        let status = list.first_status();
        let detail = list
            .first_note_json()
            .ok_or_else(|| ScholarlyError::Fatal {
                code: "openreview_note_encode".into(),
                message: "failed to serialize note detail".into(),
            })?;
        Ok(ScholarlyRemoteStatus {
            status,
            detail_json: Some(detail),
        })
    }
}

pub(super) async fn openreview_adapter_from_env() -> Result<OpenReviewAdapter, ScholarlyError> {
    OpenReviewAdapter::new_from_env().await
}

#[cfg(test)]
mod profile_export_tests {
    use super::export_openreview_submit_profile;
    use crate::publication::PublicationManifest;

    #[test]
    fn export_reads_metadata_openreview_overlay() {
        let manifest = PublicationManifest {
            publication_id: "p1".into(),
            content_type: "scientia".into(),
            source_ref: None,
            title: "T".into(),
            author: "A".into(),
            abstract_text: None,
            body_markdown: "x".into(),
            citations_json: None,
            metadata_json: Some(
                r#"{"openreview":{"invitation":"TestVenue/2024/Conference/-/Submission","signature":"TestVenue/2024/Conference"}}"#.into(),
            ),
        };
        let p = export_openreview_submit_profile(&manifest).unwrap();
        assert_eq!(p.schema_version, 1);
        assert_eq!(p.invitation, "TestVenue/2024/Conference/-/Submission");
        assert_eq!(p.signature, "TestVenue/2024/Conference");
        assert_eq!(p.readers, vec!["everyone".to_string()]);
        assert!(p.api_base.contains("openreview"));
    }
}
