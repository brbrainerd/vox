//! Server-side registry handlers.
//!
//! These are standalone async functions that accept a `CodeStore` reference
//! and request data, returning JSON-serializable responses.  They can be
//! wired into any HTTP framework (Axum, Actix, Warp, etc.)  by the CLI crate.

use serde::{Deserialize, Serialize};

use crate::hash::content_hash;
use crate::store::{CodeStore, PackageSearchResult, StoreError};

// ── Request / Response Types ────────────────────────────

/// Search request.
#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Search response.
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub packages: Vec<PackageInfo>,
    pub total: usize,
}

/// Package info (returned by search and info endpoints).
#[derive(Debug, Serialize)]
pub struct PackageInfo {
    pub name: String,
    pub latest_version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
}

/// Detailed package info (single-package endpoint).
#[derive(Debug, Serialize)]
pub struct PackageDetail {
    pub name: String,
    pub versions: Vec<String>,
    pub latest_version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
    pub hashes: Vec<String>,
}

/// Publish request.
#[derive(Debug, Deserialize)]
pub struct PublishRequest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(with = "base64_bytes")]
    pub data: Vec<u8>,
    #[serde(default)]
    pub dependencies: Vec<DepEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DepEntry {
    pub name: String,
    pub version_req: String,
}

/// Publish response.
#[derive(Debug, Serialize)]
pub struct PublishResponse {
    pub hash: String,
    pub message: String,
}

/// Download response.
#[derive(Debug, Serialize)]
pub struct DownloadResponse {
    pub content_hash: String,
    #[serde(with = "base64_bytes")]
    pub data: Vec<u8>,
}

/// Yank request.
#[derive(Debug, Deserialize)]
pub struct YankRequest {
    pub name: String,
    pub version: String,
}

/// Generic status response.
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub success: bool,
    pub message: String,
}

// ── Handlers ────────────────────────────────────────────

/// GET /api/registry/search?query=...&limit=...
pub async fn handle_search(
    store: &CodeStore,
    req: SearchRequest,
) -> Result<SearchResponse, StoreError> {
    let limit = req.limit.unwrap_or(20);
    let results = store.search_packages(&req.query, limit).await?;

    let packages: Vec<PackageInfo> = results
        .into_iter()
        .map(|r: PackageSearchResult| PackageInfo {
            name: r.name,
            latest_version: r.version,
            description: r.description,
            author: r.author,
            license: r.license,
        })
        .collect();

    let total = packages.len();
    Ok(SearchResponse { packages, total })
}

/// GET /api/registry/info/:name
pub async fn handle_info(
    store: &CodeStore,
    package_name: &str,
) -> Result<PackageDetail, StoreError> {
    let versions = store.get_package_versions(package_name).await?;
    if versions.is_empty() {
        return Err(StoreError::NotFound(format!(
            "package `{package_name}` not found"
        )));
    }

    let latest = &versions[0];
    // Get description from the latest version
    let all_results = store.search_packages(package_name, 1).await?;
    let first = all_results.first();

    Ok(PackageDetail {
        name: package_name.to_string(),
        versions: versions.iter().map(|(v, _)| v.clone()).collect(),
        latest_version: latest.0.clone(),
        description: first.and_then(|r| r.description.clone()),
        author: first.and_then(|r| r.author.clone()),
        license: first.and_then(|r| r.license.clone()),
        hashes: versions.iter().map(|(_, h)| h.clone()).collect(),
    })
}

/// POST /api/registry/publish
pub async fn handle_publish(
    store: &CodeStore,
    req: PublishRequest,
) -> Result<PublishResponse, StoreError> {
    // Check if this exact version already exists
    let existing = store.get_package_versions(&req.name).await?;
    if existing.iter().any(|(v, _)| v == &req.version) {
        return Err(StoreError::Conflict(format!(
            "{}@{} already published",
            req.name, req.version
        )));
    }

    // Content-address the package data
    let hash = content_hash(&req.data);

    // Store the raw data in CAS
    store.store("pkg", &req.data).await?;

    // Register the package
    store
        .publish_package(
            &req.name,
            &req.version,
            &hash,
            req.description.as_deref(),
            req.author.as_deref(),
            req.license.as_deref(),
        )
        .await?;

    // Register dependencies
    for dep in &req.dependencies {
        store
            .add_package_dep(&req.name, &req.version, &dep.name, &dep.version_req)
            .await?;
    }

    Ok(PublishResponse {
        hash,
        message: format!("Published {}@{}", req.name, req.version),
    })
}

/// GET /api/registry/download/:name/:version
pub async fn handle_download(
    store: &CodeStore,
    name: &str,
    version: &str,
) -> Result<DownloadResponse, StoreError> {
    let versions = store.get_package_versions(name).await?;

    // Find matching version (or latest)
    let (_, hash) = if version == "latest" {
        versions
            .first()
            .ok_or_else(|| StoreError::NotFound(format!("package `{name}` not found")))?
            .clone()
    } else {
        versions
            .iter()
            .find(|(v, _)| v == version)
            .ok_or_else(|| StoreError::NotFound(format!("{name}@{version} not found")))?
            .clone()
    };

    let data = store.get(&hash).await?;

    Ok(DownloadResponse {
        content_hash: hash,
        data,
    })
}

/// POST /api/registry/yank
pub async fn handle_yank(
    store: &CodeStore,
    req: YankRequest,
) -> Result<StatusResponse, StoreError> {
    let affected = store.yank_package(&req.name, &req.version).await?;
    if affected == 0 {
        return Err(StoreError::NotFound(format!(
            "{}@{} not found",
            req.name, req.version
        )));
    }
    Ok(StatusResponse {
        success: true,
        message: format!("Yanked {}@{}", req.name, req.version),
    })
}

/// DELETE /api/registry/packages/:name/:version
pub async fn handle_delete(
    store: &CodeStore,
    name: &str,
    version: &str,
) -> Result<StatusResponse, StoreError> {
    let affected = store.delete_package(name, version).await?;
    if affected == 0 {
        return Err(StoreError::NotFound(format!("{name}@{version} not found")));
    }
    Ok(StatusResponse {
        success: true,
        message: format!("Deleted {name}@{version}"),
    })
}

// ── Base64 serde helper ─────────────────────────────────

mod base64_bytes {
    use data_encoding::BASE64;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&BASE64.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        BASE64
            .decode(s.as_bytes())
            .map_err(serde::de::Error::custom)
    }
}
