//! `vox ci toestub-budget` — read toestub JSON from stdin and enforce the
//! `rust_parse_failures` cap set by `VOX_TOESTUB_MAX_RUST_PARSE_FAILURES`.
//!
//! Replaces the `python3 -c "…"` one-liner that previously appeared in ci.yml.

use anyhow::{Result, anyhow};
use serde_json::Value;
use std::io::Read;

const DEFAULT_CAP: u64 = 3;
const CAP_ENV: &str = "VOX_TOESTUB_MAX_RUST_PARSE_FAILURES";

pub fn run() -> Result<()> {
    let cap: u64 = std::env::var(CAP_ENV)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_CAP);

    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;

    let v: Value = serde_json::from_str(&input)
        .map_err(|e| anyhow!("toestub-budget: could not parse JSON from stdin: {e}"))?;

    let failures = v
        .get("rust_parse_failures")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);

    if failures > cap {
        eprintln!("rust_parse_failures={failures} exceeds cap {cap}");
        return Err(anyhow!(
            "toestub-budget: rust_parse_failures={failures} exceeds cap {cap}"
        ));
    }

    println!("rust_parse_failures={failures} (cap {cap}) OK");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn within_cap_ok() {
        let json = r#"{"rust_parse_failures": 2}"#;
        let v: Value = serde_json::from_str(json).unwrap();
        let failures = v
            .get("rust_parse_failures")
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        assert!(failures <= DEFAULT_CAP);
    }

    #[test]
    fn over_cap_detected() {
        let json = r#"{"rust_parse_failures": 99}"#;
        let v: Value = serde_json::from_str(json).unwrap();
        let failures = v
            .get("rust_parse_failures")
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        assert!(failures > DEFAULT_CAP);
    }
}
