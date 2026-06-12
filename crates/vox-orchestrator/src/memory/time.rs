use chrono::Utc;

/// Returns the current date as `YYYY-MM-DD`.
pub(super) fn today_str() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

/// Returns the previous date as `YYYY-MM-DD`.
#[allow(dead_code)]
pub(super) fn yesterday_str() -> String {
    (Utc::now() - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string()
}

/// Current HH:MM:SS timestamp.
#[allow(dead_code)]
pub(super) fn timestamp_hms() -> String {
    Utc::now().format("%H:%M:%S").to_string()
}
