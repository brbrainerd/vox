//! Hybrid chunking: small items pass through whole; large items split via FastCDC (design §3).

/// Items at or above this byte length are content-defined-chunked; smaller items are whole.
pub const LARGE_ITEM_THRESHOLD: usize = 4 * 1024;

const MIN_CHUNK: usize = 4 * 1024;
const AVG_CHUNK: usize = 8 * 1024;
const MAX_CHUNK: usize = 32 * 1024;

/// Split `content` into chunks. Returns a single-element vec (the whole content) when
/// `content.len() < LARGE_ITEM_THRESHOLD`; otherwise FastCDC content-defined chunks whose
/// concatenation equals `content` exactly.
pub fn chunk_content(content: &[u8]) -> Vec<Vec<u8>> {
    if content.len() < LARGE_ITEM_THRESHOLD {
        return vec![content.to_vec()];
    }
    fastcdc::v2020::FastCDC::new(content, MIN_CHUNK, AVG_CHUNK, MAX_CHUNK)
        .map(|c| content[c.offset..c.offset + c.length].to_vec())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_item_is_one_chunk() {
        let data = vec![7u8; 100];
        let chunks = chunk_content(&data);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], data);
    }

    #[test]
    fn large_item_splits_and_reassembles_exactly() {
        // 200 KB of varied content so CDC finds multiple boundaries.
        let data: Vec<u8> = (0..200_000).map(|i| (i * 2654435761usize) as u8).collect();
        let chunks = chunk_content(&data);
        assert!(
            chunks.len() > 1,
            "expected multiple chunks, got {}",
            chunks.len()
        );
        let rejoined: Vec<u8> = chunks.concat();
        assert_eq!(rejoined, data, "concatenated chunks must equal original");
    }
}
