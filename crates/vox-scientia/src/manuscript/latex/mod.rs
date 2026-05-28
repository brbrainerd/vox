//! SCIENTIA Phase 3+4 — markdown→LaTeX renderer for SCIENTIA manuscripts.
//!
//! Consumes [`crate::manuscript::scaffold::ScaffoldInput`] and produces a
//! standalone `.tex` document using the `article` document class. The
//! output is suitable for:
//!
//! - **Local PDF generation** via `tectonic` or `pdflatex`
//!   (shell-out lives outside this crate — see
//!   `vox scientia publication-render-latex` for the CLI surface).
//! - **arXiv staging bundles** — pair the `.tex` with the manifest's
//!   figure assets and tar.gz it; the existing
//!   `vox-publisher::submission::arxiv` machinery wraps it for upload.
//!
//! The renderer is pure: no shell-out, no external binaries, no rendering
//! engine. It uses `pulldown-cmark` to parse markdown bodies and emits a
//! deterministic LaTeX string. Special characters in author / title /
//! body text are escaped via a single canonical pass so the output
//! compiles under TeX Live without surprises.

pub mod bundle;
pub mod escape;
pub mod render;

pub use bundle::{list_bundle_entries, render_arxiv_bundle, BundleError};
pub use escape::escape_latex;
pub use render::render_latex;
