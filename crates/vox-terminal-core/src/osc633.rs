//! OSC-633 block parser — Rust port of the semantics from `osc633.ts`.
//!
//! Unlike the TS version (which receives pre-decoded marker calls from xterm.js),
//! this version does **both** layers: byte-level scan of ESC ] 633;... BEL/ST
//! sequences **and** output capture between markers. The GUI's xterm.js handles
//! this for the browser; here we own the full pipeline.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Osc633Event {
    PromptStart,
    PromptEnd,
    CommandLine(String),
    PreExec,
    Exit(i32),
    Output(String),
}

/// General `\xNN` hex decode — mirrors `decodeCommand` in `osc633.ts` exactly.
/// That function uses `/\\x([0-9a-fA-F]{2})/g` — not a fixed table.
pub fn decode_command(enc: &str) -> String {
    let bytes = enc.as_bytes();
    let mut out = String::with_capacity(enc.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && bytes.get(i + 1) == Some(&b'x') && i + 3 < bytes.len()
            && let Ok(n) = u8::from_str_radix(&enc[i + 2..i + 4], 16) {
                out.push(n as char);
                i += 4;
                continue;
            }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Byte-fed OSC-633 scanner.
///
/// Call [`feed`] with raw PTY bytes. Returns any events emitted in this batch.
/// Partial escape sequences are buffered and completed on the next call.
#[derive(Default)]
pub struct Osc633Parser {
    /// Internal ring buffer — holds partial data across calls.
    buf: Vec<u8>,
    /// True when we are currently inside a `C` (PreExec) marker — capturing output.
    capturing: bool,
    /// Pending output bytes accumulated between a `C` and `D` marker.
    output_buf: Vec<u8>,
}

impl Osc633Parser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed raw PTY bytes, return any events parsed from this batch.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Osc633Event> {
        self.buf.extend_from_slice(bytes);
        self.parse()
    }

    fn parse(&mut self) -> Vec<Osc633Event> {
        let mut events = Vec::new();
        let mut pos = 0;

        while pos < self.buf.len() {
            // Is there an OSC 633 sequence starting at or after `pos`?
            if let Some(osc_start) = find_osc_start(&self.buf[pos..]) {
                let abs_start = pos + osc_start;

                // Emit any output bytes before this OSC marker
                if abs_start > pos {
                    let raw = &self.buf[pos..abs_start];
                    if self.capturing {
                        self.output_buf.extend_from_slice(raw);
                    } else {
                        // Plain passthrough output (outside command execution)
                        let s = String::from_utf8_lossy(raw);
                        if !s.is_empty() {
                            events.push(Osc633Event::Output(s.into_owned()));
                        }
                    }
                    pos = abs_start;
                }

                // Try to extract the complete OSC sequence
                match extract_osc_payload(&self.buf[abs_start..]) {
                    Some((payload, seq_len)) => {
                        if let Some(ev) = self.process_marker(&payload, &mut events) {
                            events.push(ev);
                        }
                        pos = abs_start + seq_len;
                    }
                    None => {
                        // Incomplete sequence — keep from abs_start in the buffer
                        let remaining = self.buf[abs_start..].to_vec();
                        self.buf = remaining;
                        return events;
                    }
                }
            } else {
                // No OSC marker in the rest of the buffer
                let tail = &self.buf[pos..];
                if self.capturing {
                    self.output_buf.extend_from_slice(tail);
                } else if !tail.is_empty() {
                    let s = String::from_utf8_lossy(tail);
                    events.push(Osc633Event::Output(s.into_owned()));
                }
                self.buf.clear();
                return events;
            }
        }

        self.buf.clear();
        events
    }

    fn process_marker(
        &mut self,
        payload: &str,
        events: &mut Vec<Osc633Event>,
    ) -> Option<Osc633Event> {
        // payload is everything after "633;"
        if let Some(rest) = payload.strip_prefix("633;") {
            match rest {
                "A" => return Some(Osc633Event::PromptStart),
                "B" => return None, // no-op per osc633.ts
                "C" => {
                    self.capturing = true;
                    self.output_buf.clear();
                    return Some(Osc633Event::PreExec);
                }
                _ if rest.starts_with("E;") => {
                    let cmd = decode_command(&rest[2..]);
                    return Some(Osc633Event::CommandLine(cmd));
                }
                _ if rest.starts_with("D;") || rest == "D" => {
                    if self.capturing {
                        // Flush captured output
                        let s = String::from_utf8_lossy(&self.output_buf);
                        if !s.is_empty() {
                            events.push(Osc633Event::Output(s.into_owned()));
                        }
                        self.output_buf.clear();
                        self.capturing = false;
                    }
                    let exit_str = rest.strip_prefix("D;").unwrap_or("0");
                    let code = exit_str.parse::<i32>().unwrap_or(0);
                    return Some(Osc633Event::Exit(code));
                }
                _ => {}
            }
        }
        None
    }
}

/// Find the start of `ESC ] 633 ;` in `buf`, returning the offset.
fn find_osc_start(buf: &[u8]) -> Option<usize> {
    for i in 0..buf.len().saturating_sub(1) {
        if buf[i] == b'\x1b' && buf.get(i + 1) == Some(&b']') {
            // Peek ahead to confirm "633;" follows
            let peek = &buf[i + 2..];
            if peek.starts_with(b"633;") || b"633;".starts_with(peek) {
                return Some(i);
            }
        }
    }
    // Also check for lone ESC at end (partial sequence)
    if buf.last() == Some(&b'\x1b') {
        return Some(buf.len() - 1);
    }
    None
}

/// Try to extract the OSC payload from `buf` starting at `ESC ]`.
/// Returns `(payload_str, total_bytes_consumed)` or `None` if incomplete.
fn extract_osc_payload(buf: &[u8]) -> Option<(String, usize)> {
    // Must start with ESC ]
    if buf.len() < 2 || buf[0] != b'\x1b' || buf[1] != b']' {
        return None;
    }
    // Find BEL (0x07) or ST (ESC \) terminator
    let mut i = 2;
    while i < buf.len() {
        if buf[i] == b'\x07' {
            let payload = std::str::from_utf8(&buf[2..i]).ok()?.to_string();
            return Some((payload, i + 1));
        }
        if buf[i] == b'\x1b' && buf.get(i + 1) == Some(&b'\\') {
            let payload = std::str::from_utf8(&buf[2..i]).ok()?.to_string();
            return Some((payload, i + 2));
        }
        i += 1;
    }
    None // incomplete
}
