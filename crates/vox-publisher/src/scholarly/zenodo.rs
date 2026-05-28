//! Minimal Zenodo REST client (deposit draft creation).

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use sha3::Sha3_256;
use sha3::digest::Digest;

use super::ScholarlyRemoteStatus;
use super::ScholarlySubmissionReceipt;
use super::error::{ScholarlyError, classify_scholarly_http};
use super::flags;
use crate::publication::PublicationManifest;
use crate::submission::{self, ScholarlyVenue};
use crate::zenodo_api_types::{ZenodoDeposition, ZenodoDepositionCreateBody};
use crate::zenodo_metadata;

const ZENODO_API_PRODUCTION: &str = "https://zenodo.org/api";
const ZENODO_API_SANDBOX: &str = "https://sandbox.zenodo.org/api";

#[must_use]
fn zenodo_http_max_attempts() -> u32 {
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxZenodoHttpMaxAttempts)
        .expose()
        .and_then(|s| s.trim().parse().ok())
        .filter(|&n| (1..=10).contains(&n))
        .unwrap_or(3)
}

async fn sleep_before_zenodo_retry(err: &ScholarlyError, zero_based_attempt: u32) {
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
pub(super) struct ZenodoHttpClient {
    base: String,
    token: String,
    http: reqwest::Client,
}

impl ZenodoHttpClient {
    pub(super) fn new(sandbox: bool, token: String) -> Result<Self, ScholarlyError> {
        let t = token.trim();
        if t.is_empty() {
            return Err(ScholarlyError::Config {
                message: "Zenodo access token is empty".into(),
            });
        }
        let base = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxZenodoApiBase)
            .expose()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                if sandbox {
                    ZENODO_API_SANDBOX.to_string()
                } else {
                    ZENODO_API_PRODUCTION.to_string()
                }
            });
        Ok(Self {
            base,
            token: t.to_string(),
            http: vox_http_client::client_builder()
                .user_agent("vox-publisher/submission")
                .build()
                .map_err(|e| ScholarlyError::Config {
                    message: format!("http client: {e}"),
                })?,
        })
    }

    fn url_depositions(&self) -> String {
        format!("{}/deposit/depositions", self.base.trim_end_matches('/'))
    }

    fn url_deposition(&self, id: &str) -> String {
        format!(
            "{}/deposit/depositions/{id}",
            self.base.trim_end_matches('/')
        )
    }

    /// URL for issuing a new version of an existing Zenodo deposition.
    /// Currently only exercised by `tests::url_new_version_is_correct` until
    /// the multi-version deposition flow is wired into the publisher service.
    #[cfg_attr(not(test), allow(dead_code))]
    fn url_new_version(&self, id: &str) -> String {
        format!(
            "{}/deposit/depositions/{id}/actions/newversion",
            self.base.trim_end_matches('/')
        )
    }

    async fn create_deposition_draft_once(
        &self,
        body: &ZenodoDepositionCreateBody,
    ) -> Result<ZenodoDeposition, ScholarlyError> {
        let url = self.url_depositions();
        let resp = self
            .http
            .post(url)
            .header("Content-Type", "application/json")
            .bearer_auth(&self.token)
            .json(body)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        if !(200..300).contains(&status) {
            return Err(classify_scholarly_http(status, &text));
        }
        serde_json::from_str(&text).map_err(|e| ScholarlyError::Fatal {
            code: "zenodo_json_parse".into(),
            message: format!("invalid JSON from Zenodo: {e}; body={text}"),
        })
    }

    pub(super) async fn create_deposition_draft(
        &self,
        body: &ZenodoDepositionCreateBody,
    ) -> Result<ZenodoDeposition, ScholarlyError> {
        let max = zenodo_http_max_attempts();
        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            match self.create_deposition_draft_once(body).await {
                Ok(v) => return Ok(v),
                Err(e) if e.retryable() && attempt < max => {
                    sleep_before_zenodo_retry(&e, attempt.saturating_sub(1)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn get_deposition_once(
        &self,
        deposition_id: &str,
    ) -> Result<ZenodoDeposition, ScholarlyError> {
        let url = self.url_deposition(deposition_id);
        let resp = self.http.get(url).bearer_auth(&self.token).send().await?;
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        if !(200..300).contains(&status) {
            return Err(classify_scholarly_http(status, &text));
        }
        serde_json::from_str(&text).map_err(|e| ScholarlyError::Fatal {
            code: "zenodo_json_parse".into(),
            message: format!("invalid JSON from Zenodo: {e}; body={text}"),
        })
    }

    pub(super) async fn get_deposition(
        &self,
        deposition_id: &str,
    ) -> Result<ZenodoDeposition, ScholarlyError> {
        let max = zenodo_http_max_attempts();
        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            match self.get_deposition_once(deposition_id).await {
                Ok(v) => return Ok(v),
                Err(e) if e.retryable() && attempt < max => {
                    sleep_before_zenodo_retry(&e, attempt.saturating_sub(1)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn bucket_url_from_deposition(v: &ZenodoDeposition) -> Result<String, ScholarlyError> {
        v.links
            .as_ref()
            .and_then(|l| l.bucket.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string)
            .ok_or_else(|| ScholarlyError::Fatal {
                code: "zenodo_missing_bucket".into(),
                message: format!(
                    "deposit response missing links.bucket: {}",
                    serde_json::to_string(v).unwrap_or_default()
                ),
            })
    }

    async fn put_bucket_object_once(
        &self,
        bucket_url: &str,
        object_name: &str,
        data: &[u8],
        content_type: &str,
    ) -> Result<(), ScholarlyError> {
        let name = object_name.trim_start_matches('/');
        if name.is_empty() {
            return Err(ScholarlyError::Config {
                message: "Zenodo bucket object name must not be empty".into(),
            });
        }
        let url = format!("{}/{}", bucket_url.trim_end_matches('/'), name);
        let resp = self
            .http
            .put(url)
            .header("Content-Type", content_type)
            .bearer_auth(&self.token)
            .body(data.to_vec())
            .send()
            .await?;
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        if !(200..300).contains(&status) {
            return Err(classify_scholarly_http(status, &text));
        }
        Ok(())
    }

    pub(super) async fn put_bucket_object(
        &self,
        bucket_url: &str,
        object_name: &str,
        data: &[u8],
        content_type: &str,
    ) -> Result<(), ScholarlyError> {
        let max = zenodo_http_max_attempts();
        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            match self
                .put_bucket_object_once(bucket_url, object_name, data, content_type)
                .await
            {
                Ok(()) => return Ok(()),
                Err(e) if e.retryable() && attempt < max => {
                    sleep_before_zenodo_retry(&e, attempt.saturating_sub(1)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn publish_deposition_once(
        &self,
        deposition_id: &str,
    ) -> Result<ZenodoDeposition, ScholarlyError> {
        let url = format!(
            "{}/deposit/depositions/{deposition_id}/actions/publish",
            self.base.trim_end_matches('/')
        );
        let resp = self.http.post(url).bearer_auth(&self.token).send().await?;
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        if !(200..300).contains(&status) {
            return Err(classify_scholarly_http(status, &text));
        }
        serde_json::from_str(&text).map_err(|e| ScholarlyError::Fatal {
            code: "zenodo_json_parse".into(),
            message: format!("publish response JSON: {e}; body={text}"),
        })
    }

    pub(super) async fn publish_deposition(
        &self,
        deposition_id: &str,
    ) -> Result<ZenodoDeposition, ScholarlyError> {
        let max = zenodo_http_max_attempts();
        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            match self.publish_deposition_once(deposition_id).await {
                Ok(v) => return Ok(v),
                Err(e) if e.retryable() && attempt < max => {
                    sleep_before_zenodo_retry(&e, attempt.saturating_sub(1)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

fn zenodo_staging_content_type(rel: &str) -> &'static str {
    let n = rel.to_ascii_lowercase();
    if n.ends_with(".md") {
        return "text/markdown; charset=utf-8";
    }
    if n.ends_with(".json") {
        return "application/json; charset=utf-8";
    }
    if n.ends_with(".cff") {
        return "application/yaml; charset=utf-8";
    }
    if n.ends_with(".tex") {
        return "application/x-tex; charset=utf-8";
    }
    "application/octet-stream"
}

fn zenodo_verify_title_parity(
    manifest: &PublicationManifest,
    root: &Path,
) -> Result<(), ScholarlyError> {
    let p = root.join("zenodo.json");
    let raw = std::fs::read_to_string(&p).map_err(|e| ScholarlyError::Config {
        message: format!(
            "read {} for metadata parity: {e} — hint: run `vox scientia publication-scholarly-staging-export`",
            p.display()
        ),
    })?;
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| ScholarlyError::Config {
        message: format!(
            "parse zenodo.json for parity: {e} — hint: regenerate staging zenodo.json from manifest"
        ),
    })?;
    let file_title = v
        .get("metadata")
        .and_then(|m| m.get("title"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim();
    let mt = manifest.title.trim();
    if file_title != mt {
        return Err(ScholarlyError::Config {
            message: format!(
                "zenodo.json metadata.title ({file_title:?}) != manifest.title ({mt:?}); fix staging or unset VOX_ZENODO_REQUIRE_METADATA_PARITY"
            ),
        });
    }
    Ok(())
}

fn zenodo_load_staging_sha_map(root: &Path) -> Result<HashMap<String, String>, ScholarlyError> {
    let p = root.join("staging_checksums.json");
    let raw = std::fs::read_to_string(&p).map_err(|e| ScholarlyError::Config {
        message: format!(
            "read staging_checksums.json: {e} — hint: export staging with current vox-publisher (writes checksum manifest) or unset VOX_ZENODO_VERIFY_STAGING_CHECKSUMS"
        ),
    })?;
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| ScholarlyError::Config {
        message: format!("parse staging_checksums.json: {e}"),
    })?;
    let obj = v
        .get("sha3_256")
        .or_else(|| v.get("sha256"))
        .and_then(|s| s.as_object())
        .ok_or_else(|| ScholarlyError::Config {
            message: "staging_checksums.json missing top-level sha3_256 (or legacy sha256) object"
                .into(),
        })?;
    let mut m = HashMap::new();
    for (k, val) in obj {
        if let Some(s) = val.as_str() {
            m.insert(k.clone(), s.trim().to_ascii_lowercase());
        }
    }
    Ok(m)
}

fn zenodo_relpaths_to_upload(root: &Path) -> Result<Vec<String>, ScholarlyError> {
    let allow = flags::zenodo_upload_allowlist();
    let plan: Vec<String> = submission::staging_artifacts(ScholarlyVenue::Zenodo)
        .into_iter()
        .map(|a| a.relative_path)
        .filter(|r| r != "arxiv_bundle.tar.gz" && r != "arxiv_handoff.json")
        .collect();
    let candidates: Vec<String> = if allow.is_empty() { plan } else { allow };
    let mut out = Vec::new();
    for rel in candidates {
        let p = root.join(&rel);
        if rel == "citations.json" && !p.is_file() {
            continue;
        }
        if !p.is_file() {
            return Err(ScholarlyError::Config {
                message: format!(
                    "Zenodo staging upload: missing {rel} under {} — hint: run publication-scholarly-staging-export or narrow VOX_ZENODO_UPLOAD_ALLOWLIST",
                    root.display()
                ),
            });
        }
        out.push(rel);
    }
    if out.is_empty() {
        return Err(ScholarlyError::Config {
            message: "Zenodo staging upload: no files matched (empty tree or allowlist) — hint: set VOX_ZENODO_STAGING_DIR to export root"
                .into(),
        });
    }
    Ok(out)
}

async fn zenodo_upload_staging_files(
    client: &ZenodoHttpClient,
    bucket: &str,
    root: &Path,
    rels: &[String],
    sha_expected: Option<&HashMap<String, String>>,
) -> Result<(), ScholarlyError> {
    for rel in rels {
        let p = root.join(rel);
        let bytes = std::fs::read(&p).map_err(|e| ScholarlyError::Fatal {
            code: "zenodo_staging_read".into(),
            message: format!("{rel}: {e}"),
        })?;
        if let Some(map) = sha_expected
            && let Some(hex) = map.get(rel)
        {
            let d = Sha3_256::digest(&bytes);
            let got = format!("{d:x}");
            if got != *hex {
                return Err(ScholarlyError::Config {
                    message: format!(
                        "SHA-256 mismatch for {rel}: expected {hex}, got {got}; re-run staging export"
                    ),
                });
            }
        }
        let ct = zenodo_staging_content_type(rel);
        client.put_bucket_object(bucket, rel, &bytes, ct).await?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub(super) struct ZenodoAdapter {
    client: ZenodoHttpClient,
}

impl ZenodoAdapter {
    pub(super) fn from_token(sandbox: bool, token: String) -> Result<Self, ScholarlyError> {
        Ok(Self {
            client: ZenodoHttpClient::new(sandbox, token)?,
        })
    }
}

#[async_trait]
impl super::ScholarlyAdapter for ZenodoAdapter {
    fn adapter_name(&self) -> &'static str {
        "zenodo"
    }

    async fn submit(
        &self,
        manifest: &PublicationManifest,
    ) -> Result<ScholarlySubmissionReceipt, ScholarlyError> {
        let mut attach = flags::zenodo_attach_manifest_body();
        let mut publish = flags::zenodo_publish_deposition();
        if flags::zenodo_publish_now_profile() {
            attach = true;
            publish = true;
        }
        if flags::zenodo_draft_only() {
            publish = false;
        }

        let staging_root = flags::zenodo_staging_dir();

        let body = zenodo_metadata::zenodo_deposition_create_body(manifest);
        let mut dep = self.client.create_deposition_draft(&body).await?;
        let id = dep.id;
        let bucket = ZenodoHttpClient::bucket_url_from_deposition(&dep)?;

        if let Some(ref root) = staging_root {
            if flags::zenodo_require_metadata_title_parity() {
                zenodo_verify_title_parity(manifest, root)?;
            }
            let rels = zenodo_relpaths_to_upload(root)?;
            let sha_map = if flags::zenodo_verify_staging_checksums() {
                Some(zenodo_load_staging_sha_map(root)?)
            } else {
                None
            };
            zenodo_upload_staging_files(&self.client, &bucket, root, &rels, sha_map.as_ref())
                .await?;
        } else if attach {
            self.client
                .put_bucket_object(
                    &bucket,
                    "body.md",
                    manifest.body_markdown.as_bytes(),
                    "text/markdown; charset=utf-8",
                )
                .await?;
        }

        let uploaded_from_staging = staging_root.is_some();
        let effective_attach = attach || uploaded_from_staging;
        if publish && !effective_attach {
            return Err(ScholarlyError::Config {
                message: "Zenodo publish requires ≥1 file: set VOX_ZENODO_ATTACH_MANIFEST_BODY and/or VOX_ZENODO_STAGING_DIR — hint: draft-only uses VOX_ZENODO_DRAFT_ONLY=1"
                    .into(),
            });
        }
        if publish {
            dep = self.client.publish_deposition(&id.to_string()).await?;
        }
        let state = if dep.state.is_empty() {
            "draft".to_string()
        } else {
            dep.state.clone()
        };
        let digest = manifest.content_sha3_256();
        let mut dep_val = serde_json::to_value(&dep).map_err(|e| ScholarlyError::Fatal {
            code: "zenodo_receipt_encode".into(),
            message: format!("deposition as JSON value: {e}"),
        })?;
        if let Some(doi) = dep.doi.clone().filter(|s| !s.trim().is_empty())
            && let Some(m) = dep_val.as_object_mut()
        {
            m.insert("expected_doi_hint".into(), serde_json::Value::String(doi));
        }
        let meta_json = serde_json::to_string(&dep_val).map_err(|e| ScholarlyError::Fatal {
            code: "zenodo_receipt_encode".into(),
            message: format!("serialize deposition: {e}"),
        })?;
        Ok(ScholarlySubmissionReceipt {
            adapter: self.adapter_name().to_string(),
            external_submission_id: id.to_string(),
            status: state,
            response_fingerprint: Some(digest),
            metadata_json: Some(meta_json),
        })
    }

    async fn fetch_status(
        &self,
        external_submission_id: &str,
    ) -> Result<ScholarlyRemoteStatus, ScholarlyError> {
        let dep = self.client.get_deposition(external_submission_id).await?;
        let status = if dep.state.is_empty() {
            "unknown".to_string()
        } else {
            dep.state.clone()
        };
        let detail = serde_json::to_string(&dep).map_err(|e| ScholarlyError::Fatal {
            code: "zenodo_status_encode".into(),
            message: format!("serialize deposition: {e}"),
        })?;
        Ok(ScholarlyRemoteStatus {
            status,
            detail_json: Some(detail),
        })
    }
}

pub(super) fn zenodo_from_secrets() -> Result<ZenodoAdapter, ScholarlyError> {
    if flags::adapter_live_disabled("zenodo") {
        return Err(ScholarlyError::Disabled {
            reason: "VOX_SCHOLARLY_DISABLE_ZENODO is set".into(),
        });
    }
    let token = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxZenodoAccessToken)
        .expose()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let Some(token) = token else {
        return Err(ScholarlyError::Config {
            message: "missing Zenodo token: set ZENODO_ACCESS_TOKEN (or VOX_ZENODO_ACCESS_TOKEN) per secrets / `vox secrets doctor`"
                .into(),
        });
    };
    ZenodoAdapter::from_token(flags::zenodo_use_sandbox(), token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_new_version_is_correct() {
        let client = ZenodoHttpClient {
            base: "https://sandbox.zenodo.org/api".into(),
            token: "tok".into(),
            http: reqwest::Client::new(),
        };
        let url = client.url_new_version("12345");
        assert_eq!(
            url,
            "https://sandbox.zenodo.org/api/deposit/depositions/12345/actions/newversion"
        );
    }
}
