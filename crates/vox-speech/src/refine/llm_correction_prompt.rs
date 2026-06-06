//! LLM prompt generation for Oratio's correction pipeline.

/// Build the user prompt requesting JSON output for ASR refinement.
#[must_use]
pub fn build_llm_correction_prompt(raw: &str, refined: &str, confidence: f32) -> String {
    format!(
        "Normalize this speech-to-text line for a Rust CLI. Preserve paths, --flags, and :: tokens.\n\n\
Original text:\n{}\n\n\
Raw ASR:\n{}\n\n\
Deterministic confidence: {}\n\n\
Reply with ONLY compact JSON (no markdown) matching this shape:\n\
{{\"corrected_text\":string,\"confidence\":number between 0 and 1,\"changes\":[{{\"before\":string,\"after\":string}}],\"keep_original\":boolean}}\n",
        refined, raw, confidence
    )
}

/// The system prompt defining the LLM persona for ASR correction.
#[must_use]
pub fn llm_system_prompt() -> &'static str {
    "You correct ASR transcripts. Output JSON only; booleans lowercase; confidence between 0 and 1 inclusive."
}

/// Check if the LLM output is drastically longer than the raw input.
#[must_use]
pub fn hallucination_guard(llm_out: &str, raw_len: usize) -> Option<String> {
    if llm_out.len() > raw_len * 3 + 20 {
        None
    } else {
        Some(llm_out.to_string())
    }
}
