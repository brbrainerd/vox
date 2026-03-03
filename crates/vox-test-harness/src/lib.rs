use serde::{Deserialize, Serialize};

/// Represents a structured test failure diagnosis.
#[derive(Debug, Serialize, Deserialize)]
pub struct TestDiagnosis {
    pub test: String,
    pub crate_name: String,
    pub file: String,
    pub line: usize,
    pub category: TestCategory,
    pub expected: Option<String>,
    pub actual: Option<String>,
    #[serde(default)]
    pub related_decls: Vec<String>,
    #[serde(default)]
    pub suggested_files: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestCategory {
    Lexer,
    Parser,
    Typeck,
    CodegenTs,
    CodegenRust,
    Orchestrator,
    Runtime,
    Mcp,
    Unknown,
}

impl TestDiagnosis {
    /// Creates a new test diagnosis with JSON serialization helper
    pub fn new(
        test: impl Into<String>,
        crate_name: impl Into<String>,
        file: impl Into<String>,
        line: usize,
        category: TestCategory,
    ) -> Self {
        Self {
            test: test.into(),
            crate_name: crate_name.into(),
            file: file.into(),
            line,
            category,
            expected: None,
            actual: None,
            related_decls: Vec::new(),
            suggested_files: Vec::new(),
        }
    }

    /// Set expected and actual outputs for diff analysis
    pub fn with_diff(mut self, expected: impl Into<String>, actual: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self.actual = Some(actual.into());
        self
    }

    /// Output the diagnosis as structured JSON to stdout so that calling tooling (like Nextest or VoxDoctor) can parse it.
    pub fn emit_json(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            println!("VOX_DIAGNOSIS: {}", json);
        }
    }
}
