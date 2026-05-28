use crate::claim_extractor::types::VerifiabilityClass;

#[derive(Debug, Clone)]
pub struct VeriScoreResult {
    pub score: f64,
    pub class: VerifiabilityClass,
}

#[derive(Debug, Clone)]
pub struct VeriScoreConfig {
    pub min_score: f64,
}

impl Default for VeriScoreConfig {
    fn default() -> Self {
        Self { min_score: 0.5 }
    }
}

#[derive(Default)]
pub struct VeriScoreGate {
    pub config: VeriScoreConfig,
}

impl VeriScoreGate {
    pub fn new(config: VeriScoreConfig) -> Self {
        Self { config }
    }

    pub fn score_sentence(&self, sentence: &str) -> VeriScoreResult {
        let lower = sentence.to_ascii_lowercase();

        let hedge_phrases = [
            "may be",
            "might be",
            "could be",
            "possibly",
            "perhaps",
            "it seems",
            "it appears",
            "in some cases",
            "potentially",
            "likely",
            "unlikely",
            "we believe",
            "we think",
            "we hypothesize",
            "it is possible",
        ];
        let future_phrases = [
            "future work",
            "future research",
            "will explore",
            "could explore",
            "plan to",
            "aim to",
            "intend to",
            "leave for future",
        ];
        let motivation_phrases = [
            "motivated by",
            "inspired by",
            "building on",
            "as noted above",
            "we propose",
            "this paper presents",
            "in this work we",
        ];

        let hedge_score: f64 =
            hedge_phrases.iter().filter(|p| lower.contains(*p)).count() as f64 * 0.25;
        let future_score: f64 =
            future_phrases.iter().filter(|p| lower.contains(*p)).count() as f64 * 0.4;
        let motivation_score: f64 = motivation_phrases
            .iter()
            .filter(|p| lower.contains(*p))
            .count() as f64
            * 0.3;
        let unverifiable_penalty = (hedge_score + future_score + motivation_score).min(1.0);

        let has_number = sentence.chars().any(|c| c.is_ascii_digit());
        let numeric_phrases = [
            "ms",
            "seconds",
            "%",
            "percent",
            "increased by",
            "decreased by",
            "improved by",
            "reduced by",
            "p95",
            "p99",
            "latency",
            "throughput",
            "tokens/s",
        ];
        let numeric_score: f64 = if numeric_phrases.iter().any(|p| lower.contains(p)) {
            0.4
        } else {
            0.0
        };
        let number_bonus = if has_number { 0.3 } else { 0.0 };

        let verifiable_signal = (numeric_score + number_bonus).min(0.6);
        let base_score = 0.5 + verifiable_signal - unverifiable_penalty;
        let score = base_score.clamp(0.0, 1.0);

        let class = if score < self.config.min_score {
            VerifiabilityClass::Unverifiable
        } else if numeric_score > 0.0 || has_number {
            VerifiabilityClass::Numeric
        } else {
            VerifiabilityClass::Semantic
        };

        VeriScoreResult { score, class }
    }

    pub fn filter_sentences<'a>(&self, sentences: &'a [String]) -> Vec<(&'a str, VeriScoreResult)> {
        sentences
            .iter()
            .map(|s| (s.as_str(), self.score_sentence(s)))
            .filter(|(_, r)| r.score >= self.config.min_score)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_claim_is_verifiable() {
        let gate = VeriScoreGate::default();
        let r = gate.score_sentence(
            "Provider X p95 latency increased by 12ms after the 2026-04 model update.",
        );
        assert!(r.score >= 0.7);
        assert_ne!(r.class, VerifiabilityClass::Unverifiable);
    }

    #[test]
    fn future_work_sentence_is_unverifiable() {
        let gate = VeriScoreGate::default();
        let r = gate.score_sentence("Future work could explore whether this approach generalizes.");
        assert!(r.score < 0.5);
    }

    #[test]
    fn hedge_sentence_is_unverifiable() {
        let gate = VeriScoreGate::default();
        let r = gate.score_sentence(
            "It may be possible that some improvements exist in certain scenarios.",
        );
        assert!(r.score < 0.5);
    }
}
