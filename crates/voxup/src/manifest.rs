use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceToolchain {
    pub schema: String,
    pub versions: HashMap<String, String>,
    pub targets: HashMap<String, Vec<String>>,
    pub components: HashMap<String, Vec<String>>,
}

impl WorkspaceToolchain {
    pub fn parse(content: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_toolchain_yaml() {
        let yaml = "schema: '1'\nversions: {}\ntargets: {}\ncomponents: {}\n";
        let tc = WorkspaceToolchain::parse(yaml).unwrap();
        assert_eq!(tc.schema, "1");
        assert!(tc.versions.is_empty());
    }
}
