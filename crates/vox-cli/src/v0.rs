//! Integration with **v0.dev** for AI-generated React components referenced from Vox `@v0` declarations.
//!
//! Requires `V0_API_KEY`. Called from `commands::build` when a generated `.tsx` path is still missing.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

use base64::Engine;

const V0_API_URL: &str = "https://api.v0.dev/v1/chats";

/// Full URL for the v0 chats endpoint (including path). Override with **`VOX_V0_API_URL`** for tests or proxies.
fn v0_chats_url() -> String {
    std::env::var("VOX_V0_API_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| V0_API_URL.to_string())
}

fn extract_tsx_from_chat_response(chat_res: ChatResponse) -> Result<String> {
    if let Some(files) = chat_res.files {
        for file in files {
            if file.name.ends_with(".tsx") || file.name.ends_with(".jsx") {
                return Ok(file.content);
            }
        }
        Err(anyhow!("v0 response did not contain any .tsx/.jsx files"))
    } else {
        Err(anyhow!("v0 response did not contain any files"))
    }
}

#[derive(Serialize)]
struct ChatRequest {
    message: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ChatResponse {
    id: String,
    files: Option<Vec<V0File>>,
    demo: Option<String>,
}

#[derive(Deserialize)]
struct V0File {
    name: String,
    content: String,
}

fn v0_refined_prompt(component_name: &str, user_instruction: &str) -> String {
    format!(
        "Create a React component named {component_name}. {user_instruction}. \
        Return ONLY the code for this component in a file named {component_name}.tsx. \
        Use Tailwind CSS for styling. Use a **named** export: `export function {component_name}` \
        (not default export) so Vox `routes:` can import `{{ {component_name} }}`."
    )
}

async fn fetch_v0_tsx(
    component_name: &str,
    user_instruction: &str,
    image_path: Option<&Path>,
) -> Result<String> {
    let api_key = vox_secrets::resolve_secret(vox_secrets::SecretId::V0ApiKey)
        .expose()
        .map(std::string::ToString::to_string)
        .ok_or_else(|| {
            anyhow!(
                "V0_API_KEY environment variable not found. Please set it to use @v0 components."
            )
        })?;

    if let Some(path) = image_path {
        info!(
            "Generating v0 component '{}' with image: {:?}",
            component_name, path
        );
    } else {
        info!(
            "Generating v0 component '{}' with prompt: \"{}\"",
            component_name, user_instruction
        );
    }

    let client = vox_http_client::client();
    let message = v0_refined_prompt(component_name, user_instruction);

    let image_data = if let Some(path) = image_path {
        let bytes = fs::read(path).context(format!("Failed to read image file: {:?}", path))?;
        Some(base64::engine::general_purpose::STANDARD.encode(&bytes))
    } else {
        None
    };

    let req_body = ChatRequest {
        message,
        stream: false,
        image: image_data,
    };

    let url = v0_chats_url();
    let res = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&req_body)
        .send()
        .await
        .with_context(|| format!("Failed to send request to v0 API ({url})"))?;

    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(anyhow!("v0 API error ({}): {}", status, text));
    }

    let chat_res: ChatResponse = res
        .json()
        .await
        .context("Failed to parse v0 API response")?;

    extract_tsx_from_chat_response(chat_res)
}

/// Generate a UI component using v0.dev based on a prompt.
///
/// This function calls the v0 Platform API to generate React code.
/// It expects the `V0_API_KEY` environment variable to be set.
pub async fn generate_component(
    prompt: &str,
    component_name: &str,
    out_dir: &Path,
    image_path: Option<&Path>,
) -> Result<PathBuf> {
    let raw = fetch_v0_tsx(component_name, prompt, image_path).await?;
    let content = crate::v0_tsx_normalize::normalize_v0_tsx_named_export(raw, component_name);
    let file_path = out_dir.join(format!("{component_name}.tsx"));
    fs::write(&file_path, &content).context(format!(
        "Failed to write generated component to {:?}",
        file_path
    ))?;
    info!("Successfully generated v0 component at {:?}", file_path);
    Ok(file_path)
}

#[cfg(test)]
mod v0_response_tests {
    use super::*;

    #[test]
    fn extract_tsx_prefers_tsx_file() {
        let chat = ChatResponse {
            id: "1".into(),
            files: Some(vec![
                V0File {
                    name: "readme.md".into(),
                    content: "# x".into(),
                },
                V0File {
                    name: "Widget.tsx".into(),
                    content: "export function Widget() {}".into(),
                },
            ]),
            demo: None,
        };
        let s = extract_tsx_from_chat_response(chat).unwrap();
        assert!(s.contains("Widget"));
    }

    #[test]
    fn extract_tsx_errors_when_no_files() {
        let chat = ChatResponse {
            id: "1".into(),
            files: None,
            demo: None,
        };
        assert!(extract_tsx_from_chat_response(chat).is_err());
    }
}

#[cfg(test)]
#[allow(unsafe_code)]
mod v0_wiremock_tests {
    use super::*;
    use serial_test::serial;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn restore_env(key: &str, prev: Option<String>) {
        // SAFETY: `serial_test` runs this module's tests sequentially; we restore before returning.
        unsafe {
            match prev {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[serial]
    async fn fetch_hits_vox_v0_api_url() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chats"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"id":"mock","files":[{"name":"Demo.tsx","content":"export function Demo() { return null; }"}]}"#,
            ))
            .mount(&server)
            .await;

        let url = format!("{}/v1/chats", server.uri());
        let prev_url = std::env::var("VOX_V0_API_URL").ok();
        let prev_key = std::env::var(vox_secrets::SecretId::V0ApiKey.spec().canonical_env).ok();
        // SAFETY: paired with `restore_env` below; serialized by `#[serial]`.
        unsafe {
            std::env::set_var("VOX_V0_API_URL", url.as_str());
            std::env::set_var(
                vox_secrets::SecretId::V0ApiKey.spec().canonical_env,
                "test-key",
            );
        }

        let got = fetch_v0_tsx("Demo", "make a card", None)
            .await
            .expect("fetch");
        assert!(got.contains("export function Demo"));

        restore_env("VOX_V0_API_URL", prev_url);
        restore_env(
            vox_secrets::SecretId::V0ApiKey.spec().canonical_env,
            prev_key,
        );
    }
}
