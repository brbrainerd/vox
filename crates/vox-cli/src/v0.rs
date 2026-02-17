use std::path::{Path, PathBuf};
use std::fs;
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context, anyhow};
use tracing::info;

use base64::Engine;

const V0_API_URL: &str = "https://api.v0.dev/v1/chats";

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

/// Generate a UI component using v0.dev based on a prompt.
///
/// This function calls the v0 Platform API to generate React code.
/// It expects the `V0_API_KEY` environment variable to be set.
pub async fn generate_component(prompt: &str, component_name: &str, out_dir: &Path, image_path: Option<&Path>) -> Result<PathBuf> {
    let api_key = std::env::var("V0_API_KEY").map_err(|_| {
        anyhow!("V0_API_KEY environment variable not found. Please set it to use @v0 components.")
    })?;

    if let Some(path) = image_path {
        info!("Generating v0 component '{}' with image: {:?}", component_name, path);
    } else {
        info!("Generating v0 component '{}' with prompt: \"{}\"", component_name, prompt);
    }

    let client = reqwest::Client::new();

    // We append specific instructions to ensure we get a single component file we can use
    let refined_prompt = format!(
        "Create a React component named {}. {}. \
        Return ONLY the code for this component in a file named {}.tsx. \
        Use Tailwind CSS for styling. Export the component as default.",
        component_name, prompt, component_name
    );

    let image_data = if let Some(path) = image_path {
        let bytes = fs::read(path).context(format!("Failed to read image file: {:?}", path))?;
        Some(base64::engine::general_purpose::STANDARD.encode(&bytes))
    } else {
        None
    };

    let req_body = ChatRequest {
        message: refined_prompt,
        stream: false,
        image: image_data,
    };

    let res = client.post(V0_API_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&req_body)
        .send()
        .await
        .context("Failed to send request to v0 API")?;

    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(anyhow!("v0 API error ({}): {}", status, text));
    }

    let chat_res: ChatResponse = res.json().await
        .context("Failed to parse v0 API response")?;

    // Find the component file
    if let Some(files) = chat_res.files {
        for file in files {
            // We look for the main component file
            if file.name.ends_with(".tsx") || file.name.ends_with(".jsx") {
                let file_path = out_dir.join(format!("{}.tsx", component_name));
                fs::write(&file_path, &file.content)
                    .context(format!("Failed to write generated component to {:?}", file_path))?;

                info!("Successfully generated v0 component at {:?}", file_path);
                return Ok(file_path);
            }
        }
        Err(anyhow!("v0 response did not contain any .tsx/.jsx files"))
    } else {
        Err(anyhow!("v0 response did not contain any files"))
    }
}
