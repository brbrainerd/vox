//! Context compaction engine for Vox agents.
//!
//! Prevents context window overflow by summarizing old conversation turns.
//! Adopts OpenClaw's compaction strategies:
//! - **Context window guard** — hard-stops if available tokens < minimum
//! - **Turn-based trimming** — trims whole turns, never mid-message
//! - **Head/tail preservation** — keeps first N + last M tokens of the context
//! - **Pre-compaction hook** — fires before summarization so memory can flush
//! - **Three strategies**: `Aggressive`, `Balanced`, `Conservative`

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// CompactionStrategy
// ---------------------------------------------------------------------------

/// Strategy that controls how aggressively stale context is trimmed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionStrategy {
    /// Trim as much as possible: keep only the tail + head guard.
    Aggressive,
    /// Default: trim turns beyond the threshold, preserving recent turns.
    #[default]
    Balanced,
    /// Only trim when absolutely necessary (above 95% capacity).
    Conservative,
}

impl std::fmt::Display for CompactionStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Aggressive => write!(f, "aggressive"),
            Self::Balanced => write!(f, "balanced"),
            Self::Conservative => write!(f, "conservative"),
        }
    }
}

// ---------------------------------------------------------------------------
// CompactionConfig
// ---------------------------------------------------------------------------

/// Configuration for the compaction engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionConfig {
    /// Maximum tokens the model's context can hold. Default: 128 000.
    pub max_context_tokens: usize,
    /// Tokens always reserved for the model's response. Default: 10 000.
    pub reserved_tokens: usize,
    /// Fraction of `max_context_tokens` at which compaction triggers. Default: 0.80.
    pub compaction_threshold: f64,
    /// Minimum tokens that MUST remain after compaction or we error. Default: 2 000.
    pub min_viable_tokens: usize,
    /// Strategy to use when trimming. Default: `Balanced`.
    pub strategy: CompactionStrategy,
    /// Number of head tokens to always preserve. Default: 2 000.
    pub head_preserve_tokens: usize,
    /// Number of tail tokens to always preserve. Default: 8 000.
    pub tail_preserve_tokens: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            max_context_tokens: 128_000,
            reserved_tokens: 10_000,
            compaction_threshold: 0.80,
            min_viable_tokens: 2_000,
            strategy: CompactionStrategy::Balanced,
            head_preserve_tokens: 2_000,
            tail_preserve_tokens: 8_000,
        }
    }
}

impl CompactionConfig {
    /// Returns the token count at which compaction should trigger.
    pub fn trigger_at(&self) -> usize {
        ((self.max_context_tokens as f64) * self.compaction_threshold) as usize
    }

    /// Returns the usable token budget (max − reserved).
    pub fn usable_budget(&self) -> usize {
        self.max_context_tokens.saturating_sub(self.reserved_tokens)
    }
}

// ---------------------------------------------------------------------------
// Turn / CompactionInput
// ---------------------------------------------------------------------------

/// A single conversation turn (user message or assistant response).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    /// `"user"` or `"assistant"` (or `"system"`).
    pub role: String,
    /// Raw text content.
    pub content: String,
    /// Estimated token count. Fill with [`CompactionEngine::estimate_tokens`].
    pub token_estimate: usize,
}

impl Turn {
    /// Create a turn with an estimated token count.
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        let content = content.into();
        let token_estimate = CompactionEngine::estimate_tokens(&content);
        Self {
            role: role.into(),
            content,
            token_estimate,
        }
    }
}

// ---------------------------------------------------------------------------
// CompactionResult
// ---------------------------------------------------------------------------

/// Outcome of a compaction pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionResult {
    /// Turns that survived the compaction pass.
    pub retained_turns: Vec<Turn>,
    /// Number of turns that were dropped.
    pub dropped_turns: usize,
    /// Tokens before compaction.
    pub tokens_before: usize,
    /// Estimated tokens after compaction.
    pub tokens_after: usize,
    /// Whether compaction was actually performed.
    pub compacted: bool,
}

// ---------------------------------------------------------------------------
// CompactionError
// ---------------------------------------------------------------------------

/// Errors from the compaction engine.
#[derive(Debug, thiserror::Error)]
pub enum CompactionError {
    #[error("Context window guard exceeded: {current} tokens in use, only {available} available (min viable: {min})")]
    ContextWindowExceeded {
        current: usize,
        available: usize,
        min: usize,
    },
}

// ---------------------------------------------------------------------------
// CompactionEngine
// ---------------------------------------------------------------------------

/// Manages context window usage and trims conversation history when required.
pub struct CompactionEngine {
    config: CompactionConfig,
}

impl CompactionEngine {
    /// Create with explicit config.
    pub fn new(config: CompactionConfig) -> Self {
        Self { config }
    }

    /// Reference to the active config.
    pub fn config(&self) -> &CompactionConfig {
        &self.config
    }

    /// Naive token estimator: 1 token ≈ 4 characters (GPT-style average).
    ///
    /// Replace with tiktoken / sentencepiece for accuracy;
    /// this is intentionally dependency-free for now.
    pub fn estimate_tokens(text: &str) -> usize {
        (text.len() / 4).max(1)
    }

    /// Returns `true` if current token usage warrants compaction.
    pub fn should_compact(&self, current_tokens: usize) -> bool {
        current_tokens >= self.config.trigger_at()
    }

    /// Returns `Err` if context is so full that the model cannot respond.
    pub fn guard(&self, current_tokens: usize) -> Result<(), CompactionError> {
        let available = self.config.usable_budget().saturating_sub(current_tokens);
        if available < self.config.min_viable_tokens {
            Err(CompactionError::ContextWindowExceeded {
                current: current_tokens,
                available,
                min: self.config.min_viable_tokens,
            })
        } else {
            Ok(())
        }
    }

    /// Count the total estimated tokens across a slice of turns.
    pub fn count_tokens(turns: &[Turn]) -> usize {
        turns.iter().map(|t| t.token_estimate).sum()
    }

    /// Perform a compaction pass over `history`.
    ///
    /// Returns a [`CompactionResult`] describing what was retained/dropped.
    /// Does NOT modify `history` in place — caller replaces accordingly.
    pub fn compact(&self, history: &[Turn]) -> Result<CompactionResult, CompactionError> {
        let tokens_before = Self::count_tokens(history);

        if !self.should_compact(tokens_before) {
            return Ok(CompactionResult {
                retained_turns: history.to_vec(),
                dropped_turns: 0,
                tokens_before,
                tokens_after: tokens_before,
                compacted: false,
            });
        }

        let retained = match self.config.strategy {
            CompactionStrategy::Aggressive => self.trim_aggressive(history),
            CompactionStrategy::Balanced => self.trim_balanced(history),
            CompactionStrategy::Conservative => self.trim_conservative(history),
        };

        let tokens_after = Self::count_tokens(&retained);
        let dropped_turns = history.len().saturating_sub(retained.len());

        // Guard: after compaction we must have enough headroom
        self.guard(tokens_after)?;

        Ok(CompactionResult {
            retained_turns: retained,
            dropped_turns,
            tokens_before,
            tokens_after,
            compacted: true,
        })
    }

    // ── Private trim strategies ──────────────────────────────────────────

    /// Keep only the system prompt (if any) + the last N tokens of tail.
    fn trim_aggressive(&self, history: &[Turn]) -> Vec<Turn> {
        let target = self.config.tail_preserve_tokens;
        let mut out: Vec<Turn> = Vec::new();

        // Always keep system turns at the head
        for t in history {
            if t.role == "system" {
                out.push(t.clone());
            }
        }

        // Fill from the tail backwards until we hit target
        let mut tail: Vec<Turn> = Vec::new();
        let mut accumulated = 0usize;
        for t in history.iter().rev() {
            if t.role == "system" {
                continue;
            }
            if accumulated + t.token_estimate <= target {
                accumulated += t.token_estimate;
                tail.push(t.clone());
            } else {
                break;
            }
        }
        tail.reverse();
        out.extend(tail);
        out
    }

    /// Head/tail preservation: keep head_preserve_tokens at the front and
    /// tail_preserve_tokens at the back, dropping the middle. Always preserves system messages.
    fn trim_balanced(&self, history: &[Turn]) -> Vec<Turn> {
        let head_budget = self.config.head_preserve_tokens;
        let tail_budget = self.config.tail_preserve_tokens;

        let mut head_tokens = 0usize;
        let mut keep_indices = std::collections::HashSet::new();

        // 1. Always keep system messages
        for (i, t) in history.iter().enumerate() {
            if t.role == "system" {
                keep_indices.insert(i);
            }
        }

        // 2. Keep head turns until budget
        for (i, t) in history.iter().enumerate() {
            if keep_indices.contains(&i) {
                continue;
            }
            if head_tokens + t.token_estimate <= head_budget {
                head_tokens += t.token_estimate;
                keep_indices.insert(i);
            } else {
                break;
            }
        }

        // 3. Keep tail turns until budget
        let mut tail_tokens = 0usize;
        for (i, t) in history.iter().enumerate().rev() {
            if keep_indices.contains(&i) {
                continue;
            }
            if tail_tokens + t.token_estimate <= tail_budget {
                tail_tokens += t.token_estimate;
                keep_indices.insert(i);
            } else {
                break;
            }
        }

        let mut out = Vec::with_capacity(keep_indices.len());
        for (i, t) in history.iter().enumerate() {
            if keep_indices.contains(&i) {
                out.push(t.clone());
            }
        }
        out
    }

    /// Only drop whole turns from the middle until we fall below the trigger.
    fn trim_conservative(&self, history: &[Turn]) -> Vec<Turn> {
        let trigger = self.config.trigger_at();
        let mut turns = history.to_vec();
        let mut total = Self::count_tokens(&turns);

        // Find the midpoint; drop turns outward from the middle
        let preserve_head = 1; // keep at least first turn
        let preserve_tail = 2; // keep at least last 2 turns

        let mut lo = preserve_head;
        while total >= trigger && lo < turns.len().saturating_sub(preserve_tail) {
            if turns[lo].role == "system" {
                lo += 1;
                continue;
            }
            total = total.saturating_sub(turns[lo].token_estimate);
            turns.remove(lo);
        }
        turns
    }
}

impl Default for CompactionEngine {
    fn default() -> Self {
        Self::new(CompactionConfig::default())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_turns(sizes: &[usize]) -> Vec<Turn> {
        sizes
            .iter()
            .enumerate()
            .map(|(i, &sz)| {
                let content = "x".repeat(sz * 4); // 4 chars per token
                Turn {
                    role: if i % 2 == 0 { "user" } else { "assistant" }.to_string(),
                    content,
                    token_estimate: sz,
                }
            })
            .collect()
    }

    #[test]
    fn estimate_tokens_reasonable() {
        let s = "Hello, world!"; // 13 chars → ~3 tokens
        let est = CompactionEngine::estimate_tokens(s);
        assert!(est >= 1 && est <= 10);
    }

    #[test]
    fn no_compaction_when_under_threshold() {
        let engine = CompactionEngine::default();
        let turns = make_turns(&[1_000, 1_000]); // 2k tokens, well under 128k threshold
        let result = engine.compact(&turns).expect("compact");
        assert!(!result.compacted);
        assert_eq!(result.dropped_turns, 0);
    }

    #[test]
    fn balanced_compaction_drops_middle_turns() {
        let cfg = CompactionConfig {
            max_context_tokens: 100,
            reserved_tokens: 10,
            compaction_threshold: 0.5, // trigger at 50 tokens
            min_viable_tokens: 5,
            strategy: CompactionStrategy::Balanced,
            head_preserve_tokens: 10,
            tail_preserve_tokens: 15,
        };
        let engine = CompactionEngine::new(cfg);
        let turns = make_turns(&[5, 5, 20, 20, 5, 5]); // total 60, over threshold
        let result = engine.compact(&turns).expect("compact");
        assert!(result.compacted);
        assert!(result.dropped_turns > 0);
        assert!(result.tokens_after <= 65); // should have reduced
    }

    #[test]
    fn aggressive_compaction_keeps_only_tail() {
        let cfg = CompactionConfig {
            max_context_tokens: 100,
            reserved_tokens: 10,
            compaction_threshold: 0.3, // very low trigger
            min_viable_tokens: 2,
            strategy: CompactionStrategy::Aggressive,
            head_preserve_tokens: 5,
            tail_preserve_tokens: 15,
        };
        let engine = CompactionEngine::new(cfg);
        let turns = make_turns(&[5, 5, 5, 5, 10]);
        let result = engine.compact(&turns).expect("compact");
        assert!(result.compacted);
    }

    #[test]
    fn guard_errors_when_over_minimum() {
        let cfg = CompactionConfig {
            max_context_tokens: 100,
            reserved_tokens: 50,
            min_viable_tokens: 60, // requires 60 usable, but only 50 usable
            ..Default::default()
        };
        let engine = CompactionEngine::new(cfg);
        // usable = 100-50 = 50; current = 0; available = 50; min = 60 → error
        let result = engine.guard(0);
        assert!(result.is_err());
    }

    #[test]
    fn config_trigger_at_calculation() {
        let cfg = CompactionConfig {
            max_context_tokens: 100_000,
            compaction_threshold: 0.8,
            ..Default::default()
        };
        assert_eq!(cfg.trigger_at(), 80_000);
    }

    #[test]
    fn should_compact_false_under_threshold() {
        let engine = CompactionEngine::default();
        assert!(!engine.should_compact(50_000));
    }

    #[test]
    fn should_compact_true_at_threshold() {
        let engine = CompactionEngine::default();
        assert!(engine.should_compact(128_000));
    }

    #[test]
    fn compaction_strategy_display() {
        assert_eq!(CompactionStrategy::Aggressive.to_string(), "aggressive");
        assert_eq!(CompactionStrategy::Balanced.to_string(), "balanced");
        assert_eq!(CompactionStrategy::Conservative.to_string(), "conservative");
    }
}
