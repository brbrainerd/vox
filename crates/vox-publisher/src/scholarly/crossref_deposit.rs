//! Crossref deposit adapter: POST metadata XML to doi.crossref.org.
//!
//! Builds a minimal Crossref Deposit XML schema v5.3.1 document and POSTs it
//! as multipart/form-data. Uses `VoxCrossrefPlusApiKey` as `login_passwd`.
//! Reference: <https://www.crossref.org/documentation/member-setup/direct-deposit-xml/>

use async_trait::async_trait;
use chrono::Utc;

use super::error::classify_scholarly_http;
use super::flags;
use super::{ScholarlyAdapter, ScholarlyError, ScholarlyRemoteStatus, ScholarlySubmissionReceipt};
use crate::publication::PublicationManifest;

const CROSSREF_DEPOSIT_URL: &str = "https://doi.crossref.org/servlet/deposit";

#[derive(Debug, Clone)]
pub struct CrossrefDepositAdapter {
    endpoint: String,
    login_id: String,
    login_passwd: String,
    http: reqwest::Client,
}

impl CrossrefDepositAdapter {
    pub fn new(endpoint: String, login_id: String, login_passwd: String) -> Self {
        let http = vox_http_client::client_builder()
            .user_agent("vox-publisher/crossref")
            .timeout(vox_config::timeouts::D_60S)
            .build()
            .expect("crossref http client");
        Self {
            endpoint,
            login_id,
            login_passwd,
            http,
        }
    }
}

pub(super) fn crossref_from_secrets() -> Result<CrossrefDepositAdapter, ScholarlyError> {
    let passwd = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxCrossrefPlusApiKey)
        .expose()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ScholarlyError::Config {
            message: "VoxCrossrefPlusApiKey is not set".into(),
        })?;
    let login_id = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxScientiaCrossrefMailto)
        .expose()
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "scientia@voxlang.org".to_string());
    Ok(CrossrefDepositAdapter::new(
        CROSSREF_DEPOSIT_URL.to_string(),
        login_id,
        passwd,
    ))
}

/// Build a minimal Crossref Deposit XML for a journal article / preprint.
pub fn build_crossref_deposit_xml(doi: &str, title: &str, author: &str, date: &str) -> String {
    let batch_id = format!("vox-{}", sha3_short(doi));
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<doi_batch version="5.3.1"
  xmlns="http://www.crossref.org/schema/5.3.1"
  xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
  xsi:schemaLocation="http://www.crossref.org/schema/5.3.1
    https://www.crossref.org/schemas/crossref5.3.1.xsd">
  <head>
    <doi_batch_id>{batch_id}</doi_batch_id>
    <timestamp>{ts}</timestamp>
    <depositor>
      <depositor_name>Vox SCIENTIA</depositor_name>
      <email_address>{author_email}</email_address>
    </depositor>
    <registrant>Vox Research</registrant>
  </head>
  <body>
    <posted_content type="preprint">
      <contributors>
        <person_name sequence="first" contributor_role="author">
          <surname>{surname}</surname>
        </person_name>
      </contributors>
      <titles><title>{title}</title></titles>
      <posted_date>
        <month>{month}</month>
        <day>{day}</day>
        <year>{year}</year>
      </posted_date>
      <doi_data>
        <doi>{doi}</doi>
        <resource>https://doi.org/{doi}</resource>
      </doi_data>
    </posted_content>
  </body>
</doi_batch>"#,
        batch_id = batch_id,
        ts = date.replace('-', ""),
        author_email = "scientia@voxlang.org",
        surname = xml_escape(author),
        title = xml_escape(title),
        doi = xml_escape(doi),
        month = date.get(5..7).unwrap_or("01"),
        day = date.get(8..10).unwrap_or("01"),
        year = date.get(0..4).unwrap_or("2026"),
    )
}

fn sha3_short(s: &str) -> String {
    use sha3::{Digest, Sha3_256};
    let h = Sha3_256::digest(s.as_bytes());
    hex::encode(&h[..8])
}

pub(crate) fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[async_trait]
impl ScholarlyAdapter for CrossrefDepositAdapter {
    fn adapter_name(&self) -> &'static str {
        "crossref_deposit"
    }

    async fn submit(
        &self,
        manifest: &PublicationManifest,
    ) -> Result<ScholarlySubmissionReceipt, ScholarlyError> {
        if flags::scholarly_live_globally_disabled() {
            return Err(ScholarlyError::Disabled {
                reason: "VOX_SCHOLARLY_DISABLE_LIVE is set".into(),
            });
        }
        // DOI may be in metadata_json; default to a provisional identifier.
        let doi = manifest
            .metadata_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .and_then(|v| v["doi"].as_str().map(str::to_string))
            .unwrap_or_else(|| format!("10.5281/vox-provisional-{}", manifest.publication_id));
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let xml = build_crossref_deposit_xml(&doi, &manifest.title, &manifest.author, &today);
        let part = reqwest::multipart::Part::bytes(xml.into_bytes())
            .file_name("crossref_deposit.xml")
            .mime_str("application/xml")
            .map_err(|e| ScholarlyError::Config {
                message: e.to_string(),
            })?;
        let form = reqwest::multipart::Form::new()
            .text("login_id", self.login_id.clone())
            .text("login_passwd", self.login_passwd.clone())
            .part("fname", part);
        let resp = self
            .http
            .post(&self.endpoint)
            .multipart(form)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        if !(200..300).contains(&status) {
            return Err(classify_scholarly_http(status, &text));
        }
        let external_id = format!("crossref-{}", sha3_short(&doi));
        let truncated = &text[..text.len().min(500)];
        Ok(ScholarlySubmissionReceipt {
            adapter: self.adapter_name().to_string(),
            external_submission_id: external_id,
            status: "deposited".to_string(),
            response_fingerprint: Some(manifest.content_sha3_256()),
            metadata_json: Some(
                serde_json::json!({ "doi": doi, "crossref_response": truncated }).to_string(),
            ),
        })
    }

    async fn fetch_status(
        &self,
        external_submission_id: &str,
    ) -> Result<ScholarlyRemoteStatus, ScholarlyError> {
        Ok(ScholarlyRemoteStatus {
            status: "deposited".to_string(),
            detail_json: Some(
                serde_json::json!({
                    "external_submission_id": external_submission_id,
                    "note": "Crossref deposit is fire-and-forget; poll via Crossref REST API separately."
                })
                .to_string(),
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossref_xml_contains_doi_and_title() {
        let xml =
            build_crossref_deposit_xml("10.1234/test-doi", "My Title", "Author Name", "2026-05-09");
        assert!(xml.contains("10.1234/test-doi"));
        assert!(xml.contains("My Title"));
        assert!(xml.contains("Author Name"));
    }

    #[test]
    fn crossref_adapter_name() {
        let adapter = CrossrefDepositAdapter::new(
            "https://doi.crossref.org/servlet/deposit".into(),
            "user".into(),
            "pass".into(),
        );
        assert_eq!(adapter.adapter_name(), "crossref_deposit");
    }

    #[test]
    fn xml_escape_handles_special_chars() {
        assert_eq!(xml_escape("A & B < C > D"), "A &amp; B &lt; C &gt; D");
    }
}
