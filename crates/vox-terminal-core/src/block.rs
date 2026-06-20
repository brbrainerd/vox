use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockKind {
    VoxNative,
    Shell,
    AgentTurn,
    SlashCommand,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockStatus {
    Running,
    Ok,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stream {
    Stdout,
    Stderr,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputChunk {
    pub stream: Stream,
    pub text: String,
}

impl OutputChunk {
    pub fn text(stream: Stream, s: impl Into<String>) -> Self {
        Self {
            stream,
            text: s.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub id: BlockId,
    pub kind: BlockKind,
    pub input: String,
    pub output: Vec<OutputChunk>,
    pub status: BlockStatus,
    pub exit_code: Option<i32>,
}

impl Block {
    pub fn new(id: BlockId, kind: BlockKind, input: impl Into<String>) -> Self {
        Self {
            id,
            kind,
            input: input.into(),
            output: vec![],
            status: BlockStatus::Running,
            exit_code: None,
        }
    }

    pub fn push(&mut self, c: OutputChunk) {
        self.output.push(c);
    }

    pub fn finish(&mut self, exit: i32) {
        self.exit_code = Some(exit);
        self.status = if exit == 0 {
            BlockStatus::Ok
        } else {
            BlockStatus::Failed
        };
    }

    /// Text projection for agent context + transcript: output with ANSI/VT
    /// escapes stripped. Visual rendering is the front-end's responsibility.
    pub fn plain_output(&self) -> String {
        let raw: String = self.output.iter().map(|c| c.text.as_str()).collect();
        strip_ansi(&raw)
    }
}

/// Minimal CSI/OSC escape stripper for plain-text export.
/// Full VT grid rendering lives in the front-end (alacritty_terminal for TUI,
/// xterm.js for GUI).
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.peek() {
                Some('[') => {
                    chars.next(); // consume '['
                    // CSI: consume until a byte in 0x40–0x7E
                    for ch in chars.by_ref() {
                        if ('\x40'..='\x7e').contains(&ch) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next(); // consume ']'
                    // OSC: consume until BEL (0x07) or ST (ESC \)
                    for ch in chars.by_ref() {
                        if ch == '\x07' {
                            break;
                        }
                        if ch == '\x1b' {
                            // Only consume the next char if it is actually '\'
                            if matches!(chars.peek(), Some('\\')) {
                                chars.next();
                            }
                            break;
                        }
                    }
                }
                _ => {} // lone ESC — skip it
            }
        } else {
            out.push(c);
        }
    }
    out
}
