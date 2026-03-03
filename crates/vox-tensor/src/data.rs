use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::Deserialize;

/// A single prompt→response training pair (matches dogfood JSONL schema).
#[derive(Debug, Deserialize, Clone)]
pub struct TrainingPair {
    pub prompt: String,
    pub response: String,
    /// Optional quality rating (1-5). Absent means unrated.
    pub rating: Option<u8>,
    /// Optional category tag (construct type).
    pub category: Option<String>,
}

// ─── Minimal character-level vocabulary ──────────────────────────────────────
// We build a deterministic vocab: all printable ASCII characters (32-126)
// get their own token, plus a handful of Vox compound-keyword tokens,
// plus three control tokens: [PAD]=0, [UNK]=1, [EOS]=2.
const PAD_ID: usize = 0;
const UNK_ID: usize = 1;
const EOS_ID: usize = 2;
// ASCII printable starts at id 3
const ASCII_BASE: usize = 3;
// Number of printable ASCII chars (32..=126 → 95 chars)
const ASCII_LEN: usize = 95;
const COMPOUND_BASE: usize = ASCII_BASE + ASCII_LEN;

/// Compound (multi-char) tokens specific to Vox constructs.
/// Order is significant — earlier tokens are tried first.
const COMPOUND_TOKENS: &[&str] = &[
    // Vox compound keywords
    "workflow",
    "activity",
    "ret ",
    "let ",
    "actor ",
    "fn ",
    "type ",
    "import ",
    "spawn(",
    "match ",
    "with {",
    "->",
    "=>",
    "::",
    "..",
    "!=",
    "==",
    ">=",
    "<=",
    "<|im_start|>",
    "<|im_end|>",
    "```",
    "@mcp",
    "@table",
    "@query",
    "@mutation",
    "@action",
    "@server",
    "@test",
    "@component",
    "@agent_def",
    "@skill",
    "@v0",
];

/// Total vocabulary size.
pub const VOCAB_SIZE: usize = COMPOUND_BASE + COMPOUND_TOKENS.len();

/// A deterministic, dependency-free character-level tokenizer for Vox source code.
///
/// Longer compound tokens (Vox keywords, ChatML markers) are matched greedily
/// before falling back to individual ASCII characters. Non-ASCII bytes map to UNK.
pub struct VoxTokenizer;

impl VoxTokenizer {
    /// Encode a string into a sequence of token IDs.
    ///
    /// Greedy longest-match on compound tokens first, then single-char ASCII,
    /// then UNK for anything else.
    pub fn encode(text: &str) -> Vec<u32> {
        let mut ids: Vec<u32> = Vec::with_capacity(text.len());
        let bytes = text.as_bytes();
        let mut pos = 0;
        while pos < bytes.len() {
            // Try compound tokens (longest first)
            let mut matched = false;
            for (ci, &compound) in COMPOUND_TOKENS.iter().enumerate() {
                let cb = compound.as_bytes();
                if bytes[pos..].starts_with(cb) {
                    ids.push((COMPOUND_BASE + ci) as u32);
                    pos += cb.len();
                    matched = true;
                    break;
                }
            }
            if matched {
                continue;
            }
            // Single-char: printable ASCII (space=32, tilde=126)
            let byte = bytes[pos];
            if (32..=126).contains(&byte) {
                ids.push((ASCII_BASE + (byte as usize - 32)) as u32);
            } else {
                ids.push(UNK_ID as u32);
            }
            pos += 1;
        }
        ids
    }

    /// Decode token IDs back into a String (best-effort, for debugging).
    pub fn decode(ids: &[u32]) -> String {
        let mut out = String::new();
        for &id in ids {
            let id = id as usize;
            if id == PAD_ID || id == EOS_ID {
                continue;
            }
            if id == UNK_ID {
                out.push('\u{FFFD}');
            } else if id >= COMPOUND_BASE {
                let ci = id - COMPOUND_BASE;
                if ci < COMPOUND_TOKENS.len() {
                    out.push_str(COMPOUND_TOKENS[ci]);
                }
            } else if id >= ASCII_BASE {
                let ch = (id - ASCII_BASE + 32) as u8 as char;
                out.push(ch);
            }
        }
        out
    }

    /// Format in ChatML and encode to token IDs for training.
    pub fn encode_chatml(system: &str, user: &str, assistant: &str) -> Vec<u32> {
        let text = format!(
            "<|im_start|>system\n{system}<|im_end|>\n\
             <|im_start|>user\n{user}<|im_end|>\n\
             <|im_start|>assistant\n{assistant}<|im_end|>"
        );
        Self::encode(&text)
    }

    /// Tokenize and pad/truncate to exactly `max_len` tokens.
    ///
    /// Returns `(input_ids, labels)` where labels mask the prompt with -100 so
    /// the model only learns to reproduce the assistant response.
    pub fn tokenize_for_training(
        system: &str,
        user: &str,
        assistant: &str,
        max_len: usize,
    ) -> (Vec<i64>, Vec<i64>) {
        let full_ids = Self::encode_chatml(system, user, assistant);

        // Compute where the assistant prefix ends — that's the supervision boundary
        let prompt_text = format!(
            "<|im_start|>system\n{system}<|im_end|>\n\
             <|im_start|>user\n{user}<|im_end|>\n\
             <|im_start|>assistant\n"
        );
        let prompt_len = Self::encode(&prompt_text).len();

        // Truncate if needed, leaving room for EOS
        let truncated: Vec<i64> = full_ids
            .iter()
            .take(max_len.saturating_sub(1))
            .map(|&x| x as i64)
            .chain(std::iter::once(EOS_ID as i64))
            .collect();

        let actual_len = truncated.len();

        // Pad to max_len
        let mut input_ids = truncated;
        input_ids.resize(max_len, PAD_ID as i64);

        // Labels: mask prompt tokens and padding with -100
        let labels: Vec<i64> = input_ids
            .iter()
            .enumerate()
            .map(|(i, &tok)| {
                if i < prompt_len || i >= actual_len || tok == PAD_ID as i64 {
                    -100
                } else {
                    tok
                }
            })
            .collect();

        (input_ids, labels)
    }
}

// ─── DataLoader ───────────────────────────────────────────────────────────────

/// A sequential iterator over a JSONL training file.
///
/// Yields `(input_tensor, label_tensor)` pairs encoded with `VoxTokenizer`.
/// Tokens are padded/truncated to `max_len`. The GPU feature flag is not
/// required here — tensors are emitted as 1-D `burn` tensors only when the
/// consumer is compiled with `gpu`.
pub struct JsonlDataLoader {
    reader: BufReader<File>,
    max_len: usize,
    system_prompt: String,
    min_rating: u8,
}

impl JsonlDataLoader {
    /// Open a JSONL training file.
    ///
    /// * `max_len` — maximum sequence length in tokens (default 512 for CPU, 2048 for GPU)
    /// * `system_prompt` — injected as the system role in ChatML format
    /// * `min_rating` — skip rows with `rating < min_rating` (0 = include all)
    pub fn open<P: AsRef<Path>>(
        path: P,
        max_len: usize,
        system_prompt: String,
        min_rating: u8,
    ) -> std::io::Result<Self> {
        let file = File::open(path)?;
        Ok(Self {
            reader: BufReader::new(file),
            max_len,
            system_prompt,
            min_rating,
        })
    }

    /// Convenience: open with sensible CPU defaults (max_len=512, no rating filter).
    pub fn new<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        Self::open(path, 512, default_system_prompt(), 0)
    }
}

fn default_system_prompt() -> String {
    "You are a Vox programming language expert. Generate valid, complete Vox code.".to_string()
}

impl Iterator for JsonlDataLoader {
    /// Yields `(input_ids, labels, pair)` where ids and labels are raw `i64` vecs.
    /// Use `into_tensors()` (gpu feature) to convert to burn tensors.
    type Item = (Vec<i64>, Vec<i64>, TrainingPair);

    fn next(&mut self) -> Option<Self::Item> {
        let mut line = String::new();
        loop {
            line.clear();
            let n = self.reader.read_line(&mut line).ok()?;
            if n == 0 {
                return None; // EOF
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let pair: TrainingPair = match serde_json::from_str(trimmed) {
                Ok(p) => p,
                Err(_) => continue, // skip malformed lines
            };
            // Rating filter
            if let Some(r) = pair.rating {
                if r < self.min_rating {
                    continue;
                }
            }
            let (input_ids, labels) = VoxTokenizer::tokenize_for_training(
                &self.system_prompt,
                &pair.prompt,
                &pair.response,
                self.max_len,
            );
            return Some((input_ids, labels, pair));
        }
    }
}

// ─── stat helpers ─────────────────────────────────────────────────────────────

/// Count the number of non-empty lines in a JSONL file (O(1) RAM).
pub fn count_jsonl_records<P: AsRef<Path>>(path: P) -> std::io::Result<usize> {
    let file = File::open(path)?;
    let mut count = 0usize;
    for line in BufReader::new(file).lines() {
        let l = line?;
        if !l.trim().is_empty() {
            count += 1;
        }
    }
    Ok(count)
}

/// Load all records from a JSONL file into memory. Skips malformed lines.
pub fn load_all<P: AsRef<Path>>(path: P, min_rating: u8) -> std::io::Result<Vec<TrainingPair>> {
    let file = File::open(path)?;
    let mut out: Vec<TrainingPair> = Vec::new();
    for line in BufReader::new(file).lines() {
        let l = line?;
        let trimmed = l.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(pair) = serde_json::from_str::<TrainingPair>(trimmed) {
            if pair.rating.map_or(true, |r| r >= min_rating) {
                out.push(pair);
            }
        }
    }
    Ok(out)
}

// ─── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn encode_ascii_roundtrip() {
        let text = "fn hello() to int:";
        let ids = VoxTokenizer::encode(text);
        assert!(!ids.is_empty());
        let decoded = VoxTokenizer::decode(&ids);
        // fn is a compound token — decoded should still reproduce the original text
        assert_eq!(decoded, text);
    }

    #[test]
    fn compound_token_matched_before_chars() {
        let ids = VoxTokenizer::encode("workflow");
        // Should be a single compound token, not 8 individual chars
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0] as usize, COMPOUND_BASE); // first compound token
    }

    #[test]
    fn encode_chatml_contains_all_roles() {
        let ids = VoxTokenizer::encode_chatml("sys", "user msg", "response");
        assert!(!ids.is_empty());
        let decoded = VoxTokenizer::decode(&ids);
        assert!(decoded.contains("sys"));
        assert!(decoded.contains("user msg"));
        assert!(decoded.contains("response"));
    }

    #[test]
    fn tokenize_for_training_pads_to_max_len() {
        let (input_ids, labels) = VoxTokenizer::tokenize_for_training("S", "U", "A", 32);
        assert_eq!(input_ids.len(), 32);
        assert_eq!(labels.len(), 32);
    }

    #[test]
    fn tokenize_for_training_labels_mask_prompt() {
        let (_, labels) = VoxTokenizer::tokenize_for_training("sys", "usr", "resp", 64);
        // Labels should have at least one real (non -100, non-pad) token
        assert!(labels.iter().any(|&l| l >= 0 && l != PAD_ID as i64));
        // Labels should have -100 for prompt region
        assert!(labels.iter().any(|&l| l == -100));
    }

    #[test]
    fn data_loader_reads_jsonl() {
        let dir = std::env::temp_dir();
        let path = dir.join("vox_dl_test.jsonl");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, r#"{{"prompt":"write a fn","response":"fn foo() to int: ret 1"}}"#).unwrap();
            writeln!(f, r#"{{"prompt":"another","response":"actor Foo:"}}"#).unwrap();
        }
        let loader = JsonlDataLoader::new(&path).unwrap();
        let rows: Vec<_> = loader.collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].2.prompt, "write a fn");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn data_loader_skips_low_rating() {
        let dir = std::env::temp_dir();
        let path = dir.join("vox_dl_rating_test.jsonl");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, r#"{{"prompt":"a","response":"b","rating":5}}"#).unwrap();
            writeln!(f, r#"{{"prompt":"c","response":"d","rating":2}}"#).unwrap();
        }
        let loader = JsonlDataLoader::open(&path, 64, "sys".into(), 3).unwrap();
        let rows: Vec<_> = loader.collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].2.rating, Some(5));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn count_jsonl_records_correct() {
        let dir = std::env::temp_dir();
        let path = dir.join("vox_count_test.jsonl");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            for i in 0..7 {
                writeln!(f, r#"{{"prompt":"p{}","response":"r{}"}}"#, i, i).unwrap();
            }
        }
        let count = count_jsonl_records(&path).unwrap();
        assert_eq!(count, 7);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn vocab_size_constant_matches_actual() {
        // If COMPOUND_TOKENS grows, VOCAB_SIZE must remain accurate
        assert_eq!(VOCAB_SIZE, COMPOUND_BASE + COMPOUND_TOKENS.len());
        assert_eq!(COMPOUND_BASE, ASCII_BASE + ASCII_LEN);
    }

    #[test]
    fn unk_for_non_ascii() {
        // U+0080 is a 2-byte UTF-8 sequence (0xC2, 0x80). The tokenizer is
        // byte-level, so each non-ASCII byte emits one UNK token.
        let ids = VoxTokenizer::encode("\u{0080}"); // non-ASCII byte
        assert_eq!(ids.len(), 2); // 2 bytes → 2 UNK tokens
        assert!(ids.iter().all(|&id| id == UNK_ID as u32));
    }
}
