//! SCIENTIA Phase C — IMRaD manuscript scaffolder.
//!
//! Given a [`ScaffoldInput`] derived from a `FindingCandidate` + its verified
//! atomic claims + its RO-Crate metadata, this crate emits a long-form
//! markdown manuscript skeleton with **only provenance-bound safe slots
//! filled**. Sections forbidden by the
//! [worthiness rubric](../../../docs/src/reference/scientia-publication-worthiness-rules.md)
//! (`Introduction`, `Discussion`, `Significance`) are emitted as explicit
//! `<!-- TODO(narrative): -->` blocks listing the cited facts the human
//! should compose around — never auto-filled with prose.
//!
//! The crate is pure: it accepts a typed input and returns a string. No DB
//! queries, no LLM calls, no network I/O. Callers stage the inputs from
//! `vox-db` and `vox-scientia` separately.

pub mod render;
pub mod safe_slots;
pub mod section_tree;

pub use render::render_imrad;
pub use safe_slots::{ForbiddenSection, is_section_forbidden};
pub use section_tree::{
    AuthorEntry, CitedFact, FigureEntry, ResultsRow, ScaffoldError, ScaffoldInput, SectionTree,
};
