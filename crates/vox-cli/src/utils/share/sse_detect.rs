//! SSE (Server-Sent Events) route detection.
//!
//! Probes `http://127.0.0.1:<port>/openapi.json` and checks whether any route
//! returns `text/event-stream`. Used to auto-switch away from Cloudflare backend
//! (which buffers responses and breaks SSE).

/// Returns true if the app at `localhost:<port>` has any SSE routes.
///
/// Fetches `/openapi.json` with a short timeout. Returns false on any error
/// (app might not be running yet, or might not have an OpenAPI endpoint).
pub async fn has_sse_routes(upstream_port: u16) -> bool {
    let url = format!("http://127.0.0.1:{}/openapi.json", upstream_port);
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    let Ok(resp) = client.get(&url).send().await else {
        return false;
    };
    let Ok(json) = resp.json::<serde_json::Value>().await else {
        return false;
    };

    detect_sse_in_openapi(&json)
}

/// Returns true if any route in the OpenAPI spec has a `text/event-stream` response.
pub fn detect_sse_in_openapi(spec: &serde_json::Value) -> bool {
    let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) else {
        return false;
    };
    for (_path, path_item) in paths {
        let Some(methods) = path_item.as_object() else {
            continue;
        };
        for (_method, operation) in methods {
            if operation_has_sse(operation) {
                return true;
            }
        }
    }
    false
}

fn operation_has_sse(op: &serde_json::Value) -> bool {
    let Some(responses) = op.get("responses").and_then(|r| r.as_object()) else {
        return false;
    };
    for (_status, response) in responses {
        if let Some(content) = response.get("content").and_then(|c| c.as_object())
            && content.contains_key("text/event-stream")
        {
            return true;
        }
        // Also check $ref-resolved inline schemas — but for S6 MVP, just check content directly.
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_sse_in_openapi_spec() {
        let spec = json!({
            "paths": {
                "/stream": {
                    "get": {
                        "responses": {
                            "200": {
                                "content": {
                                    "text/event-stream": {
                                        "schema": { "type": "string" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        assert!(detect_sse_in_openapi(&spec));
    }

    #[test]
    fn no_sse_in_regular_spec() {
        let spec = json!({
            "paths": {
                "/health": {
                    "get": {
                        "responses": {
                            "200": {
                                "content": {
                                    "application/json": {}
                                }
                            }
                        }
                    }
                }
            }
        });
        assert!(!detect_sse_in_openapi(&spec));
    }

    #[test]
    fn empty_spec_returns_false() {
        assert!(!detect_sse_in_openapi(&json!({})));
        assert!(!detect_sse_in_openapi(&json!({"paths": {}})));
    }
}
