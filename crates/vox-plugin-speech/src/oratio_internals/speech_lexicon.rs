//! Optional project lexicon: spoken aliases → canonical tokens (identifiers, symbols).
//!
//! Aliases are matched per **word token** (alphanumeric + `_`). Multi-word phrases are not
//! expanded yet; use single-token aliases or run a separate phrase normalizer first.

use std::collections::{HashMap, HashSet};

use serde::Deserialize;

/// One lexicon entry: canonical text plus optional spoken aliases.
#[derive(Debug, Clone, Deserialize)]
pub struct LexiconEntry {
    /// Target spelling (e.g. identifier or symbol).
    pub canonical: String,
    /// Phrases that should map to `canonical` (lowercase match).
    #[serde(default)]
    pub aliases: Vec<String>,
}

/// Loaded lexicon for transcript normalization.
#[derive(Debug, Clone, Default)]
pub struct SpeechLexicon {
    /// Lowercased alias → canonical replacement.
    map: HashMap<String, String>,
}

impl SpeechLexicon {
    /// Parse lexicon from JSON (see `contracts/speech-to-code/lexicon.schema.json`).
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        #[derive(Deserialize)]
        struct Root {
            #[allow(dead_code)]
            schema_version: Option<String>,
            entries: Vec<LexiconEntry>,
        }
        let root: Root = serde_json::from_slice(bytes)?;
        let mut map = HashMap::new();
        for e in root.entries {
            let c = e.canonical.trim().to_string();
            if c.is_empty() {
                continue;
            }
            for a in e.aliases {
                let k = a.trim().to_ascii_lowercase();
                if !k.is_empty() {
                    map.insert(k, c.clone());
                }
            }
        }
        Ok(Self { map })
    }

    /// Merge `other` into `self`; existing keys in `self` win (deterministic precedence for stacked lexicons).
    pub fn merge_from(&mut self, other: Self) {
        for (k, v) in other.map {
            self.map.entry(k).or_insert(v);
        }
    }

    /// True when no entries were loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Unique aliases and canonicals for contextual biasing / reranking (longest strings are most discriminative).
    #[must_use]
    pub fn bias_phrases_sorted(&self, max_phrases: usize) -> Vec<String> {
        let mut seen = HashSet::<String>::new();
        let mut out: Vec<String> = Vec::new();
        for (alias, canon) in &self.map {
            if !alias.is_empty() && seen.insert(alias.clone()) {
                out.push(alias.clone());
            }
            if !canon.is_empty() && seen.insert(canon.clone()) {
                out.push(canon.clone());
            }
        }
        out.sort_by_key(|s| std::cmp::Reverse(s.len()));
        out.truncate(max_phrases.max(1));
        out
    }
}
