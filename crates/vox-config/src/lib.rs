//! Centralized configuration for Vox: env vars, defaults, and path resolution.
//!
//! Precedence: CLI args > env > config file > defaults.

pub mod paths;
pub mod config;

pub use paths::{
    config_dir, data_dir, default_db_path, local_user_id, state_dir, APP_DIR_NAME,
    DEFAULT_DB_FILENAME,
};
pub use config::VoxConfig;


/// Minimum Vox MCP server version required for full agent capability.
pub const VOX_MCP_MIN_VERSION: &str = ">=0.2.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_version() {
        assert_eq!(VOX_MCP_MIN_VERSION, ">=0.2.0");
    }

    #[test]
    fn test_path_constants() {
        assert_eq!(APP_DIR_NAME, "vox");
        assert_eq!(DEFAULT_DB_FILENAME, "vox_skills.db");
    }
}
