//! Session state machine: wires `Osc633Parser` → `Block` transitions.
//!
//! `Session` owns the block list and the current open block. It applies
//! `Osc633Event`s to blocks, assigns monotonic `BlockId`s, and emits
//! `SessionEvent`s for front-end rendering.

use tokio::sync::broadcast;

use crate::block::{Block, BlockId, BlockKind, OutputChunk, Stream};
use crate::osc633::{Osc633Event, Osc633Parser};

#[derive(Debug, Clone)]
pub enum SessionEvent {
    BlockOpened { id: BlockId },
    OutputAppended { id: BlockId, chunk: OutputChunk },
    BlockClosed { id: BlockId },
    AgentMessage { text: String },
}

pub struct Session {
    pub id: String,
    blocks: Vec<Block>,
    next_id: u64,
    open_id: Option<BlockId>,
    parser: Osc633Parser,
    tx: broadcast::Sender<SessionEvent>,
}

impl Session {
    pub fn new(id: impl Into<String>) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            id: id.into(),
            blocks: vec![],
            next_id: 1,
            open_id: None,
            parser: Osc633Parser::new(),
            tx,
        }
    }

    /// Subscribe to `SessionEvent`s (for front-ends).
    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.tx.subscribe()
    }

    /// Feed raw PTY bytes into the session. Parses OSC-633 markers and updates
    /// the block list accordingly.
    pub fn on_pty_bytes(&mut self, bytes: &[u8]) {
        let events = self.parser.feed(bytes);
        for ev in events {
            self.apply_osc(ev);
        }
    }

    /// Snapshot of all completed + in-flight blocks.
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    fn apply_osc(&mut self, ev: Osc633Event) {
        match ev {
            Osc633Event::PromptStart => {
                // Close any open block without an exit (e.g. interrupted)
                if let Some(id) = self.open_id.take() {
                    if let Some(b) = self.blocks.iter_mut().find(|b| b.id == id) {
                        b.finish(1);
                    }
                    let _ = self.tx.send(SessionEvent::BlockClosed { id });
                }
                // Start a new pending block
                let id = BlockId(self.next_id);
                self.next_id += 1;
                self.blocks.push(Block::new(id, BlockKind::Shell, ""));
                self.open_id = Some(id);
                let _ = self.tx.send(SessionEvent::BlockOpened { id });
            }
            Osc633Event::CommandLine(cmd) => {
                if let Some(id) = self.open_id
                    && let Some(b) = self.blocks.iter_mut().find(|b| b.id == id)
                {
                    b.input = cmd;
                }
            }
            Osc633Event::PreExec => {
                // Output capture begins; nothing structural to do here
            }
            Osc633Event::Output(text) => {
                if let Some(id) = self.open_id {
                    let chunk = OutputChunk::text(Stream::Stdout, &text);
                    if let Some(b) = self.blocks.iter_mut().find(|b| b.id == id) {
                        b.push(chunk.clone());
                    }
                    let _ = self.tx.send(SessionEvent::OutputAppended { id, chunk });
                }
            }
            Osc633Event::Exit(code) => {
                if let Some(id) = self.open_id.take() {
                    if let Some(b) = self.blocks.iter_mut().find(|b| b.id == id) {
                        b.finish(code);
                    }
                    let _ = self.tx.send(SessionEvent::BlockClosed { id });
                }
            }
            Osc633Event::PromptEnd => {}
        }
    }
}
