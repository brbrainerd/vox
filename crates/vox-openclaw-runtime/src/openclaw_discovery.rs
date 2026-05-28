//! OpenClaw endpoint discovery and cache resolution.

use serde::Deserialize;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DEFAULT_HTTP_GATEWAY_URL: &str = "http://127.0.0.1:3000";
pub const DEFAULT_WS_GATEWAY_URL: &str = "ws://127.0.0.1:18789";
pub const DEFAULT_CACHE_TTL_SECONDS: u64 = 300;
const MIN_CACHE_TTL_SECONDS: u64 = 30;
const MAX_CACHE_TTL_SECONDS: u64 = 86_400;

/// Optional override knobs for discovery resolution.
#[derive(Debug, Clone, Default)]
pub struct OpenClawDiscoveryOverrides {
    pub explicit_http_gateway_url: Option<String>,
    pub explicit_ws_gateway_url: Option<String>,
    pub explicit_well_known_url: Option<String>,
}

/// Resolved endpoint bundle consumed by CLI/MCP/runtime adapter wiring.
#[derive(Debug, Clone)]
pub struct OpenClawResolvedEndpoints {
    pub http_gateway_url: String,
    pub ws_gateway_url: String,
    pub catalog_list_url: Option<String>,
    pub catalog_search_url: Option<String>,
    pub discovery_source: String,
    pub cache_expires_at_ms: Option<u64>,
}

#[derive(Debug, Clone)]
struct CachedEntry {
    endpoints: OpenClawResolvedEndpoints,
    expires_at_ms: u64,
}

#[derive(Debug, Default)]
struct DiscoveryCache {
    current: Option<CachedEntry>,
    last_good: Option<OpenClawResolvedEndpoints>,
}

#[derive(Debug, Deserialize)]
struct OpenClawWellKnownGateway {
    #[serde(
        default,
        alias = "gatewayUrl",
        alias = "gateway_url",
        alias = "http",
        alias = "http_url"
    )]
    http_url: Option<String>,
    #[serde(default, alias = "ws", alias = "ws_url", alias = "gatewayWsUrl")]
    ws_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenClawWellKnownCatalog {
    #[serde(default, alias = "list", alias = "skillsUrl")]
    list_url: Option<String>,
    #[serde(default, alias = "search", alias = "searchEndpoint")]
    search_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenClawWellKnownDocument {
    #[serde(default, alias = "schema_version")]
    schema_version: Option<u64>,
    #[serde(default)]
    gateway: Option<OpenClawWellKnownGateway>,
    #[serde(default)]
    catalog: Option<OpenClawWellKnownCatalog>,
    #[serde(default, alias = "cache_ttl_seconds")]
    cache_ttl_seconds: Option<u64>,
}

static DISCOVERY_CACHE: OnceLock<Mutex<DiscoveryCache>> = OnceLock::new();

fn cache() -> &'static Mutex<DiscoveryCache> {
    DISCOVERY_CACHE.get_or_init(|| Mutex::new(DiscoveryCache::default()))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_millis() as u64
}

fn trim_nonempty(input: Option<String>) -> Option<String> {
    input.and_then(|s| {
        let trimmed = s.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn env_nonempty(name: &str) -> Option<String> {
    trim_nonempty(std::env::var(name).ok())
}

fn clamp_ttl(ttl: Option<u64>) -> u64 {
    ttl.unwrap_or(DEFAULT_CACHE_TTL_SECONDS)
        .clamp(MIN_CACHE_TTL_SECONDS, MAX_CACHE_TTL_SECONDS)
}

fn derive_default_well_known_url(http_gateway_url: &str) -> String {
    format!(
        "{}/.well-known/openclaw.json",
        http_gateway_url.trim_end_matches('/')
    )
}

fn apply_precedence(
    base: OpenClawResolvedEndpoints,
    overrides: &OpenClawDiscoveryOverrides,
) -> OpenClawResolvedEndpoints {
    let env_http = env_nonempty("VOX_OPENCLAW_URL");
    let env_ws = env_nonempty("VOX_OPENCLAW_WS_URL");
    let env_catalog_list = env_nonempty("VOX_OPENCLAW_CATALOG_LIST_URL");
    let env_catalog_search = env_nonempty("VOX_OPENCLAW_CATALOG_SEARCH_URL");

    OpenClawResolvedEndpoints {
        http_gateway_url: trim_nonempty(overrides.explicit_http_gateway_url.clone())
            .or(env_http)
            .unwrap_or(base.http_gateway_url),
        ws_gateway_url: trim_nonempty(overrides.explicit_ws_gateway_url.clone())
            .or(env_ws)
            .unwrap_or(base.ws_gateway_url),
        catalog_list_url: env_catalog_list.or(base.catalog_list_url),
        catalog_search_url: env_catalog_search.or(base.catalog_search_url),
        discovery_source: base.discovery_source,
        cache_expires_at_ms: base.cache_expires_at_ms,
    }
}

fn fallback_endpoints(source: String) -> OpenClawResolvedEndpoints {
    OpenClawResolvedEndpoints {
        http_gateway_url: DEFAULT_HTTP_GATEWAY_URL.to_string(),
        ws_gateway_url: DEFAULT_WS_GATEWAY_URL.to_string(),
        catalog_list_url: Some(format!("{DEFAULT_HTTP_GATEWAY_URL}/v1/skills")),
        catalog_search_url: Some(format!("{DEFAULT_HTTP_GATEWAY_URL}/v1/skills/search")),
        discovery_source: source,
        cache_expires_at_ms: None,
    }
}

async fn fetch_well_known(
    well_known_url: &str,
) -> Result<(OpenClawResolvedEndpoints, u64), crate::openclaw_adapter::OpenClawAdapterError> {
    let client = vox_http_client::client_builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| crate::openclaw_adapter::OpenClawAdapterError::Other(e.to_string()))?;
    let resp = client
        .get(well_known_url)
        .send()
        .await
        .map_err(|e| crate::openclaw_adapter::OpenClawAdapterError::Other(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(crate::openclaw_adapter::OpenClawAdapterError::Other(
            format!("OpenClaw discovery HTTP {}", resp.status()),
        ));
    }
    let doc: OpenClawWellKnownDocument = resp
        .json()
        .await
        .map_err(|e| crate::openclaw_adapter::OpenClawAdapterError::Other(e.to_string()))?;

    let _schema_version = doc.schema_version.unwrap_or(1);
    let mut out = fallback_endpoints(format!("well-known:{well_known_url}"));
    if let Some(gateway) = doc.gateway {
        if let Some(http_url) = trim_nonempty(gateway.http_url) {
            out.http_gateway_url = http_url;
        }
        if let Some(ws_url) = trim_nonempty(gateway.ws_url) {
            out.ws_gateway_url = ws_url;
        }
    }
    if let Some(catalog) = doc.catalog {
        out.catalog_list_url = trim_nonempty(catalog.list_url).or(out.catalog_list_url);
        out.catalog_search_url = trim_nonempty(catalog.search_url).or(out.catalog_search_url);
    }
    let ttl = clamp_ttl(doc.cache_ttl_seconds);
    Ok((out, ttl))
}

/// Resolve OpenClaw endpoints with shared cache + last-known-good fallback.
pub async fn resolve_openclaw_endpoints(
    overrides: OpenClawDiscoveryOverrides,
) -> OpenClawResolvedEndpoints {
    let now = now_ms();
    if let Some(cached) = {
        let guard = cache().lock().unwrap_or_else(|e| e.into_inner());
        guard.current.clone()
    } && cached.expires_at_ms > now
    {
        return apply_precedence(cached.endpoints, &overrides);
    }

    let seed_http = trim_nonempty(overrides.explicit_http_gateway_url.clone())
        .or_else(|| env_nonempty("VOX_OPENCLAW_URL"))
        .unwrap_or_else(|| DEFAULT_HTTP_GATEWAY_URL.to_string());
    let well_known_url = trim_nonempty(overrides.explicit_well_known_url.clone())
        .or_else(|| env_nonempty("VOX_OPENCLAW_WELL_KNOWN_URL"))
        .unwrap_or_else(|| derive_default_well_known_url(&seed_http));

    match fetch_well_known(&well_known_url).await {
        Ok((mut discovered, ttl_seconds)) => {
            let expires_at_ms = now.saturating_add(ttl_seconds.saturating_mul(1000));
            discovered.cache_expires_at_ms = Some(expires_at_ms);
            {
                let mut guard = cache().lock().unwrap_or_else(|e| e.into_inner());
                guard.current = Some(CachedEntry {
                    endpoints: discovered.clone(),
                    expires_at_ms,
                });
                guard.last_good = Some(discovered.clone());
            }
            apply_precedence(discovered, &overrides)
        }
        Err(err) => {
            let last_good = {
                let guard = cache().lock().unwrap_or_else(|e| e.into_inner());
                guard.last_good.clone()
            };
            let fallback = match last_good {
                Some(mut cached) => {
                    cached.discovery_source = format!("cache-last-good (fetch failed: {err})");
                    cached
                }
                None => fallback_endpoints(format!("defaults (fetch failed: {err})")),
            };
            apply_precedence(fallback, &overrides)
        }
    }
}

#[cfg(test)]
pub fn clear_openclaw_discovery_cache_for_tests() {
    let mut guard = cache().lock().unwrap_or_else(|e| e.into_inner());
    guard.current = None;
    guard.last_good = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ttl_is_clamped_to_supported_bounds() {
        assert_eq!(clamp_ttl(Some(1)), MIN_CACHE_TTL_SECONDS);
        assert_eq!(clamp_ttl(Some(9_999_999)), MAX_CACHE_TTL_SECONDS);
        assert_eq!(clamp_ttl(None), DEFAULT_CACHE_TTL_SECONDS);
    }

    #[test]
    fn derives_well_known_from_http_base() {
        let got = derive_default_well_known_url("https://gateway.example/");
        assert_eq!(got, "https://gateway.example/.well-known/openclaw.json");
    }

    #[test]
    fn fallback_endpoints_include_default_catalog_urls() {
        let got = fallback_endpoints("defaults".to_string());
        assert_eq!(got.http_gateway_url, DEFAULT_HTTP_GATEWAY_URL);
        assert_eq!(got.ws_gateway_url, DEFAULT_WS_GATEWAY_URL);
        assert_eq!(
            got.catalog_list_url.as_deref(),
            Some("http://127.0.0.1:3000/v1/skills")
        );
        assert_eq!(
            got.catalog_search_url.as_deref(),
            Some("http://127.0.0.1:3000/v1/skills/search")
        );
    }
}
