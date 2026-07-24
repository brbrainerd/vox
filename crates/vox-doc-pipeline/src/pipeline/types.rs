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
    /// index.mdx's SYNC-FROM-README block content differs from README's matching ANCHOR block,
    /// after known intentional link-scheme transforms are applied.
    ReadmeSyncDrift {
        block: String,
    },
    /// README.md has an ANCHOR block but index.mdx has no matching SYNC-FROM-README block.
    ReadmeSyncMissingBlock {
        block: String,
    },
    /// index.mdx expects a README.md ANCHOR block that doesn't exist there.
    ReadmeSyncMissingAnchor {
        block: String,
    },
    /// One of the two files the README<->index.mdx sync check compares (README.md or
    /// docs/src/index.mdx) could not be read at all — e.g. moved or renamed. Without this,
    /// a missing source file would silently disable the whole check instead of failing loud.
    ReadmeSyncSourceMissing {
        path: String,
    },
    /// docs/src/reference/stability.md's table content differs from README's `tier_table`
    /// ANCHOR block, after known intentional link-scheme and heading-style transforms are
    /// applied. A dedicated variant (not a reuse of `ReadmeSyncDrift`) because this check
    /// compares README against a different file (stability.md, not index.mdx) with its own
    /// remediation text — reusing the index.mdx-flavored messages in `workflow_for_kind`
    /// would misdirect a fix-agent at the wrong file.
    ReadmeStabilitySyncDrift {
        block: String,
    },
    /// README.md has the `tier_table` ANCHOR block but docs/src/reference/stability.md has
    /// no recognizable table content (its `Vox is marching toward...` intro marker is
    /// missing or the file is otherwise unrecognizable).
    ReadmeStabilitySyncMissingBlock {
        block: String,
    },
    /// The README<->stability.md sync check expects a `tier_table` ANCHOR block in
    /// README.md but it's missing there.
    ReadmeStabilitySyncMissingAnchor {
        block: String,
    },
    /// One of the two files the README<->stability.md sync check compares (README.md or
    /// docs/src/reference/stability.md) could not be read at all.
    ReadmeStabilitySyncSourceMissing {
        path: String,
    },
}
