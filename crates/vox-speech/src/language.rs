//! Language hint normalization and diagnostics for Whisper backends.

use serde_json::{Value, json};

/// Normalize `language_hint` for Candle Whisper (ISO-ish codes) and return JSON diagnostics.
///
/// Invalid hints are **not** forwarded to the backend (returns `None`) while still recording the raw value.
#[must_use]
pub fn prepare_language_hint(hint: Option<&str>) -> (Value, Option<String>) {
    let Some(raw) = hint.map(str::trim).filter(|s| !s.is_empty()) else {
        return (
            json!({
                "raw": null,
                "normalized": null,
                "notes": [],
            }),
            None,
        );
    };

    let lower = raw.to_ascii_lowercase();
    let mut notes = Vec::<String>::new();

    let mut valid = true;
    if lower.len() < 2 || lower.len() > 16 {
        valid = false;
        notes.push("language_hint_length_out_of_range".to_string());
    }
    if !lower
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        valid = false;
        notes.push("language_hint_invalid_charset".to_string());
    }

    if valid {
        tracing::debug!(
            target: "vox_oratio_language",
            raw_hint = raw,
            normalized = lower.as_str(),
            "language_hint accepted"
        );
        (
            json!({
                "raw": raw,
                "normalized": lower,
                "notes": notes,
            }),
            Some(lower),
        )
    } else {
        tracing::debug!(
            target: "vox_oratio_language",
            raw_hint = raw,
            "language_hint rejected; falling back to backend defaults"
        );
        (
            json!({
                "raw": raw,
                "normalized": null,
                "notes": notes,
            }),
            None,
        )
    }
}
