use super::ast::ExtractedGraph;
use std::fs;
use std::path::{Path, PathBuf};

pub struct CacheManager {
    cache_dir: PathBuf,
}

impl CacheManager {
    pub fn new(cache_dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&cache_dir);
        CacheManager { cache_dir }
    }

    fn file_cache_path(&self, file_path: &Path) -> PathBuf {
        let hash = blake3::hash(file_path.to_string_lossy().as_bytes()).to_hex();
        self.cache_dir.join(format!("{}.json", hash))
    }

    pub fn get_cached_hash(&self, file_path: &Path) -> Option<String> {
        let cache_path = self.file_cache_path(file_path);
        if cache_path.exists() {
            if let Ok(content) = fs::read_to_string(cache_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    return json
                        .get("hash")
                        .and_then(|h| h.as_str())
                        .map(str::to_string);
                }
            }
        }
        None
    }

    pub fn write_cache(&self, file_path: &Path, hash: &str, graph: &ExtractedGraph) {
        let cache_path = self.file_cache_path(file_path);
        let val = serde_json::json!({
            "hash": hash,
            "graph": graph
        });
        if let Ok(serialized) = serde_json::to_string_pretty(&val) {
            let _ = fs::write(cache_path, serialized);
        }
    }

    pub fn load_cache(&self, file_path: &Path) -> Option<ExtractedGraph> {
        let cache_path = self.file_cache_path(file_path);
        if cache_path.exists() {
            if let Ok(content) = fs::read_to_string(cache_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(graph_val) = json.get("graph") {
                        return serde_json::from_value::<ExtractedGraph>(graph_val.clone()).ok();
                    }
                }
            }
        }
        None
    }
}
