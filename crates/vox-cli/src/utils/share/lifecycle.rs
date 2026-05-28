//! Duration lifecycle: countdown printing + auto-shutdown signal.
//!
//! Runs as a background task spawned by `ShareSession::start()`.
//! Communicates shutdown via a `tokio::sync::mpsc` channel.

use std::time::Duration;
use tokio::sync::mpsc;

/// Runs the countdown lifecycle task. Sends a unit on `done_tx` when duration elapses.
/// Prints countdown to stdout.
pub async fn run_countdown(duration: Duration, done_tx: mpsc::Sender<()>) {
    // Sleep until the deadline using tokio's timer directly, which correctly handles
    // very short durations (including sub-second ones used in tests).
    tokio::time::sleep(duration).await;

    // Duration has elapsed — signal the session.
    let _ = done_tx.send(()).await;
}

/// Runs the countdown printing loop independently of the shutdown signal.
/// Prints time remaining to stdout on the schedule described in the module docs.
pub async fn run_countdown_printer(duration: Duration) {
    let deadline = tokio::time::Instant::now() + duration;

    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return;
        }
        // Print countdown when ≤ 1h remaining (every ~minute) or every ~5 minutes otherwise.
        let secs = remaining.as_secs();
        let should_print = if secs <= 3600 {
            // Within the last hour — print every minute (we tick every 30s, so every other tick).
            true
        } else {
            // Further out — print every 5 minutes.
            secs % 300 < 30
        };
        if should_print {
            println!("[vox share] {} remaining", format_duration(remaining));
        }
    }
}

/// Format a duration for user display: "2h 30m", "45m", "30s".
pub fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    if hours > 0 && mins > 0 {
        format!("{}h {}m", hours, mins)
    } else if hours > 0 {
        format!("{}h", hours)
    } else if mins > 0 {
        format!("{}m", mins)
    } else {
        format!("{}s", secs)
    }
}

#[cfg(test)]
mod tests {
    use super::format_duration;
    use std::time::Duration;

    #[test]
    fn format_hours_and_minutes() {
        assert_eq!(format_duration(Duration::from_secs(9000)), "2h 30m");
    }
    #[test]
    fn format_hours_only() {
        assert_eq!(format_duration(Duration::from_secs(7200)), "2h");
    }
    #[test]
    fn format_minutes_only() {
        assert_eq!(format_duration(Duration::from_secs(90)), "1m");
    }
    #[test]
    fn format_seconds_only() {
        assert_eq!(format_duration(Duration::from_secs(45)), "45s");
    }
}
