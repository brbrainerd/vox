//! Repository CI guard checks extracted from `vox-cli` (`vox ci *` implementation wedge).

pub mod ai_fixtures_coverage;
pub mod dep_sprawl;
pub mod docs_deprecated_command_guard;
pub mod frozen_crates;
pub mod gui_visual_review;
pub mod line_endings;
pub mod no_plugin_cdylib_as_compile_dep;
pub mod no_tauri_in_core;
pub mod nomenclature_guard;
pub mod openclaw_contract;
pub mod parse_check;
pub mod plugin_dep_boundary;
pub mod retirement_audit;
pub mod row_serde_lint;
pub mod runner_policy_check;
pub mod string_id_lint;
pub mod sync_ignore_files;
pub mod toestub_budget;
