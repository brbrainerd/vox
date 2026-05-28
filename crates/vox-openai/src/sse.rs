//! UTF-8 SSE `data:` line assembly across arbitrary chunk boundaries and
//! delta extraction from OpenAI-shaped streaming JSON.
//!
//! **`eventsource-stream` / `reqwest-eventsource`:** deferred — we need lossy UTF-8 assembly and
//! OpenAI-shaped `delta.content` without assuming one provider’s event framing; evaluate again if
//! a crate exposes pluggable line assembly compatible with OpenRouter/HF chunking.

use serde_json::Value;

/// Accumulates UTF-8 chunks and invokes `on_line` for each newline-delimited line (CR stripped).
#[derive(Debug, Default)]
pub struct Utf8LineBuffer {
    buf: String,
}

impl Utf8LineBuffer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push lossy-decoded `bytes` and emit complete lines.
    pub fn push_lossy_bytes(&mut self, bytes: &[u8], mut on_line: impl FnMut(&str)) {
        self.buf.push_str(&String::from_utf8_lossy(bytes));
        while let Some(pos) = self.buf.find('\n') {
            let line = self.buf[..pos].trim_end_matches('\r').to_string();
            self.buf.drain(..=pos);
            on_line(line.as_str());
        }
    }

    /// Invoke `on_line` on any trailing fragment that did not end with `\n`, then clear.
    pub fn flush_trailing(&mut self, mut on_line: impl FnMut(&str)) {
        let tail = self.buf.trim();
        if !tail.is_empty() {
            on_line(tail);
        }
        self.buf.clear();
    }
}

/// Parse `data` JSON from one SSE `data: …` line (after the `data: ` prefix) and return delta content, if any.
#[must_use]
pub fn chat_completion_delta_content(data: &str) -> Option<String> {
    let v: Value = serde_json::from_str(data).ok()?;
    v["choices"].get(0)?["delta"]["content"]
        .as_str()
        .map(std::string::ToString::to_string)
}

/// One SSE line: `data: {json}` → non-empty delta content; `[DONE]`, blanks, and non-`data:` lines → `None`.
#[must_use]
pub fn sse_data_line_delta(line: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let payload = line.strip_prefix("data: ")?;
    let payload = payload.trim();
    if payload == "[DONE]" {
        return None;
    }
    chat_completion_delta_content(payload).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_split_across_chunks() {
        let mut b = Utf8LineBuffer::new();
        let mut lines = Vec::new();
        b.push_lossy_bytes(b"data: {\"x\":1}\r\nda", |l| lines.push(l.to_string()));
        assert_eq!(lines.len(), 1);
        lines.clear();
        b.push_lossy_bytes(b"ta: ok\n", |l| lines.push(l.to_string()));
        assert_eq!(lines, vec!["data: ok"]);
    }

    #[test]
    fn delta_extracts_content() {
        let j = r#"{"choices":[{"delta":{"content":"hi"}}]}"#;
        assert_eq!(chat_completion_delta_content(j).as_deref(), Some("hi"));
    }

    #[test]
    fn sse_line_done_and_data_prefix() {
        assert_eq!(sse_data_line_delta(""), None);
        assert_eq!(sse_data_line_delta("event: ping"), None);
        assert_eq!(sse_data_line_delta("data: [DONE]"), None);
        let j = r#"{"choices":[{"delta":{"content":"x"}}]}"#;
        assert_eq!(
            sse_data_line_delta(&format!("data: {j}")).as_deref(),
            Some("x")
        );
    }
}
