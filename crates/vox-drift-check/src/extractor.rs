use crate::features::ExtractedFeatures;
use anyhow::Result;
use std::path::Path;

pub trait LanguageExtractor: Send + Sync {
    fn extract(&self, path: &Path, content: &str) -> Result<ExtractedFeatures>;
}

/// Extract crate name from path like `crates/vox-foo/src/lib.rs` → `"vox-foo"`.
pub fn crate_name_from_path(path: &Path) -> Option<String> {
    let mut components = path.components().peekable();
    while let Some(part) = components.next() {
        let part_str = part.as_os_str().to_string_lossy();
        if part_str == "crates"
            && let Some(name_part) = components.next()
        {
            let name = name_part.as_os_str().to_string_lossy().into_owned();
            return Some(name);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn crate_name_from_path_under_crates() {
        let p = Path::new("crates/vox-config/src/lib.rs");
        assert_eq!(crate_name_from_path(p), Some("vox-config".to_string()));
    }

    #[test]
    fn crate_name_from_path_unknown() {
        let p = Path::new("apps/my-app/index.ts");
        assert_eq!(crate_name_from_path(p), None);
    }
}
