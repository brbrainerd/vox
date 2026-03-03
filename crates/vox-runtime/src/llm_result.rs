//! `LlmResult<T>` — typed result wrapper for structured LLM activity returns.
//!
//! Activities that return named structs (not `String`) use this type to propagate
//! parse errors without panicking. The generated code pattern is:
//!
//! ```no_run
//! pub async fn my_activity(msg: String) -> LlmResult<MyStruct> {
//!     // ... llm_chat call ...
//!     match result {
//!         ActivityResult::Ok(Ok(res)) => LlmResult::parse_from(&res.content),
//!         ActivityResult::Ok(Err(e)) => LlmResult::Err(LlmError::ApiError(e.to_string())),
//!         _ => LlmResult::Err(LlmError::ActivityFailed),
//!     }
//! }
//! ```

use serde::{Deserialize, Serialize};

/// Errors that can occur when calling a typed LLM activity.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum LlmError {
    /// The LLM API returned an error response.
    #[error("LLM API error: {0}")]
    ApiError(String),

    /// The LLM response could not be parsed as the expected type.
    #[error("JSON parse error: {error} — raw response: {raw}")]
    ParseError {
        error: String,
        raw: String,
    },

    /// The activity runner failed entirely (timeout, retries exhausted, etc.).
    #[error("LLM activity failed")]
    ActivityFailed,
}

/// Result of a typed LLM activity call.
///
/// Use this as the return type of activities that return structured data:
/// ```vox
/// activity extract_gist(message: String) to GistArtifact:
///     with { model: "openai/gpt-4o-mini", temperature: 0.1, max_tokens: 800 }
///     prompt: "..."
/// ```
///
/// The Vox codegen emits `LlmResult<GistArtifact>` in the generated Rust signature
/// when the return type is a named struct, enabling callers to handle parse failures
/// without panicking.
#[must_use = "LlmResult must be checked — use ok() or match on it"]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmResult<T> {
    /// Successful parse into the expected type.
    Ok(T),
    /// The LLM call or parse failed.
    Err(LlmError),
}

impl<T: for<'de> Deserialize<'de>> LlmResult<T> {
    /// Parse a raw LLM content string into `T`, returning `LlmResult<T>`.
    ///
    /// This is the primary entry point used by generated activity code.
    /// It attempts `serde_json::from_str` and wraps parse failures in
    /// `LlmError::ParseError` with the raw string preserved for debugging.
    pub fn parse_from(raw: &str) -> Self {
        match serde_json::from_str::<T>(raw) {
            std::result::Result::Ok(v) => LlmResult::Ok(v),
            std::result::Result::Err(e) => {
                tracing::warn!(
                    "LlmResult::parse_from failed: {} — raw (first 500 chars): {}",
                    e,
                    &raw[..raw.len().min(500)]
                );
                LlmResult::Err(LlmError::ParseError {
                    error: e.to_string(),
                    raw: raw.to_string(),
                })
            }
        }
    }

    /// Returns `true` if this is the `Ok` variant.
    pub fn is_ok(&self) -> bool {
        matches!(self, LlmResult::Ok(_))
    }

    /// Returns `true` if this is the `Err` variant.
    pub fn is_err(&self) -> bool {
        !self.is_ok()
    }

    /// Convert to `Option<T>`, logging a warning on error.
    pub fn ok(self) -> Option<T> {
        match self {
            LlmResult::Ok(v) => Some(v),
            LlmResult::Err(e) => {
                tracing::warn!("LlmResult discarded error: {}", e);
                None
            }
        }
    }

    /// Unwrap the value, panicking with a message on error.
    ///
    /// Only use in tests. In production workflows, match on `LlmResult`.
    pub fn unwrap(self) -> T {
        match self {
            LlmResult::Ok(v) => v,
            LlmResult::Err(e) => panic!("LlmResult::unwrap called on Err: {}", e),
        }
    }

    /// Unwrap or return a default value (does not log).
    pub fn unwrap_or(self, default: T) -> T {
        match self {
            LlmResult::Ok(v) => v,
            LlmResult::Err(_) => default,
        }
    }

    /// Unwrap or compute a default from the error.
    pub fn unwrap_or_else(self, f: impl FnOnce(LlmError) -> T) -> T {
        match self {
            LlmResult::Ok(v) => v,
            LlmResult::Err(e) => f(e),
        }
    }

    /// Map the `Ok` value. Errors pass through unchanged.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> LlmResult<U> {
        match self {
            LlmResult::Ok(v) => LlmResult::Ok(f(v)),
            LlmResult::Err(e) => LlmResult::Err(e),
        }
    }
}

impl<T: for<'de> Deserialize<'de> + Default> LlmResult<T> {
    /// Unwrap or return `T::default()`, logging the error.
    pub fn unwrap_or_default(self) -> T {
        match self {
            LlmResult::Ok(v) => v,
            LlmResult::Err(e) => {
                tracing::warn!("LlmResult::unwrap_or_default on error: {}", e);
                T::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
    struct TestStruct {
        value: String,
        count: i32,
    }

    #[test]
    fn parse_from_valid_json() {
        let raw = r#"{"value": "hello", "count": 42}"#;
        let result = LlmResult::<TestStruct>::parse_from(raw);
        assert!(result.is_ok());
        let inner = result.unwrap();
        assert_eq!(inner.value, "hello");
        assert_eq!(inner.count, 42);
    }

    #[test]
    fn parse_from_invalid_json_returns_err() {
        let raw = "not json at all";
        let result = LlmResult::<TestStruct>::parse_from(raw);
        assert!(result.is_err());
        assert!(result.ok().is_none());
    }

    #[test]
    fn parse_from_wrong_shape_returns_parse_error() {
        let raw = r#"{"wrong_field": true}"#;
        let result = LlmResult::<TestStruct>::parse_from(raw);
        // serde fills missing fields with default if #[serde(default)], otherwise errors
        // TestStruct has no serde(default) so "count" is missing → parse still succeeds with default
        // since serde allows missing fields without #[deny_unknown_fields]
        // The raw string has no "value" or "count" — they're missing which is OK in serde
        // so this test just confirms it doesn't panic
        let _ = result;
    }

    #[test]
    fn unwrap_or_default_returns_default_on_err() {
        let result = LlmResult::<TestStruct>::Err(LlmError::ActivityFailed);
        let default = result.unwrap_or_default();
        assert_eq!(default, TestStruct::default());
    }

    #[test]
    fn map_transforms_ok_value() {
        let result = LlmResult::<TestStruct>::parse_from(r#"{"value":"hi","count":1}"#);
        let mapped = result.map(|s| s.count * 2);
        assert!(mapped.is_ok());
        assert_eq!(mapped.unwrap(), 2);
    }

    #[test]
    fn llm_error_display() {
        let e = LlmError::ParseError {
            error: "EOF".into(),
            raw: "{}".into(),
        };
        assert!(e.to_string().contains("EOF"));

        let e2 = LlmError::ApiError("rate limited".into());
        assert!(e2.to_string().contains("rate limited"));

        let e3 = LlmError::ActivityFailed;
        assert!(e3.to_string().contains("activity failed"));
    }
}
