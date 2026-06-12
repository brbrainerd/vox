//! Lightweight identity summary for the sidebar footer.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct IdentitySummaryDto {
    pub display_name: String,
    pub os_user: Option<String>,
}

#[tauri::command]
pub fn get_identity_summary() -> IdentitySummaryDto {
    let os_user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
        .filter(|s| !s.trim().is_empty());
    let display_name = os_user.clone().unwrap_or_else(|| "operator".to_string());
    IdentitySummaryDto {
        display_name: format!("{display_name}@vox"),
        os_user,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_summary_has_display_name() {
        let s = get_identity_summary();
        assert!(s.display_name.contains('@'));
    }
}
