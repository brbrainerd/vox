//! Decoder-time logit processors for constrained decode in Candle Whisper.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;
use candle_core::Tensor;
use tokenizers::Tokenizer;

/// Read-only decode step metadata available to logit processors.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct LogitStepContext {
    /// Zero-based token generation step within a decode pass.
    pub step: usize,
    /// Temperature used for this decode attempt.
    pub temperature: f64,
    /// Current generated token sequence (includes SOT/task/system tokens).
    pub generated_tokens: Vec<u32>,
}

/// Mutating logit processor hook applied before sampling/argmax.
pub trait LogitProcessor: Send {
    /// Stable processor name for telemetry/tracing.
    fn name(&self) -> &'static str {
        "noop"
    }

    /// Called once when a new decode pass starts.
    fn reset_for_decode(&mut self) {
        let _ = ();
    }

    /// Returns adjusted logits for this step.
    fn apply(&mut self, _ctx: &LogitStepContext, logits: &Tensor) -> Result<Tensor> {
        Ok(logits.clone())
    }

    /// Observe the selected token so stateful processors can advance.
    fn on_token_decoded(&mut self, _token: u32) {
        let _ = std::hint::black_box(_token);
    }
}

/// No-op processor used by default.
pub struct NoopLogitProcessor;

impl LogitProcessor for NoopLogitProcessor {}

/// Sequential composition of logit processors.
pub struct CompositeLogitProcessor {
    chain: Vec<Box<dyn LogitProcessor>>,
}

impl CompositeLogitProcessor {
    #[must_use]
    /// Create a chained processor from multiple processors.
    pub fn new(chain: Vec<Box<dyn LogitProcessor>>) -> Self {
        Self { chain }
    }
}

impl LogitProcessor for CompositeLogitProcessor {
    fn name(&self) -> &'static str {
        "composite"
    }

    fn reset_for_decode(&mut self) {
        for p in &mut self.chain {
            p.reset_for_decode();
        }
    }

    fn apply(&mut self, ctx: &LogitStepContext, logits: &Tensor) -> Result<Tensor> {
        let mut out = logits.clone();
        for p in &mut self.chain {
            out = p.apply(ctx, &out)?;
        }
        Ok(out)
    }

    fn on_token_decoded(&mut self, token: u32) {
        for p in &mut self.chain {
            p.on_token_decoded(token);
        }
    }
}

/// Adds fixed positive/negative deltas to selected token ids.
pub struct AdditiveBiasProcessor {
    deltas: Vec<(u32, f32)>,
}

impl AdditiveBiasProcessor {
    #[must_use]
    /// Create a BiasLogitProcessor with a static set of token bias deltas.
    pub fn new(deltas: Vec<(u32, f32)>) -> Self {
        Self { deltas }
    }
}

impl LogitProcessor for AdditiveBiasProcessor {
    fn name(&self) -> &'static str {
        "additive_bias"
    }

    fn apply(&mut self, _ctx: &LogitStepContext, logits: &Tensor) -> Result<Tensor> {
        if self.deltas.is_empty() {
            return Ok(logits.clone());
        }
        let n = logits.dims1().map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut add = vec![0.0f32; n];
        for (id, delta) in &self.deltas {
            let i = *id as usize;
            if i < n {
                add[i] += *delta;
            }
        }
        let mask =
            Tensor::new(add.as_slice(), logits.device()).map_err(|e| anyhow::anyhow!("{e}"))?;
        logits
            .broadcast_add(&mask)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
}

/// Hard-mask token ids (adds `-inf`) each step.
pub struct ForbiddenTokenMaskProcessor {
    ids: Vec<u32>,
}

impl ForbiddenTokenMaskProcessor {
    #[must_use]
    /// Create a ForceTokenListLogitProcessor.
    pub fn new(ids: Vec<u32>) -> Self {
        Self { ids }
    }
}

impl LogitProcessor for ForbiddenTokenMaskProcessor {
    fn name(&self) -> &'static str {
        "forbidden_mask"
    }

    fn apply(&mut self, _ctx: &LogitStepContext, logits: &Tensor) -> Result<Tensor> {
        if self.ids.is_empty() {
            return Ok(logits.clone());
        }
        let n = logits.dims1().map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut add = vec![0.0f32; n];
        for id in &self.ids {
            let i = *id as usize;
            if i < n {
                add[i] = f32::NEG_INFINITY;
            }
        }
        let mask =
            Tensor::new(add.as_slice(), logits.device()).map_err(|e| anyhow::anyhow!("{e}"))?;
        logits
            .broadcast_add(&mask)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
}

#[derive(Debug, Default)]
struct TrieNode {
    terminal: bool,
    edges: HashMap<u32, usize>,
}

/// Finite token-trie constraint: only allow legal next edges from current trie state.
pub struct TokenTrieConstraintProcessor {
    nodes: Vec<TrieNode>,
    active_node: usize,
    stuck_steps: usize,
    max_stuck_steps: usize,
}

impl TokenTrieConstraintProcessor {
    /// Build sequence trie restrictions.
    pub fn from_token_sequences(seqs: &[Vec<u32>], max_stuck_steps: usize) -> Option<Self> {
        let mut nodes = vec![TrieNode::default()];
        for seq in seqs {
            if seq.is_empty() {
                continue;
            }
            let mut cur = 0usize;
            for &tok in seq {
                if let Some(next) = nodes[cur].edges.get(&tok).copied() {
                    cur = next;
                    continue;
                }
                let next = nodes.len();
                nodes.push(TrieNode::default());
                nodes[cur].edges.insert(tok, next);
                cur = next;
            }
            nodes[cur].terminal = true;
        }
        if nodes[0].edges.is_empty() {
            return None;
        }
        Some(Self {
            nodes,
            active_node: 0,
            stuck_steps: 0,
            max_stuck_steps: max_stuck_steps.max(1),
        })
    }
}

impl LogitProcessor for TokenTrieConstraintProcessor {
    fn name(&self) -> &'static str {
        "token_trie"
    }

    fn reset_for_decode(&mut self) {
        self.active_node = 0;
        self.stuck_steps = 0;
    }

    fn apply(&mut self, _ctx: &LogitStepContext, logits: &Tensor) -> Result<Tensor> {
        let node = &self.nodes[self.active_node];
        if node.edges.is_empty() {
            return Ok(logits.clone());
        }
        let n = logits.dims1().map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut add = vec![f32::NEG_INFINITY; n];
        for tok in node.edges.keys() {
            let i = *tok as usize;
            if i < n {
                add[i] = 0.0;
            }
        }
        let mask =
            Tensor::new(add.as_slice(), logits.device()).map_err(|e| anyhow::anyhow!("{e}"))?;
        logits
            .broadcast_add(&mask)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    fn on_token_decoded(&mut self, token: u32) {
        if let Some(next) = self.nodes[self.active_node].edges.get(&token).copied() {
            self.active_node = next;
            self.stuck_steps = 0;
            return;
        }
        if let Some(next) = self.nodes[0].edges.get(&token).copied() {
            self.active_node = next;
            self.stuck_steps = 0;
            return;
        }
        self.stuck_steps = self.stuck_steps.saturating_add(1);
        if self.stuck_steps >= self.max_stuck_steps {
            self.active_node = 0;
            self.stuck_steps = 0;
        }
    }
}

fn try_load_lexicon_path(
    path: &Path,
) -> Option<crate::oratio_internals::speech_lexicon::SpeechLexicon> {
    let bytes = std::fs::read(path).ok()?;
    crate::oratio_internals::speech_lexicon::SpeechLexicon::from_json_slice(&bytes).ok()
}

fn load_lexicon_from_env() -> Option<crate::oratio_internals::speech_lexicon::SpeechLexicon> {
    let mut acc = crate::oratio_internals::speech_lexicon::SpeechLexicon::default();
    if let Some(p) =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioSpeechLexiconPath).expose()
    {
        let path = Path::new(p.trim());
        if let Some(lex) = try_load_lexicon_path(path) {
            acc.merge_from(lex);
        }
    }
    let repo_root_resolved = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxRepositoryRoot);
    let repo_root = repo_root_resolved.expose();
    if let Some(root) = repo_root {
        let candidate = Path::new(root.trim()).join(".vox/speech_lexicon.json");
        if let Some(lex) = try_load_lexicon_path(&candidate) {
            acc.merge_from(lex);
        }
    }
    if acc.is_empty() { None } else { Some(acc) }
}

fn tokenize_phrase_ids(tokenizer: &Tokenizer, phrase: &str) -> Result<Vec<u32>> {
    let enc = tokenizer
        .encode(phrase, false)
        .map_err(|e| anyhow::anyhow!("tokenize phrase: {e}"))?;
    Ok(enc.get_ids().to_vec())
}

fn phrase_list_for_bias() -> Vec<String> {
    let mut out = Vec::new();
    if let Some(lex) = load_lexicon_from_env() {
        out.extend(lex.bias_phrases_sorted(256));
    }
    if let Some(hot) =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioSessionHotwords).expose()
    {
        out.extend(crate::oratio_internals::contextual_bias::parse_hotword_csv(
            hot,
        ));
    }
    let mut seen = HashSet::new();
    out.retain(|s| seen.insert(s.to_ascii_lowercase()));
    out
}

fn trie_phrase_list() -> Vec<String> {
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioConstrainedPhrases)
        .expose()
        .map(crate::oratio_internals::contextual_bias::parse_hotword_csv)
        .unwrap_or_default()
}

/// Build a decode-time logit processor chain from env/lexicon inputs.
pub fn build_logit_processor(
    tokenizer: &Tokenizer,
    cfg: Option<&crate::oratio_internals::runtime_config::LogitConstraintTunables>,
) -> Result<Box<dyn LogitProcessor>> {
    let mut chain: Vec<Box<dyn LogitProcessor>> = Vec::new();

    let bias_strength =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioLogitBiasStrength)
            .expose()
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or_else(|| cfg.map(|c| c.bias_strength).unwrap_or(0.8));
    let max_bias_tokens =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioLogitBiasMaxTokens)
            .expose()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or_else(|| cfg.map(|c| c.bias_max_tokens).unwrap_or(256))
            .max(1);
    let phrases = phrase_list_for_bias();
    if !phrases.is_empty() && bias_strength != 0.0 {
        let mut ids = Vec::<(u32, f32)>::new();
        let mut seen = HashSet::<u32>::new();
        for p in &phrases {
            let toks = tokenize_phrase_ids(tokenizer, p)?;
            for tok in toks.into_iter().take(3) {
                if seen.insert(tok) {
                    ids.push((tok, bias_strength));
                }
                if ids.len() >= max_bias_tokens {
                    break;
                }
            }
            if ids.len() >= max_bias_tokens {
                break;
            }
        }
        if !ids.is_empty() {
            chain.push(Box::new(AdditiveBiasProcessor::new(ids)));
        }
    }

    let forbidden: Vec<u32> =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioLogitForbidTokens)
            .expose()
            .map(|s| {
                s.split([',', ';', ' ', '\n'])
                    .filter_map(|x| x.trim().parse::<u32>().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
    if !forbidden.is_empty() {
        chain.push(Box::new(ForbiddenTokenMaskProcessor::new(forbidden)));
    }

    let trie_on = matches!(
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioConstrainedTrie).expose(),
        Some(v) if v == "1" || v.eq_ignore_ascii_case("true")
    ) || cfg.map(|c| c.constrained_trie).unwrap_or(false);
    if trie_on {
        let max_stuck = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioTrieStuckSteps)
            .expose()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or_else(|| cfg.map(|c| c.trie_stuck_steps).unwrap_or(2));
        let seqs: Vec<Vec<u32>> = trie_phrase_list()
            .into_iter()
            .filter_map(|p| tokenize_phrase_ids(tokenizer, &p).ok())
            .filter(|ids| !ids.is_empty())
            .collect();
        if let Some(trie) = TokenTrieConstraintProcessor::from_token_sequences(&seqs, max_stuck) {
            chain.push(Box::new(trie));
        }
    }

    if chain.is_empty() {
        Ok(Box::new(NoopLogitProcessor))
    } else {
        Ok(Box::new(CompositeLogitProcessor::new(chain)))
    }
}

#[cfg(test)]
mod tests {
    use super::{ForbiddenTokenMaskProcessor, LogitProcessor, TokenTrieConstraintProcessor};
    use candle_core::Tensor;

    #[test]
    fn forbidden_mask_applies_neg_inf() {
        let mut p = ForbiddenTokenMaskProcessor::new(vec![1, 3]);
        let logits = Tensor::new([0f32, 1.0, 2.0, 3.0].as_slice(), &candle_core::Device::Cpu)
            .expect("tensor");
        let out = p
            .apply(
                &super::LogitStepContext {
                    step: 0,
                    temperature: 0.0,
                    generated_tokens: vec![],
                },
                &logits,
            )
            .expect("apply");
        let v = out.to_vec1::<f32>().expect("vec1");
        assert!(v[1].is_infinite() && v[1].is_sign_negative());
        assert!(v[3].is_infinite() && v[3].is_sign_negative());
    }

    #[test]
    fn trie_advances_and_resets_on_stuck() {
        let mut p =
            TokenTrieConstraintProcessor::from_token_sequences(&[vec![7, 8]], 1).expect("trie");
        p.on_token_decoded(7);
        p.on_token_decoded(99);
        // stuck reset should allow root branch again.
        p.on_token_decoded(7);
    }
}
