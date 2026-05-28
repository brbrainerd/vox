//! Shared progress bar utilities for vox-cli.
//! All bars draw to stderr by default (indicatif default).

use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

/// Custom color-aware progress style for compiler stages.
fn build_style() -> ProgressStyle {
    let template = if crate::diagnostics::should_color_stderr() {
        "{prefix:>12.cyan.bold} {msg}"
    } else {
        "{prefix:>12} {msg}"
    };
    ProgressStyle::with_template(template).unwrap()
}

/// Custom spinner style for indeterminate operations.
fn spinner_style() -> ProgressStyle {
    let template = if crate::diagnostics::should_color_stderr() {
        "{spinner:.cyan.bold} {msg}"
    } else {
        "- {msg}"
    };
    ProgressStyle::with_template(template)
        .unwrap()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
}

/// Progress indicator for build/compile stages.
pub struct BuildProgress {
    pb: ProgressBar,
}

impl BuildProgress {
    /// Create a new build progress spinner for the given stage (e.g., "Compiling").
    pub fn new(stage: &str) -> Self {
        let pb = ProgressBar::new_spinner();
        pb.set_style(build_style());
        pb.set_prefix(stage.to_string());
        pb.enable_steady_tick(vox_config::timeouts::D_80MS);
        Self { pb }
    }

    /// Set the current message (e.g., the file being compiled).
    pub fn set_message(&self, msg: &str) {
        self.pb.set_message(msg.to_string());
    }

    /// Finish with a success message and clear the spinner.
    pub fn finish_success(&self, msg: &str) {
        self.pb.finish_and_clear();
        crate::diagnostics::print_success(msg);
    }

    /// Finish with an error message and clear the spinner.
    pub fn finish_error(&self, msg: &str) {
        self.pb.finish_and_clear();
        crate::diagnostics::print_error(msg);
    }
}

/// Generic spinner for long-running CLI operations.
pub struct SpinnerProgress {
    pb: ProgressBar,
}

impl SpinnerProgress {
    /// Create a new generic spinner with the given message.
    pub fn new(msg: &str) -> Self {
        let pb = ProgressBar::new_spinner();
        pb.set_style(spinner_style());
        pb.set_message(msg.to_string());
        pb.enable_steady_tick(vox_config::timeouts::D_80MS);
        Self { pb }
    }

    /// Finish with a final message and clear the spinner.
    pub fn finish_with(&self, msg: &str) {
        self.pb.finish_and_clear();
        crate::diagnostics::print_success(msg);
    }

    /// Just clear the spinner without printing anything.
    pub fn finish_and_clear(&self) {
        self.pb.finish_and_clear();
    }
}
