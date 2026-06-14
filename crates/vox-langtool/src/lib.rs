//! `vox-langtool` — DB-free CLI for working with the Vox language.

pub mod commands;

#[cfg(test)]
mod tests;

/// Heuristic: treat file as script-like (uses `parse_script`) unless it
/// contains app-surface decorators that must live at module position.
pub fn is_script_like(source: &str) -> bool {
    let app_markers = [
        "@page",
        "@query",
        "@mutation",
        "@server",
        "@component",
        "@table",
        "@workflow",
    ];
    !app_markers.iter().any(|m| source.contains(m))
}
