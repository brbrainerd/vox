//! Hugging Face Hub downloads for `vox mens train --model <repo_id>`.

use std::path::PathBuf;

use hf_hub::{HFClient, split_id};

fn normalize_hf_token_env() {
    let token_resolved = vox_secrets::resolve_secret(vox_secrets::SecretId::HuggingFaceToken);
    let token = token_resolved.expose();

    if let Some(token) = token {
        // hf-hub defaults to HF_TOKEN then HUGGING_FACE_HUB_TOKEN.
        // We set both to ensure the crate finds it regardless of its internal priority.
        #[allow(unsafe_code)]
        // SAFETY: Called sequentially before spawning HF requests.
        unsafe {
            std::env::set_var("HF_TOKEN", token);
            std::env::set_var("HUGGING_FACE_HUB_TOKEN", token);
        }
    }
}

/// Resolved local paths after downloading a model repo snapshot.
#[derive(Debug, Clone)]
pub struct DownloadedModelFiles {
    /// Directory containing the resolved snapshot (parent of `config.json` when present).
    pub cache_dir: PathBuf,
    pub config: PathBuf,
    pub weights: Vec<PathBuf>,
    pub tokenizer: Option<PathBuf>,
}

impl DownloadedModelFiles {
    /// True if at least one weight file uses the SafeTensors format.
    #[must_use]
    pub fn is_safetensors(&self) -> bool {
        self.weights.iter().any(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("safetensors"))
        })
    }
}

/// Download `config.json`, tokenizer files (if listed), and all `*.safetensors` shards.
pub async fn download_model(repo_id: &str) -> anyhow::Result<DownloadedModelFiles> {
    normalize_hf_token_env();
    let client = HFClient::new().map_err(|e| anyhow::anyhow!("hf-hub HFClient::new: {e}"))?;
    let (owner, name) = split_id(repo_id);
    let repo = client.model(owner, name);
    let info = repo
        .info()
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("hf-hub repo info for {repo_id}: {e}"))?;
    // 1.0 makes `siblings` optional; an absent listing is indistinguishable from
    // an empty one at the type level, so name the difference here rather than
    // silently reporting "no safetensors" for a repo we simply failed to list.
    let siblings = info
        .siblings
        .ok_or_else(|| anyhow::anyhow!("repo {repo_id} info returned no file listing"))?;

    let config = repo
        .download_file()
        .filename("config.json")
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("download config.json: {e}"))?;

    let cache_dir = config
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let mut tokenizer = None::<PathBuf>;
    for name in ["tokenizer.json", "tokenizer.model"] {
        if siblings.iter().any(|s| s.rfilename == name) {
            tokenizer = Some(
                repo.download_file()
                    .filename(name)
                    .send()
                    .await
                    .map_err(|e| anyhow::anyhow!("download {name}: {e}"))?,
            );
            break;
        }
    }

    let mut weight_names: Vec<&str> = siblings
        .iter()
        .map(|s| s.rfilename.as_str())
        .filter(|n| n.ends_with(".safetensors"))
        .collect();
    weight_names.sort_unstable();
    if weight_names.is_empty() {
        anyhow::bail!(
            "repo {repo_id} has no *.safetensors files in the Hub manifest; need a safetensors-based model"
        );
    }

    let mut weights = Vec::with_capacity(weight_names.len());
    for w in weight_names {
        let p = repo
            .download_file()
            .filename(w)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("download {w}: {e}"))?;
        weights.push(p);
    }

    Ok(DownloadedModelFiles {
        cache_dir,
        config,
        weights,
        tokenizer,
    })
}

/// Block on [`download_model`] using a dedicated runtime (sync training / CLI entrypoints).
pub fn download_model_blocking(repo_id: &str) -> anyhow::Result<DownloadedModelFiles> {
    let repo_id = repo_id.to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = tokio::runtime::Runtime::new()
            .map_err(|e| anyhow::anyhow!("tokio runtime init failed: {e}"))
            .and_then(|rt| rt.block_on(download_model(&repo_id)));
        let _ = tx.send(result);
    });
    rx.recv()
        .map_err(|_| anyhow::anyhow!("HF download thread exited without sending result"))?
}

#[cfg(all(test, feature = "mens-hf-hub"))]
#[allow(unsafe_code)] // Serialized env mutation for token sync tests (Rust 2024 `set_var` safety).
mod tests {
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn hf_token_propagates_to_hugging_face_hub_token() {
        let _g = ENV_LOCK.lock().expect("env lock");
        unsafe {
            std::env::remove_var("HF_TOKEN");
            std::env::remove_var("HUGGING_FACE_HUB_TOKEN");
            std::env::set_var("HF_TOKEN", "from-hf-only");
        }
        super::normalize_hf_token_env();
        assert_eq!(
            vox_secrets::resolve_secret(vox_secrets::SecretId::HuggingFaceToken)
                .expose()
                .expect("hub token"),
            "from-hf-only"
        );
        unsafe {
            std::env::remove_var("HF_TOKEN");
            std::env::remove_var("HUGGING_FACE_HUB_TOKEN");
        }
    }

    #[test]
    fn hugging_face_hub_token_propagates_to_hf_token() {
        let _g = ENV_LOCK.lock().expect("env lock");
        unsafe {
            std::env::remove_var("HF_TOKEN");
            std::env::remove_var("HUGGING_FACE_HUB_TOKEN");
            std::env::set_var("HUGGING_FACE_HUB_TOKEN", "from-hub-only");
        }
        super::normalize_hf_token_env();
        assert_eq!(
            vox_secrets::resolve_secret(vox_secrets::SecretId::HuggingFaceToken)
                .expose()
                .expect("hf token"),
            "from-hub-only"
        );
        unsafe {
            std::env::remove_var("HF_TOKEN");
            std::env::remove_var("HUGGING_FACE_HUB_TOKEN");
        }
    }
}
