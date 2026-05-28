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
