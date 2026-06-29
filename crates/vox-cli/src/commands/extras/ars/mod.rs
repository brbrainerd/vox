//! `vox skill` — manage Vox skills from the CLI.

mod context;
mod discover;
mod eval_promote;
mod registry;
mod run_skill;
mod skills_crud;

#[cfg(test)]
mod tests;

pub use context::{context_assemble, context_assemble_bundle};
pub use discover::discover;
pub use eval_promote::{eval_task, promote_skill};
pub use run_skill::run;
pub use skills_crud::{add, create, info, install, list, search, uninstall};
