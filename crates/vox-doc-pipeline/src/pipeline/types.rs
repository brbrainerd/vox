//! Doc pipeline data structures.

use std::path::PathBuf;

#[derive(Debug)]
pub struct LintError {
    pub file: PathBuf,
    pub line: usize,
    pub kind: LintKind,
}

#[derive(Debug)]
pub enum LintKind {
    UnclosedCodeFence,
    ShortCodeFence {
        backticks: usize,
        at_line: usize,
    },
    GenericDescription,
    MissingFrontmatter,
    MissingCategory,
    MissingTrainingRationale,
    UnknownCategory {
        value: String,
    },
    UnknownStatus {
        value: String,
    },
    UnknownSchemaType {
        value: String,
    },
    BrokenIncludeAnchor {
        file: String,
        anchor: String,
    },
    BrokenIncludeFile {
        file: String,
    },
    WholeFileIncludeHasTrainingHeader {
        file: String,
    },
    DocTestFailed {
        msg: String,
    },
    UnlabeledCodeFence {
        at_line: usize,
    },
    /// Second YAML frontmatter block detected (usually an accidental merge).
    DuplicateFrontmatter {
        second_block_start_line: usize,
    },
    /// A hand-authored `last_updated:` key is present at all; the pipeline derives this from Git.
    HandAuthoredLastUpdated,
}
