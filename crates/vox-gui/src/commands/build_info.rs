use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct GuiBuildInfo {
    pub version: String,
    pub build_number: String,
    pub git_hash: String,
    pub display: String,
}

#[tauri::command]
pub async fn get_build_info() -> Result<GuiBuildInfo, String> {
    let version = env!("CARGO_PKG_VERSION").to_string();
    let build_number = option_env!("VOX_BUILD_NUMBER")
        .unwrap_or("dev")
        .to_string();
    let git_hash = option_env!("VOX_GIT_HASH").unwrap_or("unknown").to_string();
    Ok(GuiBuildInfo {
        display: format!("{version}+build.{build_number} ({git_hash})"),
        version,
        build_number,
        git_hash,
    })
}
