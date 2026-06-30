//! [`StackPlanner::plan`] — partition files into stack chunks.

use super::heuristics;
use super::types::{StackChunk, StackManifest, StackPlanConfig, StackPlanner};

impl StackPlanner {
    /// Create a new planner from the given configuration.
    pub fn new(config: StackPlanConfig) -> Self {
        Self { config }
    }

    /// Determines if a file should be hidden from CodeRabbit entirely.
    pub fn is_ignored(path: &str) -> bool {
        heuristics::is_ignored(path)
    }

    /// Maps a file to its semantic layer and chunk order (used for all-file stack scans).
    ///
    /// For changed-only reviews use [`super::super::semantic_planner::SemanticPlanner`] instead.
    pub fn get_chunk_id(path: &str) -> (u32, &'static str) {
        heuristics::get_chunk_id(path)
    }

    /// Partition `all_files` from `git ls-files` into semantic PR chunks.
    ///
    /// Files matched by [`StackPlanner::is_ignored`] are excluded.
    /// Large chunks are sub-divided into parts of at most `max_files_per_pr` files.
    pub fn plan(&self, all_files: Vec<String>) -> StackManifest {
        let mut chunks_map = std::collections::HashMap::new();

        let mut total_files = 0;
        for file in all_files {
            if Self::is_ignored(&file) {
                continue;
            }
            total_files += 1;
            let (order, name) = Self::get_chunk_id(&file);

            let chunk = chunks_map
                .entry(name.to_string())
                .or_insert_with(|| StackChunk {
                    order,
                    name: name.to_string(),
                    files: Vec::new(),
                });
            chunk.files.push(file.clone());
        }

        let mut chunks: Vec<StackChunk> = chunks_map.into_values().collect();
        chunks.sort_by_key(|c| c.order);

        // Optional: Sub-divide massive chunks if they exceed self.config.max_files_per_pr
        let mut final_chunks = Vec::new();
        for chunk in chunks {
            for (i, sub_batch) in chunk
                .files
                .chunks(self.config.max_files_per_pr as usize)
                .enumerate()
            {
                let suffix = if chunk.files.len() > self.config.max_files_per_pr as usize {
                    format!("_part{}", i + 1)
                } else {
                    String::new()
                };
                final_chunks.push(StackChunk {
                    order: chunk.order, // Subparts maintain same general order sequence
                    name: format!("{}{}", chunk.name, suffix),
                    files: sub_batch.to_vec(),
                });
            }
        }

        StackManifest {
            generated_at: chrono::Utc::now().to_rfc3339(),
            chunks: final_chunks,
            total_files,
        }
    }
}
