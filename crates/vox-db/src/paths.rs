//! Cross-platform data directory resolution for Vox.
//!
//! Delegates to `vox_config` for a single source of truth. Re-exports for backward compatibility.

pub use vox_config::{
    config_dir, data_dir, default_db_path, local_user_id, state_dir, APP_DIR_NAME,
    DEFAULT_DB_FILENAME,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_returns_some() {
        let dir = data_dir();
        assert!(dir.is_some(), "data_dir() should resolve on this platform");
        let path = dir.unwrap();
        assert!(
            path.to_str().unwrap().contains("vox"),
            "path should contain 'vox'"
        );
    }

    #[test]
    fn default_db_path_has_filename() {
        let path = default_db_path().expect("should resolve");
        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            DEFAULT_DB_FILENAME
        );
    }

    #[test]
    fn env_override_works() {
        let tmp = std::env::temp_dir().join("vox_paths_test_override");
        std::env::set_var("VOX_DATA_DIR", tmp.to_str().unwrap());
        let dir = data_dir().expect("should resolve");
        assert_eq!(dir, tmp);
        std::env::remove_var("VOX_DATA_DIR");
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn local_user_id_not_empty() {
        let id = local_user_id();
        assert!(!id.is_empty(), "local_user_id() should never be empty");
    }

    #[test]
    fn state_dir_creates_subdirectory() {
        let dir = state_dir().expect("should resolve");
        assert!(dir.ends_with("state"));
    }
}
