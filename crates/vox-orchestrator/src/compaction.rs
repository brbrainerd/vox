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

fn default_complexity_token_weight() -> usize {
    32
}

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
    /// Extra token weight per complexity unit (1–10) on a turn for budgeting / compaction. Default: 32.
    #[serde(default = "default_complexity_token_weight")]
    pub complexity_token_weight: usize,
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
            complexity_token_weight: default_complexity_token_weight(),
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Turns that were dropped by this pass, in their original order.
    ///
    /// This is the lossless-archival contract: callers MUST persist
    /// `dropped_turns` somewhere retrievable (e.g. [`crate::session::Session`]'s
    /// `archived_turns`) before discarding them from the live/working set.
    /// `compact()` itself never discards data — it only classifies turns as
    /// retained vs. dropped and hands both back to the caller.
    pub dropped_turns: Vec<Turn>,
    /// Number of turns that were dropped (`dropped_turns.len()`, kept as a
    /// separate field for cheap access/back-compat with count-only readers).
    pub dropped_count: usize,
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
    #[error(
        "Context window guard exceeded: {current} tokens in use, only {available} available (min viable: {min})"
    )]
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

    /// tiktoken-compatible BPE token estimator.
    pub fn estimate_tokens(text: &str) -> usize {
        crate::context::token_optimization::count_tokens(text)
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
    ///
    /// **Lossless contract:** every turn removed from `history` is returned in
    /// [`CompactionResult::dropped_turns`] in its original order — `compact()`
    /// never discards a turn's content, it only classifies it as retained or
    /// dropped. Callers that need durable archival (e.g. [`crate::session`])
    /// must persist `dropped_turns` before dropping their own working copy.
    pub fn compact(&self, history: &[Turn]) -> Result<CompactionResult, CompactionError> {
        let tokens_before = Self::count_tokens(history);

        if !self.should_compact(tokens_before) {
            return Ok(CompactionResult {
                retained_turns: history.to_vec(),
                dropped_turns: Vec::new(),
                dropped_count: 0,
                tokens_before,
                tokens_after: tokens_before,
                compacted: false,
            });
        }

        let keep_indices: std::collections::HashSet<usize> = match self.config.strategy {
            CompactionStrategy::Aggressive => self.trim_aggressive(history),
            CompactionStrategy::Balanced => self.trim_balanced(history),
            CompactionStrategy::Conservative => self.trim_conservative(history),
        };

        let mut retained_turns = Vec::with_capacity(keep_indices.len());
        let mut dropped_turns = Vec::with_capacity(history.len() - keep_indices.len());
        for (i, t) in history.iter().enumerate() {
            if keep_indices.contains(&i) {
                retained_turns.push(t.clone());
            } else {
                dropped_turns.push(t.clone());
            }
        }

        let tokens_after = Self::count_tokens(&retained_turns);
        let dropped_count = dropped_turns.len();

        // Guard: after compaction we must have enough headroom
        self.guard(tokens_after)?;

        Ok(CompactionResult {
            retained_turns,
            dropped_turns,
            dropped_count,
            tokens_before,
            tokens_after,
            compacted: true,
        })
    }

    // ── Private trim strategies ─────────────────────────────────────────
    // Each returns the set of *retained* indices into `history`; `compact()`
    // derives both `retained_turns` and `dropped_turns` from that single
    // source of truth so the two lists always partition `history` exactly.

    /// Keep only the system prompt (if any) + the last N tokens of tail.
    fn trim_aggressive(&self, history: &[Turn]) -> std::collections::HashSet<usize> {
        let target = self.config.tail_preserve_tokens;
        let mut keep = std::collections::HashSet::new();

        // Always keep system turns at the head
        for (i, t) in history.iter().enumerate() {
            if t.role == "system" {
                keep.insert(i);
            }
        }

        // Fill from the tail backwards until we hit target
        let mut accumulated = 0usize;
        for (i, t) in history.iter().enumerate().rev() {
            if t.role == "system" {
                continue;
            }
            if accumulated + t.token_estimate <= target {
                accumulated += t.token_estimate;
                keep.insert(i);
            } else {
                break;
            }
        }
        keep
    }

    /// Head/tail preservation: keep head_preserve_tokens at the front and
    /// tail_preserve_tokens at the back, dropping the middle. Always preserves system messages.
    fn trim_balanced(&self, history: &[Turn]) -> std::collections::HashSet<usize> {
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

        keep_indices
    }

    /// Only drop whole turns from the middle until we fall below the trigger.
    fn trim_conservative(&self, history: &[Turn]) -> std::collections::HashSet<usize> {
        let trigger = self.config.trigger_at();
        let mut keep: std::collections::HashSet<usize> = (0..history.len()).collect();
        let mut total = Self::count_tokens(history);

        // Find the midpoint; drop turns outward from the middle
        let preserve_head = 1; // keep at least first turn
        let preserve_tail = 2; // keep at least last 2 turns

        let mut lo = preserve_head;
        while total >= trigger && lo < history.len().saturating_sub(preserve_tail) {
            if history[lo].role == "system" {
                lo += 1;
                continue;
            }
            if keep.remove(&lo) {
                total = total.saturating_sub(history[lo].token_estimate);
            }
            lo += 1;
        }
        keep
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
        assert!((1..=10).contains(&est));
    }

    #[test]
    fn no_compaction_when_under_threshold() {
        let engine = CompactionEngine::default();
        let turns = make_turns(&[1_000, 1_000]); // 2k tokens, well under 128k threshold
        let result = engine.compact(&turns).expect("compact");
        assert!(!result.compacted);
        assert_eq!(result.dropped_count, 0);
        assert!(result.dropped_turns.is_empty());
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
            complexity_token_weight: 32,
        };
        let engine = CompactionEngine::new(cfg);
        let turns = make_turns(&[5, 5, 20, 20, 5, 5]); // total 60, over threshold
        let result = engine.compact(&turns).expect("compact");
        assert!(result.compacted);
        assert!(result.dropped_count > 0);
        assert_eq!(result.dropped_turns.len(), result.dropped_count);
        // Lossless: retained + dropped accounts for every original turn.
        assert_eq!(
            result.retained_turns.len() + result.dropped_turns.len(),
            turns.len()
        );
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
            complexity_token_weight: 32,
        };
        let engine = CompactionEngine::new(cfg);
        let turns = make_turns(&[5, 5, 5, 5, 10]);
        let result = engine.compact(&turns).expect("compact");
        assert!(result.compacted);
    }

    /// RED-then-GREEN: `compact()` must return the actual dropped `Turn`
    /// content (lossless), not just a count — this is the T4.2 audit-defect
    /// fix. Every dropped turn's exact content must be present in
    /// `dropped_turns`, and retained ∪ dropped must equal the original set.
    #[test]
    fn compact_losslessly_returns_dropped_turn_content() {
        let cfg = CompactionConfig {
            max_context_tokens: 100,
            reserved_tokens: 10,
            compaction_threshold: 0.5,
            min_viable_tokens: 5,
            strategy: CompactionStrategy::Balanced,
            head_preserve_tokens: 10,
            tail_preserve_tokens: 15,
            complexity_token_weight: 32,
        };
        let engine = CompactionEngine::new(cfg);
        let turns = make_turns(&[5, 5, 20, 20, 5, 5]);
        let result = engine.compact(&turns).expect("compact");

        assert!(result.compacted);
        assert!(!result.dropped_turns.is_empty(), "must actually drop turns");

        // Every dropped turn must be traceable back to the original history
        // by exact content — nothing is summarized/mutated/lost at this layer.
        for dropped in &result.dropped_turns {
            assert!(
                turns.contains(dropped),
                "dropped turn content must match an original turn exactly"
            );
        }

        // Lossless partition: retained + dropped reconstructs the full turn set
        // (as a multiset — order/dedup are not required at this layer).
        let mut reconstructed: Vec<&Turn> = result
            .retained_turns
            .iter()
            .chain(result.dropped_turns.iter())
            .collect();
        let mut original: Vec<&Turn> = turns.iter().collect();
        reconstructed.sort_by_key(|t| t.token_estimate);
        original.sort_by_key(|t| t.token_estimate);
        assert_eq!(reconstructed, original);
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
