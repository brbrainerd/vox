use serde::{Deserialize, Serialize};

/// Client for the VoxPM package registry.
/// Handles search, download, publish, and info operations.
pub struct RegistryClient {
    base_url: String,
    auth_token: Option<String>,
    client: reqwest::Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryPackageInfo {
    pub name: String,
    pub description: Option<String>,
    pub latest_version: String,
    pub versions: Vec<String>,
    pub author: Option<String>,
    pub license: Option<String>,
    pub downloads: u64,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub packages: Vec<RegistryPackageInfo>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishRequest {
    pub name: String,
    pub version: String,
    pub kind: String,
    pub description: Option<String>,
    pub license: Option<String>,
    pub content_hash: String,
    pub data: Vec<u8>,
    pub dependencies: Vec<PublishDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishDependency {
    pub name: String,
    pub version_req: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadResponse {
    pub content_hash: String,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub enum RegistryError {
    Http(reqwest::Error),
    Api(String),
    Auth(String),
    NotFound(String),
    Conflict(String),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(e) => write!(f, "HTTP error: {e}"),
            Self::Api(msg) => write!(f, "API error: {msg}"),
            Self::Auth(msg) => write!(f, "Auth error: {msg}"),
            Self::NotFound(pkg) => write!(f, "Package not found: {pkg}"),
            Self::Conflict(msg) => write!(f, "Conflict: {msg}"),
        }
    }
}

impl std::error::Error for RegistryError {}

impl From<reqwest::Error> for RegistryError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e)
    }
}

impl RegistryClient {
    /// Create a new registry client.
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_token: None,
            client: reqwest::Client::new(),
        }
    }

    /// Create a client with authentication.
    pub fn with_auth(base_url: &str, token: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            auth_token: Some(token.to_string()),
            client: reqwest::Client::new(),
        }
    }

    /// Set or update the auth token.
    pub fn set_token(&mut self, token: &str) {
        self.auth_token = Some(token.to_string());
    }

    /// Build a request with optional auth header.
    fn authed_get(&self, url: &str) -> reqwest::RequestBuilder {
        let mut req = self.client.get(url);
        if let Some(ref token) = self.auth_token {
            req = req.bearer_auth(token);
        }
        req
    }

    fn authed_put(&self, url: &str) -> reqwest::RequestBuilder {
        let mut req = self.client.put(url);
        if let Some(ref token) = self.auth_token {
            req = req.bearer_auth(token);
        }
        req
    }

    /// Search the registry for packages matching a query.
    pub async fn search(
        &self,
        query: &str,
        limit: u32,
        offset: u32,
    ) -> Result<SearchResult, RegistryError> {
        let url = format!(
            "{}/api/v1/packages?q={}&limit={}&offset={}",
            self.base_url, query, limit, offset
        );
        let resp = self.authed_get(&url).send().await?;

        if resp.status().is_success() {
            let result: SearchResult = resp.json().await?;
            Ok(result)
        } else if resp.status().as_u16() == 404 {
            Ok(SearchResult {
                packages: vec![],
                total: 0,
            })
        } else {
            let text = resp.text().await.unwrap_or_default();
            Err(RegistryError::Api(text))
        }
    }

    /// Get info about a specific package.
    pub async fn info(&self, name: &str) -> Result<RegistryPackageInfo, RegistryError> {
        let url = format!("{}/api/v1/packages/{}", self.base_url, name);
        let resp = self.authed_get(&url).send().await?;

        if resp.status().is_success() {
            let info: RegistryPackageInfo = resp.json().await?;
            Ok(info)
        } else if resp.status().as_u16() == 404 {
            Err(RegistryError::NotFound(name.to_string()))
        } else {
            let text = resp.text().await.unwrap_or_default();
            Err(RegistryError::Api(text))
        }
    }

    /// Download a specific version of a package.
    pub async fn download(
        &self,
        name: &str,
        version: &str,
    ) -> Result<DownloadResponse, RegistryError> {
        let url = format!(
            "{}/api/v1/packages/{}/{}/download",
            self.base_url, name, version
        );
        let resp = self.authed_get(&url).send().await?;

        if resp.status().is_success() {
            let dl: DownloadResponse = resp.json().await?;
            Ok(dl)
        } else if resp.status().as_u16() == 404 {
            Err(RegistryError::NotFound(format!("{name}@{version}")))
        } else {
            let text = resp.text().await.unwrap_or_default();
            Err(RegistryError::Api(text))
        }
    }

    /// Publish a package to the registry (requires auth).
    pub async fn publish(&self, req: PublishRequest) -> Result<(), RegistryError> {
        if self.auth_token.is_none() {
            return Err(RegistryError::Auth(
                "Authentication required for publishing. Run `vox login` first.".to_string(),
            ));
        }
        let url = format!("{}/api/v1/packages/{}", self.base_url, req.name);
        let resp = self.authed_put(&url).json(&req).send().await?;

        if resp.status().is_success() {
            Ok(())
        } else if resp.status().as_u16() == 409 {
            Err(RegistryError::Conflict(format!(
                "{}@{} already exists",
                req.name, req.version
            )))
        } else {
            let text = resp.text().await.unwrap_or_default();
            Err(RegistryError::Api(text))
        }
    }
}
