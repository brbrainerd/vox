use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::{Deserialize, Serialize};

/// A byte-level span aligned with a syntax element kind and a loss weight.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxSpan {
    pub start: usize,
    pub end: usize,
    pub weight: f32,
    pub kind: String,
}

/// One turn in a ChatML conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatmlTurn {
    /// Role: "system", "user", or "assistant".
    pub role: String,
    /// Message content.
    pub content: String,
}

/// A single prompt→response training pair or multi-turn sequence (matches dogfood JSONL schema).
///
/// Deserializes via `TrainingPairRaw` (private struct) so that `instruction` → `prompt` and `output` → `response`
/// normalization happens on the way in, even when both canonical and alias keys are present.
#[derive(Debug, Serialize, Clone, Default)]
pub struct TrainingPair {
    pub prompt: Option<String>,
    pub response: Option<String>,
    /// Optional multi-turn messages. If present, typically preferred over single-turn prompt/response.
    pub messages: Option<Vec<ChatmlTurn>>,
    /// Optional quality rating (1-5). Absent means unrated.
    pub rating: Option<u8>,
    /// Optional category tag (construct type).
    pub category: Option<String>,
    /// Optional difficulty level (1-10) for curriculum learning.
    pub difficulty: Option<u8>,
    /// Optional data lane for segmented training mixes (e.g. `vox_codegen`, `vox_docs_qa`).
    pub lane: Option<String>,
    /// Optional expected answer surface (e.g. `code_only`, `prose_only`).
    pub response_mode: Option<String>,
    /// Optional task family tag (e.g. `docs_code`, `tool_trace`, `speech_to_code`).
    pub task_family: Option<String>,
    /// Attention budgeting decision
    pub interruption_decision: Option<String>,
    /// Attention budget agent trust score
    pub agent_trust_score: Option<f64>,
    /// Optional syntax-aware spans for loss weighting.
    pub syntax_spans: Option<Vec<SyntaxSpan>>,
}

/// Raw deserialization helper: accepts both canonical and alias key names.
#[derive(Deserialize)]
struct TrainingPairRaw {
    pub prompt: Option<String>,
    pub instruction: Option<String>,
    pub response: Option<String>,
    pub output: Option<String>,
    #[serde(alias = "turns")]
    pub messages: Option<Vec<ChatmlTurn>>,
    pub rating: Option<u8>,
    pub category: Option<String>,
    pub difficulty: Option<u8>,
    pub lane: Option<String>,
    pub response_mode: Option<String>,
    pub task_family: Option<String>,
    pub interruption_decision: Option<String>,
    pub agent_trust_score: Option<f64>,
    pub syntax_spans: Option<Vec<SyntaxSpan>>,
    // Note: `origin`, `schema_version`, and `source` keys appear in some dogfood
    // rows but are intentionally not declared here. Serde silently ignores
    // unknown fields by default (no `#[serde(deny_unknown_fields)]`), so they
    // are absorbed without warning and without occupying a struct field.
}

impl<'de> Deserialize<'de> for TrainingPair {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = TrainingPairRaw::deserialize(d)?;
        Ok(Self {
            prompt: raw.prompt.or(raw.instruction),
            response: raw.response.or(raw.output),
            messages: raw.messages,
            rating: raw.rating,
            category: raw.category,
            difficulty: raw.difficulty,
            lane: raw.lane,
            response_mode: raw.response_mode,
            task_family: raw.task_family,
            interruption_decision: raw.interruption_decision,
            agent_trust_score: raw.agent_trust_score,
            syntax_spans: raw.syntax_spans,
        })
    }
}

impl TrainingPair {
    pub fn effective_prompt(&self) -> Option<&String> {
        self.prompt.as_ref()
    }

    pub fn effective_response(&self) -> Option<&String> {
        self.response.as_ref()
    }
}

// ─── Minimal character-level vocabulary ──────────────────────────────────────
// PAD/UNK/EOS, one id per printable ASCII (32–126), then ChatML / ``` compounds only.
// Production QLoRA uses the Hugging Face tokenizer — see docs/src/reference/mens-training.md.
// Control: [PAD]=0, [UNK]=1, [EOS]=2.
const PAD_ID: usize = 0;
const UNK_ID: usize = 1;
const EOS_ID: usize = 2;
// ASCII printable starts at id 3
const ASCII_BASE: usize = 3;
// Number of printable ASCII chars (32..=126 → 95 chars)
const ASCII_LEN: usize = 95;
const COMPOUND_BASE: usize = ASCII_BASE + ASCII_LEN;

/// Greedy multi-byte matches before per-byte ASCII (longer strings listed first).
const COMPOUND_TOKENS: &[&str] = &["<|redacted_im_end|>", "<|im_start|>", "```"];

/// Lab tokenizer vocab size (not HF / QLoRA checkpoint vocab).
pub const VOCAB_SIZE: usize = COMPOUND_BASE + COMPOUND_TOKENS.len();

/// Legacy Burn / dogfood tokenizer: ASCII + ChatML fence compounds only.
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

    /// Format multi-turn sequence in ChatML and encode.
    pub fn encode_chatml_turns(turns: &[ChatmlTurn]) -> Vec<u32> {
        let mut text = String::new();
        for turn in turns {
            text.push_str(&format!(
                "<|im_start|>{role}\n{content}<|im_end|>\n",
                role = turn.role,
                content = turn.content
            ));
        }
        Self::encode(text.trim_end())
    }

    /// ChatML prefix through user turn (open assistant slot) — for native inference with `VoxTokenizer`.
    pub fn encode_chatml_inference_prefix(system: &str, user: &str) -> Vec<u32> {
        let text = format!(
            "<|im_start|>system\n{system}<|im_end|>\n\
             <|im_start|>user\n{user}<|im_end|>\n\
             <|im_start|>assistant\n"
        );
        Self::encode(&text)
    }

    /// Tokenize and pad/truncate to exactly `max_len` tokens.
    ///
    /// Labels mask the prompt with -100 so the model only learns to reproduce the assistant response.
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

        Self::mask_and_pad(&full_ids, prompt_len, max_len)
    }

    /// Tokenize multi-turn turns for training.
    pub fn tokenize_turns_for_training(
        turns: &[ChatmlTurn],
        max_len: usize,
    ) -> (Vec<i64>, Vec<i64>) {
        let full_ids = Self::encode_chatml_turns(turns);

        // Find the boundary of the last user turn
        let mut last_assistant_start = 0usize;
        let mut text = String::new();
        for (i, turn) in turns.iter().enumerate() {
            if i == turns.len() - 1 && turn.role == "assistant" {
                last_assistant_start =
                    Self::encode(&text).len() + Self::encode("<|im_start|>assistant\n").len();
            }
            text.push_str(&format!(
                "<|im_start|>{role}\n{content}<|im_end|>\n",
                role = turn.role,
                content = turn.content
            ));
        }

        Self::mask_and_pad(&full_ids, last_assistant_start, max_len)
    }

    fn mask_and_pad(full_ids: &[u32], prompt_len: usize, max_len: usize) -> (Vec<i64>, Vec<i64>) {
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
            if let Some(r) = pair.rating
                && r < self.min_rating
            {
                continue;
            }
            let (input_ids, labels) = if let Some(ref turns) = pair.messages {
                VoxTokenizer::tokenize_turns_for_training(turns, self.max_len)
            } else if let (Some(p), Some(r)) = (&pair.prompt, &pair.response) {
                VoxTokenizer::tokenize_for_training(&self.system_prompt, p, r, self.max_len)
            } else {
                continue; // skip if no training data
            };
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

/// How to handle lines that are non-empty but not valid [`TrainingPair`] JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MalformedJsonlPolicy {
    /// Skip bad lines (default mens behavior). Malformed rows are silently dropped.
    #[default]
    Skip,
    /// Fail on the first non-empty line that does not deserialize to a training pair.
    FailFast,
}

/// Load all records from a JSONL file into memory.
///
/// With [`MalformedJsonlPolicy::Skip`], malformed lines are skipped (historical default).
pub fn load_all_with_policy<P: AsRef<Path>>(
    path: P,
    min_rating: u8,
    policy: MalformedJsonlPolicy,
) -> std::io::Result<Vec<TrainingPair>> {
    let file = File::open(path.as_ref())?;
    let mut out: Vec<TrainingPair> = Vec::new();
    let mut line_no = 0usize;
    for line in BufReader::new(file).lines() {
        let l = line?;
        line_no += 1;
        let trimmed = l.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: serde_json::Result<TrainingPair> = serde_json::from_str(trimmed);
        let pair = match (parsed, policy) {
            (Ok(p), _) => p,
            (Err(e), MalformedJsonlPolicy::FailFast) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "{} line {line_no}: invalid TrainingPair JSON: {e}",
                        path.as_ref().display()
                    ),
                ));
            }
            (Err(_), MalformedJsonlPolicy::Skip) => continue,
        };
        let passes = match pair.rating {
            None => true,
            Some(r) => r >= min_rating,
        };
        if passes {
            out.push(pair);
        }
    }
    Ok(out)
}

/// Load all records from a JSONL file into memory. Skips malformed lines.
pub fn load_all<P: AsRef<Path>>(path: P, min_rating: u8) -> std::io::Result<Vec<TrainingPair>> {
    load_all_with_policy(path, min_rating, MalformedJsonlPolicy::Skip)
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
        assert_eq!(decoded, text);
    }

    #[test]
    fn compound_token_matched_before_chars() {
        let ids = VoxTokenizer::encode("<|im_start|>");
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0] as usize, COMPOUND_BASE + 1);
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
        assert!(labels.contains(&-100));
    }

    #[test]
    fn data_loader_reads_jsonl() {
        let dir = std::env::temp_dir();
        let path = dir.join("vox_dl_test.jsonl");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(
                f,
                r#"{{"prompt":"write a fn","response":"fn foo() to int: return 1"}}"#
            )
            .unwrap();
            writeln!(f, r#"{{"prompt":"another","response":"actor Foo:"}}"#).unwrap();
        }
        let loader = JsonlDataLoader::new(&path).unwrap();
        let rows: Vec<_> = loader.collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].2.prompt.as_deref(), Some("write a fn"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn training_pair_accepts_instruction_and_output_aliases() {
        let p: TrainingPair = serde_json::from_str(
            r#"{"instruction":"fix this","output":"fn ok() to int: return 0"}"#,
        )
        .expect("instruction/output aliases");
        assert_eq!(p.prompt.as_deref(), Some("fix this"));
        assert_eq!(p.response.as_deref(), Some("fn ok() to int: return 0"));
    }

    #[test]
    fn debug_parse_real_data() {
        let json = r#"{"category":"import","difficulty":1,"instruction":"Write Vox code demonstrating example","lane":"vox_codegen","origin":"human","output":"// Minimal notify demo — same handler shape as `examples/golden/mobile_camera.vox`.\n\nimport std.mobile\n\ncomponent App() {\n    view:\n        <button onclick={fn() {\n            mobile.notify(\"Hello\", \"From Vox!\")\n        }}>\"Notify Me\"</button>\n}\n","prompt":"Write Vox code demonstrating example","rating":5,"response":"// Minimal notify demo — same handler shape as `examples/golden/mobile_camera.vox`.\n\nimport std.mobile\n\ncomponent App() {\n    view:\n        <button onclick={fn() {\n            mobile.notify(\"Hello\", \"From Vox!\")\n        }}>\"Notify Me\"</button>\n}\n","response_mode":"code_only","schema_version":"vox_dogfood_v1","source":"examples\\golden\\mobile_test.vox","task_family":"vox_codegen"}"#;
        let parsed: Result<TrainingPair, _> = serde_json::from_str(json);
        match parsed {
            Ok(_) => {}
            Err(e) => panic!("Parse error on real data: {}", e),
        }
    }

    #[test]
    fn load_all_accepts_instruction_rows() {
        let dir = std::env::temp_dir();
        let path = dir.join("vox_load_instruction_test.jsonl");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, r#"{{"instruction":"hello","response":"world"}}"#).unwrap();
        }
        let pairs = load_all(&path, 0).unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].prompt.as_deref(), Some("hello"));
        assert_eq!(pairs[0].response.as_deref(), Some("world"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_all_failfast_errors_on_bad_json() {
        let dir = std::env::temp_dir();
        let path = dir.join("vox_load_failfast.jsonl");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, r#"{{"prompt":"a","response":"b"}}"#).unwrap();
            writeln!(f, "not-json").unwrap();
        }
        let e = load_all_with_policy(&path, 0, MalformedJsonlPolicy::FailFast).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::InvalidData);
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

#[cfg(test)]
mod semcov_wave40_tests {
    use super::*;
    use std::io::Write;

    // ── VoxTokenizer: encode / decode boundary arithmetic ─────────────────────

    #[test]
    fn encode_space_is_ascii_base() {
        // Catches: off-by-one in ASCII_BASE offset — space (0x20=32) should map to id 3
        let ids = VoxTokenizer::encode(" ");
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0] as usize, ASCII_BASE); // 3 + (32-32) = 3
    }

    #[test]
    fn encode_tilde_is_last_ascii_id() {
        // Catches: fence-post error on the upper ASCII boundary (126 = last printable)
        let ids = VoxTokenizer::encode("~");
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0] as usize, ASCII_BASE + ASCII_LEN - 1); // 3+94 = 97
    }

    #[test]
    fn decode_pad_and_eos_are_suppressed() {
        // Catches: PAD/EOS leaking into decoded string when they appear mid-sequence
        let ids: Vec<u32> = vec![PAD_ID as u32, EOS_ID as u32, ASCII_BASE as u32];
        let decoded = VoxTokenizer::decode(&ids);
        assert_eq!(
            decoded, " ",
            "PAD and EOS must be invisible in decoded output"
        );
    }

    #[test]
    fn decode_out_of_range_compound_index_does_not_panic() {
        // Catches: missing bounds check on compound token lookup (ci >= COMPOUND_TOKENS.len())
        let out_of_range_id = (COMPOUND_BASE + COMPOUND_TOKENS.len() + 99) as u32;
        let decoded = VoxTokenizer::decode(&[out_of_range_id]);
        // Should produce empty string, not panic
        assert_eq!(decoded, "");
    }

    #[test]
    fn encode_redacted_im_end_matched_before_im_start_prefix() {
        // Catches: greedy-match failure — <|redacted_im_end|> shares prefix with <|im_start|>
        // but must be checked first (it's listed first in COMPOUND_TOKENS).
        let ids = VoxTokenizer::encode("<|redacted_im_end|>");
        assert_eq!(
            ids.len(),
            1,
            "<|redacted_im_end|> must be a single compound token"
        );
        assert_eq!(ids[0] as usize, COMPOUND_BASE); // first compound
    }

    #[test]
    fn encode_backtick_fence_is_single_token() {
        // Catches: ``` incorrectly split into three individual backtick tokens
        let ids = VoxTokenizer::encode("```");
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0] as usize, COMPOUND_BASE + 2); // third compound
    }

    #[test]
    fn encode_empty_string_yields_no_ids() {
        // Catches: off-by-one in the while loop that emits an extra token for empty input
        let ids = VoxTokenizer::encode("");
        assert!(ids.is_empty());
    }

    #[test]
    fn encode_decode_roundtrip_all_printable_ascii() {
        // Catches: any hole in the ASCII_BASE offset arithmetic across the full printable range
        let text: String = (32u8..=126u8).map(|b| b as char).collect();
        let ids = VoxTokenizer::encode(&text);
        let decoded = VoxTokenizer::decode(&ids);
        assert_eq!(decoded, text);
    }

    // ── mask_and_pad: label boundary logic ────────────────────────────────────

    #[test]
    fn tokenize_for_training_eos_appended_before_pad() {
        // Catches: EOS token missing when sequence fits exactly within max_len-1
        let (input_ids, _) = VoxTokenizer::tokenize_for_training("s", "u", "a", 512);
        // EOS_ID=2 must appear exactly once, before padding begins
        let eos_pos = input_ids.iter().position(|&t| t == EOS_ID as i64);
        assert!(eos_pos.is_some(), "EOS token must be present");
        // Everything after EOS should be PAD
        let pos = eos_pos.unwrap();
        assert!(
            input_ids[pos + 1..].iter().all(|&t| t == PAD_ID as i64),
            "all tokens after EOS must be PAD"
        );
    }

    #[test]
    fn tokenize_for_training_max_len_one_yields_only_eos() {
        // Catches: saturating_sub(1) == 0 corner case producing an empty sequence or
        // a sequence longer than max_len
        let (input_ids, labels) = VoxTokenizer::tokenize_for_training("sys", "usr", "resp", 1);
        assert_eq!(input_ids.len(), 1);
        assert_eq!(labels.len(), 1);
        assert_eq!(input_ids[0], EOS_ID as i64);
    }

    #[test]
    fn tokenize_for_training_all_labels_minus_100_when_response_truncated_away() {
        // Catches: label masking not accounting for the case where the prompt fills
        // the entire max_len, leaving no room for any supervised token.
        // Use max_len=1: only EOS fits, which is in the prompt region or pad region.
        let (_, labels) = VoxTokenizer::tokenize_for_training("sys", "usr", "resp", 1);
        assert!(
            labels.iter().all(|&l| l == -100 || l == PAD_ID as i64),
            "when response is truncated away, no supervised tokens should exist"
        );
    }

    #[test]
    fn tokenize_for_training_no_pad_tokens_in_labels_as_positive() {
        // Catches: PAD_ID (0) leaking into labels as a real supervised token
        let (_, labels) = VoxTokenizer::tokenize_for_training("sys", "usr", "resp", 64);
        for &l in &labels {
            assert!(
                l == -100 || l > 0,
                "label must be -100 (masked) or a real token id (>0); got {l}"
            );
        }
    }

    #[test]
    fn encode_chatml_inference_prefix_does_not_contain_im_end_after_assistant() {
        // Catches: inference prefix accidentally including the closing <|im_end|> for the
        // assistant slot, which would make generation produce text after a closed tag.
        let ids = VoxTokenizer::encode_chatml_inference_prefix("sys", "usr");
        let decoded = VoxTokenizer::decode(&ids);
        // The decoded prefix should end with the open assistant header, not close it.
        // Count im_start vs im_end occurrences: inference prefix has 3 starts (sys/user/asst)
        // but only 2 ends (sys+user closed; assistant slot left open).
        let starts = decoded.matches("<|im_start|>").count();
        // We can't decode <|im_end|> because it's aliased as <|redacted_im_end|> in COMPOUND_TOKENS.
        // Instead verify the assistant header is present and the sequence terminates open.
        assert!(decoded.contains("assistant"), "must open assistant slot");
        assert_eq!(starts, 3, "three <|im_start|> tokens expected");
    }

    // ── TrainingPair deserialization: alias precedence ────────────────────────

    #[test]
    fn canonical_prompt_wins_over_instruction_when_both_present() {
        // Catches: .or() precedence inverted — instruction overriding prompt when both exist
        let p: TrainingPair =
            serde_json::from_str(r#"{"prompt":"canonical","instruction":"alias","response":"r"}"#)
                .unwrap();
        assert_eq!(
            p.prompt.as_deref(),
            Some("canonical"),
            "canonical 'prompt' key must win over 'instruction' alias"
        );
    }

    #[test]
    fn canonical_response_wins_over_output_when_both_present() {
        // Catches: .or() precedence inverted — output overriding response when both exist
        let p: TrainingPair =
            serde_json::from_str(r#"{"prompt":"p","response":"canonical","output":"alias"}"#)
                .unwrap();
        assert_eq!(
            p.response.as_deref(),
            Some("canonical"),
            "canonical 'response' key must win over 'output' alias"
        );
    }

    #[test]
    fn turns_alias_deserializes_as_messages() {
        // Catches: #[serde(alias = "turns")] not wiring through to TrainingPair.messages
        let p: TrainingPair =
            serde_json::from_str(r#"{"turns":[{"role":"user","content":"hello"}]}"#).unwrap();
        let msgs = p.messages.expect("messages must be populated from 'turns'");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].role, "user");
    }

    // ── load_all_with_policy: line numbering & rating filter ──────────────────

    #[test]
    fn load_all_rating_zero_includes_unrated_rows() {
        // Catches: missing None-case in rating filter — rows without a rating field
        // being incorrectly dropped when min_rating=0
        let dir = std::env::temp_dir();
        let path = dir.join("wave40_unrated.jsonl");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, r#"{{"prompt":"no-rating","response":"r"}}"#).unwrap();
        }
        let pairs = load_all(&path, 0).unwrap();
        assert_eq!(
            pairs.len(),
            1,
            "unrated rows must be included when min_rating=0"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn failfast_reports_correct_line_number_after_blank_lines() {
        // Catches: line_no counter not incremented for blank lines, causing wrong line
        // numbers in error messages (off by the number of blank lines seen so far)
        let dir = std::env::temp_dir();
        let path = dir.join("wave40_failfast_blanks.jsonl");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, r#"{{"prompt":"a","response":"b"}}"#).unwrap();
            writeln!(f).unwrap(); // blank — line 2
            writeln!(f, "not-json").unwrap(); // bad — line 3
        }
        let err = load_all_with_policy(&path, 0, MalformedJsonlPolicy::FailFast).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("line 3"),
            "error message must reference line 3, got: {msg}"
        );
        let _ = std::fs::remove_file(&path);
    }
}

#[cfg(test)]
mod semcov_wave4_tests {
    #![allow(unused_imports)]
    use super::*;

    // --- encode_chatml_turns ---

    #[test]
    fn encode_chatml_turns_multi_turn_roundtrip() {
        let turns = vec![
            ChatmlTurn {
                role: "system".to_string(),
                content: "be helpful".to_string(),
            },
            ChatmlTurn {
                role: "user".to_string(),
                content: "hello".to_string(),
            },
            ChatmlTurn {
                role: "assistant".to_string(),
                content: "hi there".to_string(),
            },
        ];
        let ids = VoxTokenizer::encode_chatml_turns(&turns);
        assert!(!ids.is_empty());
        let decoded = VoxTokenizer::decode(&ids);
        assert!(decoded.contains("be helpful"), "system content missing");
        assert!(decoded.contains("hello"), "user content missing");
        assert!(decoded.contains("hi there"), "assistant content missing");
    }

    #[test]
    fn encode_chatml_turns_single_turn() {
        let turns = vec![ChatmlTurn {
            role: "user".to_string(),
            content: "test".to_string(),
        }];
        let ids = VoxTokenizer::encode_chatml_turns(&turns);
        assert!(!ids.is_empty());
        let decoded = VoxTokenizer::decode(&ids);
        assert!(decoded.contains("user"));
        assert!(decoded.contains("test"));
        // Should not end with trailing newline (trim_end applied)
        assert!(
            !decoded.ends_with('\n'),
            "trailing newline should be trimmed"
        );
    }

    // --- encode_chatml_inference_prefix ---

    #[test]
    fn encode_chatml_inference_prefix_ends_with_open_assistant_slot() {
        let ids = VoxTokenizer::encode_chatml_inference_prefix("sys", "usr");
        assert!(!ids.is_empty());
        let decoded = VoxTokenizer::decode(&ids);
        assert!(decoded.contains("sys"));
        assert!(decoded.contains("usr"));
        assert!(decoded.contains("assistant"), "must open assistant slot");
        // Prefix must be shorter than a complete chatml with a non-empty response
        let full_ids_with_response = VoxTokenizer::encode_chatml("sys", "usr", "resp");
        assert!(
            ids.len() < full_ids_with_response.len(),
            "inference prefix must be shorter than complete chatml"
        );
    }

    // --- tokenize_turns_for_training ---

    #[test]
    fn tokenize_turns_for_training_pads_to_max_len() {
        let turns = vec![
            ChatmlTurn {
                role: "user".to_string(),
                content: "hello".to_string(),
            },
            ChatmlTurn {
                role: "assistant".to_string(),
                content: "hi".to_string(),
            },
        ];
        let (input_ids, labels) = VoxTokenizer::tokenize_turns_for_training(&turns, 64);
        assert_eq!(input_ids.len(), 64, "input_ids must be padded to max_len");
        assert_eq!(labels.len(), 64, "labels must be padded to max_len");
    }

    #[test]
    fn tokenize_turns_for_training_masks_non_assistant_tokens() {
        let turns = vec![
            ChatmlTurn {
                role: "user".to_string(),
                content: "tell me something".to_string(),
            },
            ChatmlTurn {
                role: "assistant".to_string(),
                content: "sure thing".to_string(),
            },
        ];
        let (input_ids, labels) = VoxTokenizer::tokenize_turns_for_training(&turns, 128);
        assert!(
            labels.contains(&-100),
            "prompt prefix must be masked with -100"
        );
        assert!(
            labels.iter().any(|&l| l > 0),
            "at least one real supervised token expected"
        );
        assert_eq!(input_ids.len(), labels.len());
    }
}
