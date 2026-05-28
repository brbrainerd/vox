//! Diagnostic output helpers for the Vox CLI.
//!
//! Provides consistent formatting for errors, warnings, hints, and deprecation
//! notices. All diagnostic output goes to **stderr** so that stdout remains
//! machine-parseable (e.g. for `--json` or piped output).
//!
//! # Formatting contract
//! - `error:`   → red bold prefix, non-zero exit expected
//! - `warning:` → yellow bold prefix
//! - `hint:`    → cyan bold prefix
//! - Deprecated → yellow `⚠` prefix with migration hint

use owo_colors::OwoColorize;

pub use vox_cli_core::diagnostics::{
    ColorChoice, set_color_choice, should_color_stderr, should_color_stdout,
};

// ── Deprecation ───────────────────────────────────────────────────────────────

/// Emit a deprecation warning to stderr telling the user the new canonical command.
///
/// Used by hidden root-level aliases that delegate to subcommand groups.
pub fn warn_deprecated(old: &str, new: &str) {
    if should_color_stderr() {
        eprintln!(
            "{} `vox {}` is deprecated — use `vox {}` instead.",
            "⚠".yellow().bold(),
            old,
            new.cyan().bold()
        );
    } else {
        eprintln!("⚠ `vox {}` is deprecated — use `vox {}` instead.", old, new);
    }
}

// ── Structured diagnostics ────────────────────────────────────────────────────

/// Print a formatted error to stderr.
///
/// ```text
/// error: could not open 'foo.vox': No such file or directory
///   hint: check the path with `ls -la foo.vox`
/// ```
pub fn print_error(message: &str) {
    if should_color_stderr() {
        let prefix = if std::env::var("VOX_CVD_SAFE").is_ok() {
            "❌ error:".magenta().bold().to_string()
        } else {
            "❌ error:".red().bold().to_string()
        };
        eprintln!("{} {message}", prefix);
    } else {
        eprintln!("❌ error: {message}");
    }
}

/// Print a formatted error with an actionable hint.
pub fn print_error_with_hint(message: &str, hint: &str) {
    if should_color_stderr() {
        let prefix = if std::env::var("VOX_CVD_SAFE").is_ok() {
            "❌ error:".magenta().bold().to_string()
        } else {
            "❌ error:".red().bold().to_string()
        };
        eprintln!("{} {message}", prefix);
        eprintln!("  {} {hint}", "💡 hint:".cyan().bold());
    } else {
        eprintln!("❌ error: {message}");
        eprintln!("  💡 hint: {hint}");
    }
}

/// Print a warning to stderr.
pub fn print_warning(message: &str) {
    if should_color_stderr() {
        eprintln!("{} {message}", "⚠️ warning:".yellow().bold());
    } else {
        eprintln!("⚠️ warning: {message}");
    }
}

/// Print a warning with an actionable hint.
pub fn print_warning_with_hint(message: &str, hint: &str) {
    if should_color_stderr() {
        eprintln!("{} {message}", "⚠️ warning:".yellow().bold());
        eprintln!("  {} {hint}", "💡 hint:".cyan().bold());
    } else {
        eprintln!("⚠️ warning: {message}");
        eprintln!("  💡 hint: {hint}");
    }
}

/// Print a hint/info line to stderr (non-error context).
pub fn print_hint(message: &str) {
    if should_color_stderr() {
        eprintln!("  {} {message}", "💡 hint:".cyan().bold());
    } else {
        eprintln!("  💡 hint: {message}");
    }
}

/// Print an actionable "Next step" hint to stderr (e.g. after successful build).
pub fn print_next_step(message: &str) {
    if should_color_stderr() {
        eprintln!("  {} {message}", "→".cyan().bold());
    } else {
        eprintln!("  → {message}");
    }
}

/// Print a suggestion to run with backtrace enabled when an error occurs.
pub fn print_backtrace_hint() {
    if std::env::var("RUST_BACKTRACE").is_err() {
        print_hint("Run with `RUST_BACKTRACE=1` for a full backtrace.");
    }
}

// ── Did-you-mean ──────────────────────────────────────────────────────────────

/// Suggest the closest matching string from a list of candidates using
/// Levenshtein distance. Returns `None` if no candidate is within 3 edits.
///
/// # Examples
/// ```
/// use vox_cli::diagnostics::did_you_mean;
/// assert_eq!(did_you_mean("buid", &["build", "bundle", "check"]), Some("build".to_string()));
/// ```
pub fn did_you_mean(input: &str, candidates: &[&str]) -> Option<String> {
    let best = candidates
        .iter()
        .filter_map(|c| {
            let dist = levenshtein(input, c);
            // Only suggest if fewer than 3 edits AND the match is meaningful
            // (not all characters differ, i.e. dist < length of shorter string)
            let max_len = input.len().min(c.len());
            if dist < 3 || (dist == 3 && max_len > 4) {
                Some((dist, *c))
            } else {
                None
            }
        })
        .min_by_key(|(d, _)| *d);
    best.map(|(_, c)| c.to_string())
}

/// Print a "Did you mean?" suggestion if a close match exists.
pub fn suggest_did_you_mean(input: &str, candidates: &[&str]) {
    if let Some(suggestion) = did_you_mean(input, candidates) {
        if should_color_stderr() {
            eprintln!(
                "  {} Did you mean `{}`?",
                "💡 hint:".cyan().bold(),
                suggestion.bold()
            );
        } else {
            eprintln!("  💡 hint: Did you mean `{}`?", suggestion);
        }
    }
}

/// Compute Levenshtein edit distance between two strings.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let m = a.len();
    let n = b.len();
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for (i, row) in dp.iter_mut().enumerate().take(m + 1) {
        row[0] = i;
    }
    for (j, cell) in dp[0].iter_mut().enumerate().take(n + 1) {
        *cell = j;
    }

    for i in 1..=m {
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[m][n]
}

// ── Terminal success / info ───────────────────────────────────────────────────

/// Print a success line (green ✓) to stdout.
pub fn print_success(message: &str) {
    if should_color_stdout() {
        let prefix = if std::env::var("VOX_CVD_SAFE").is_ok() {
            "✓".cyan().bold().to_string()
        } else {
            "✓".green().bold().to_string()
        };
        println!("{} {message}", prefix);
    } else {
        println!("✓ {message}");
    }
}

/// Print a success line with a duration timestamp.
pub fn print_success_with_time(message: &str, duration: std::time::Duration) {
    if should_color_stdout() {
        let prefix = if std::env::var("VOX_CVD_SAFE").is_ok() {
            "✓".cyan().bold().to_string()
        } else {
            "✓".green().bold().to_string()
        };
        println!(
            "{} {message} {}",
            prefix,
            format!("({:.2?})", duration).truecolor(128, 128, 128)
        );
    } else {
        println!("✓ {message} ({:.2?})", duration);
    }
}

/// Print a neutral info line to stdout (no color, plain).
pub fn print_info(message: &str) {
    println!("{message}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levenshtein_exact() {
        assert_eq!(levenshtein("build", "build"), 0);
    }

    #[test]
    fn levenshtein_one_edit() {
        assert_eq!(levenshtein("buid", "build"), 1);
    }

    #[test]
    fn levenshtein_completely_different() {
        // "abc" → "xyz": 3 substitutions = distance 3
        assert_eq!(levenshtein("abc", "xyz"), 3);
        // Longer strings with nothing in common score higher
        assert!(levenshtein("hello", "zzzzz") >= 4);
    }

    #[test]
    fn did_you_mean_finds_close() {
        let candidates = ["build", "bundle", "check", "run"];
        assert_eq!(did_you_mean("buid", &candidates), Some("build".to_string()));
    }

    #[test]
    fn did_you_mean_no_match() {
        let candidates = ["build", "bundle"];
        assert_eq!(did_you_mean("xyz", &candidates), None);
    }

    #[test]
    fn did_you_mean_exact_match() {
        let candidates = ["build", "bundle"];
        assert_eq!(
            did_you_mean("build", &candidates),
            Some("build".to_string())
        );
    }
}
