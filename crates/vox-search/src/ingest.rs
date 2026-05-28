//! Mirror repository markdown trees into `vox-db` `search_documents` / chunks.

use std::path::Path;

use walkdir::WalkDir;

use vox_db::{StoreError, VoxDb};

fn blake3_hex(data: &[u8]) -> String {
    blake3::hash(data).to_hex().to_string()
}

/// Upsert a single document body as one chunk row (used by web retrieval mirrors).
pub async fn persist_text_document_chunk(
    db: &VoxDb,
    source_uri: &str,
    title: &str,
    body: &str,
    mime: &str,
) -> Result<(), StoreError> {
    let hash = blake3_hex(body.as_bytes());
    let doc_id = db
        .upsert_search_document(source_uri, title, mime, &hash)
        .await?;
    db.replace_search_document_chunks_with_refs(doc_id, &[body.to_string()], &[None])
        .await?;
    Ok(())
}

/// Ingest all `.md` files under `root` (e.g. `docs/src`) into the searchable corpus.
///
/// `source_uri_prefix` becomes the `search_documents.source_uri` prefix (e.g. `vox-docs:`).
pub async fn ingest_markdown_tree(
    db: &VoxDb,
    root: &Path,
    source_uri_prefix: &str,
) -> Result<usize, StoreError> {
    if !root.is_dir() {
        return Ok(0);
    }
    let mut count = 0usize;
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let body = match vox_bounded_fs::read_utf8_path_capped(path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let rel = path.strip_prefix(root).unwrap_or(path);
        let rel_slash = rel.to_string_lossy().replace('\\', "/");
        let source_uri = format!("{source_uri_prefix}{rel_slash}");
        let title = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| rel_slash.clone());
        let hash = blake3_hex(body.as_bytes());
        let doc_id = db
            .upsert_search_document(&source_uri, &title, "text/markdown", &hash)
            .await?;
        let chunks = chunk_markdown_sections(&body);
        let refs: Vec<Option<String>> = vec![None; chunks.len()];
        db.replace_search_document_chunks_with_refs(doc_id, &chunks, &refs)
            .await?;
        count += 1;
    }
    Ok(count)
}

fn chunk_markdown_sections(text: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut cur = String::new();
    for line in text.lines() {
        if line.starts_with("# ") || line.starts_with("## ") {
            if !cur.trim().is_empty() {
                chunks.push(cur.trim().to_string());
            }
            cur = format!("{line}\n");
        } else {
            cur.push_str(line);
            cur.push('\n');
            if cur.len() > 4096 {
                chunks.push(cur.trim().to_string());
                cur.clear();
            }
        }
    }
    if !cur.trim().is_empty() {
        chunks.push(cur.trim().to_string());
    }
    if chunks.is_empty() && !text.trim().is_empty() {
        chunks.push(text.trim().to_string());
    }
    chunks
}
