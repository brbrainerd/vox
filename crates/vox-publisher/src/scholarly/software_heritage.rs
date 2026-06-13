//! Software Heritage "Save Code Now" client.
//!
//! Pure helpers (URL building, JSON parse, metadata merge) plus async
//! request/poll functions that require a `reqwest::Client`.

use std::time::Duration;

use anyhow::{Context, Result};

const SWH_API_BASE: &str = "https://archive.softwareheritage.org/api/1";

/// Build the Save Code Now endpoint URL for `origin_url`.
///
/// Per SWH docs the origin URL is embedded directly in the path (not
/// percent-encoded) and the path must end with a trailing slash.
#[must_use]
pub fn save_code_now_url(origin_url: &str) -> String {
    format!(
        "{}/origin/save/git/url/{}/",
        SWH_API_BASE,
        origin_url.trim_end_matches('/')
    )
}

/// Snapshot of a Software Heritage save request/task status.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SaveStatus {
    pub request_status: String,
    pub task_status: String,
    pub snapshot_swhid: Option<String>,
}

/// Parse a Software Heritage save-status JSON body into [`SaveStatus`].
///
/// Maps `save_request_status` → `request_status`, `save_task_status` →
/// `task_status`, `snapshot_swhid` → `snapshot_swhid`.
pub fn parse_save_status(body: &str) -> Result<SaveStatus> {
    let v: serde_json::Value =
        serde_json::from_str(body).context("parse Software Heritage save status JSON")?;
    let request_status = v
        .get("save_request_status")
        .and_then(|x| x.as_str())
        .unwrap_or("unknown")
        .to_string();
    let task_status = v
        .get("save_task_status")
        .and_then(|x| x.as_str())
        .unwrap_or("unknown")
        .to_string();
    let snapshot_swhid = v
        .get("snapshot_swhid")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Ok(SaveStatus {
        request_status,
        task_status,
        snapshot_swhid,
    })
}

/// POST a Save Code Now request for `origin_url`.
///
/// If `token` is provided it is sent as a Bearer token (raises SWH rate limit
/// from 120 to 1200 req/h). The response body is parsed into [`SaveStatus`].
pub async fn request_save(
    client: &reqwest::Client,
    origin_url: &str,
    token: Option<&str>,
) -> Result<SaveStatus> {
    let url = save_code_now_url(origin_url);
    let mut req = client.post(&url);
    if let Some(t) = token.filter(|s| !s.trim().is_empty()) {
        req = req.bearer_auth(t);
    }
    let resp = req
        .send()
        .await
        .context("Software Heritage save POST failed")?;
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    if !(200..300).contains(&status) {
        anyhow::bail!(
            "Software Heritage save POST HTTP {status}: {body}",
            status = status,
            body = &body[..body.len().min(400)]
        );
    }
    parse_save_status(&body)
}

/// Poll GET on the same URL until `task_status` is `"succeeded"` or `"failed"`,
/// or until `max_wait` elapses. Returns the final [`SaveStatus`].
///
/// Sleeps `interval` between polls.
pub async fn poll_save_status(
    client: &reqwest::Client,
    origin_url: &str,
    token: Option<&str>,
    max_wait: Duration,
    interval: Duration,
) -> Result<SaveStatus> {
    let url = save_code_now_url(origin_url);
    let deadline = tokio::time::Instant::now() + max_wait;
    loop {
        let mut req = client.get(&url);
        if let Some(t) = token.filter(|s| !s.trim().is_empty()) {
            req = req.bearer_auth(t);
        }
        let resp = req
            .send()
            .await
            .context("Software Heritage save GET failed")?;
        let status_code = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        if !(200..300).contains(&status_code) {
            anyhow::bail!(
                "Software Heritage save GET HTTP {status_code}: {body}",
                body = &body[..body.len().min(400)]
            );
        }
        let s = parse_save_status(&body)?;
        match s.task_status.as_str() {
            "succeeded" | "failed" => return Ok(s),
            _ => {}
        }
        if tokio::time::Instant::now() >= deadline {
            return Ok(s);
        }
        tokio::time::sleep(interval).await;
    }
}

/// Merge a completed SWH save into a publication `metadata_json` blob.
///
/// Sets `scientia.swh_save` to `{origin_url, request_status, task_status,
/// snapshot_swhid?}` and, when `snapshot_swhid` is present, also sets
/// `scientia.swhid` (used by `zenodo_metadata.rs`'s `related_identifiers`
/// builder). Returns the updated JSON string.
///
/// If `metadata_json` is `None` or empty a fresh object is created.
pub fn merge_swh_into_metadata_json(
    metadata_json: Option<&str>,
    status: &SaveStatus,
    origin_url: &str,
) -> Result<String> {
    let mut root: serde_json::Value =
        if let Some(s) = metadata_json.filter(|s| !s.trim().is_empty()) {
            serde_json::from_str(s).context("parse existing metadata_json")?
        } else {
            serde_json::Value::Object(serde_json::Map::new())
        };

    let obj = root
        .as_object_mut()
        .context("metadata_json must be a JSON object")?;

    let scientia = obj
        .entry("scientia")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let sci_obj = scientia
        .as_object_mut()
        .context("metadata_json.scientia must be an object")?;

    let mut swh_save = serde_json::json!({
        "origin_url": origin_url,
        "request_status": status.request_status,
        "task_status": status.task_status,
    });
    if let Some(swhid) = status.snapshot_swhid.as_deref() {
        swh_save.as_object_mut().unwrap().insert(
            "snapshot_swhid".into(),
            serde_json::Value::String(swhid.to_string()),
        );
        sci_obj.insert("swhid".into(), serde_json::Value::String(swhid.to_string()));
    }
    sci_obj.insert("swh_save".into(), swh_save);

    serde_json::to_string(&root).context("serialize updated metadata_json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_code_now_url_correct() {
        assert_eq!(
            save_code_now_url("https://github.com/vox-foundation/vox"),
            "https://archive.softwareheritage.org/api/1/origin/save/git/url/https://github.com/vox-foundation/vox/"
        );
    }

    #[test]
    fn save_code_now_url_trailing_slash_not_doubled() {
        let u = save_code_now_url("https://github.com/vox-foundation/vox/");
        assert_eq!(
            u,
            "https://archive.softwareheritage.org/api/1/origin/save/git/url/https://github.com/vox-foundation/vox/"
        );
    }

    #[test]
    fn parse_save_status_full() {
        let json = r#"{
            "save_request_status": "accepted",
            "save_task_status": "succeeded",
            "snapshot_swhid": "swh:1:snp:abc123"
        }"#;
        let s = parse_save_status(json).unwrap();
        assert_eq!(s.request_status, "accepted");
        assert_eq!(s.task_status, "succeeded");
        assert_eq!(s.snapshot_swhid.as_deref(), Some("swh:1:snp:abc123"));
    }

    #[test]
    fn parse_save_status_minimal() {
        let json = r#"{"save_request_status":"pending","save_task_status":"scheduled"}"#;
        let s = parse_save_status(json).unwrap();
        assert_eq!(s.request_status, "pending");
        assert_eq!(s.task_status, "scheduled");
        assert!(s.snapshot_swhid.is_none());
    }

    #[test]
    fn merge_swh_into_metadata_json_with_swhid() {
        let status = SaveStatus {
            request_status: "accepted".into(),
            task_status: "succeeded".into(),
            snapshot_swhid: Some("swh:1:snp:deadbeef".into()),
        };
        let out =
            merge_swh_into_metadata_json(None, &status, "https://github.com/foo/bar").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            v["scientia"]["swh_save"]["origin_url"].as_str().unwrap(),
            "https://github.com/foo/bar"
        );
        assert_eq!(
            v["scientia"]["swh_save"]["snapshot_swhid"]
                .as_str()
                .unwrap(),
            "swh:1:snp:deadbeef"
        );
        assert_eq!(
            v["scientia"]["swhid"].as_str().unwrap(),
            "swh:1:snp:deadbeef"
        );
    }

    #[test]
    fn merge_swh_into_metadata_json_no_swhid() {
        let status = SaveStatus {
            request_status: "accepted".into(),
            task_status: "not yet scheduled".into(),
            snapshot_swhid: None,
        };
        let existing = r#"{"scientia":{"title":"test"}}"#;
        let out = merge_swh_into_metadata_json(Some(existing), &status, "https://example.com/repo")
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["scientia"]["title"].as_str().unwrap(), "test");
        assert_eq!(
            v["scientia"]["swh_save"]["task_status"].as_str().unwrap(),
            "not yet scheduled"
        );
        assert!(v["scientia"].get("swhid").is_none());
    }
}
