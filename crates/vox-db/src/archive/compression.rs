//! zstd compression with optional prepared dictionary (design §4.1, Rev 2 §Correction 5).

use crate::store::types::StoreError;

const ZSTD_LEVEL_DEFAULT: i32 = 12;

pub fn compress_prepared(
    data: &[u8],
    dict: Option<&zstd::dict::EncoderDictionary<'_>>,
) -> Result<Vec<u8>, StoreError> {
    let mut c = match dict {
        Some(ed) => zstd::bulk::Compressor::with_prepared_dictionary(ed),
        None => zstd::bulk::Compressor::new(ZSTD_LEVEL_DEFAULT),
    }
    .map_err(|e| StoreError::Db(format!("zstd compressor: {e}")))?;
    c.compress(data)
        .map_err(|e| StoreError::Db(format!("zstd compress: {e}")))
}

pub fn decompress_prepared(
    data: &[u8],
    capacity: usize,
    dict: Option<&zstd::dict::DecoderDictionary<'_>>,
) -> Result<Vec<u8>, StoreError> {
    let mut d = match dict {
        Some(dd) => zstd::bulk::Decompressor::with_prepared_dictionary(dd),
        None => zstd::bulk::Decompressor::new(),
    }
    .map_err(|e| StoreError::Db(format!("zstd decompressor: {e}")))?;
    d.decompress(data, capacity)
        .map_err(|e| StoreError::Db(format!("zstd decompress: {e}")))
}

pub fn compress(data: &[u8], dict: Option<&[u8]>) -> Result<Vec<u8>, StoreError> {
    match dict {
        Some(d) => {
            let enc_dict = zstd::dict::EncoderDictionary::copy(d, ZSTD_LEVEL_DEFAULT);
            compress_prepared(data, Some(&enc_dict))
        }
        None => compress_prepared(data, None),
    }
}

pub fn decompress(data: &[u8], capacity: usize, dict: Option<&[u8]>) -> Result<Vec<u8>, StoreError> {
    match dict {
        Some(d) => {
            let dec_dict = zstd::dict::DecoderDictionary::copy(d);
            decompress_prepared(data, capacity, Some(&dec_dict))
        }
        None => decompress_prepared(data, capacity, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_no_dict() {
        let data = b"the quick brown fox jumps over the lazy dog".repeat(50);
        let comp = compress(&data, None).unwrap();
        assert!(comp.len() < data.len(), "should shrink repetitive data");
        let back = decompress(&comp, data.len(), None).unwrap();
        assert_eq!(back, data);
    }

    #[test]
    fn round_trip_with_dict() {
        let dict_bytes = b"context window archive session message tool output ".repeat(20);
        let data = b"context window archive session foo bar".to_vec();
        let comp = compress(&data, Some(&dict_bytes)).unwrap();
        let back = decompress(&comp, data.len(), Some(&dict_bytes)).unwrap();
        assert_eq!(back, data);
    }

    #[test]
    fn prepared_dict_round_trip() {
        let dict_bytes = b"context window archive session message tool output ".repeat(20);
        let enc = zstd::dict::EncoderDictionary::copy(&dict_bytes, 12);
        let dec = zstd::dict::DecoderDictionary::copy(&dict_bytes);
        let data = b"context window archive session foo bar".to_vec();
        let comp = compress_prepared(&data, Some(&enc)).unwrap();
        let back = decompress_prepared(&comp, data.len(), Some(&dec)).unwrap();
        assert_eq!(back, data);
    }
}
