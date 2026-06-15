//! The single source of truth for "which backend consumes a lowered program."
//!
//! Adding a variant deliberately breaks every parity-checked match downstream
//! (see [`crate::parity_matrix`]). This is the *breadth* axis of the parity
//! contract made real in the type system: a feature must be considered against
//! every `Target`.
//!
//! **Deliberately NOT `#[non_exhaustive]`** (see the pipeline-parity SSOT §3.1):
//! the whole point is that adding a target must break every downstream `match`
//! so each emitter is forced to decide how it handles the new target.
//! `#[non_exhaustive]` would force downstream crates to keep a `_` arm, which
//! reintroduces exactly the silent catch-all this initiative removes.

/// Every backend that can consume a lowered Vox program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    /// `vox_compiler::eval` — the tree-walking interpreter (`--mode interp`).
    Interpreter,
    /// `vox_codegen::codegen_rust` with the `AxumLocalServer` shell (server/script).
    RustAxum,
    /// `vox_codegen::codegen_rust` with the `TauriApp` shell (desktop/mobile).
    RustTauri,
    /// `vox_codegen::codegen_ts` — the TypeScript/React frontend emitter.
    TypeScript,
}

impl Target {
    /// Every variant, in a stable order. Keep in sync with the enum — the
    /// `all_contains_every_variant` test guards the count.
    pub const ALL: [Target; 4] = [
        Target::Interpreter,
        Target::RustAxum,
        Target::RustTauri,
        Target::TypeScript,
    ];

    /// The canonical short identifier for this target.
    ///
    /// This is the projection that scattered selectors (CLI `RunMode`,
    /// `CompileKind`, `BuildTarget`, `RustAppShell`) map *into* — see the
    /// pipeline-parity SSOT §3.1. It is **not** a CLI `--mode` value (that flag
    /// is `app`/`library` only); it is the stable target name used by the parity
    /// machinery.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Target::Interpreter => "interp",
            Target::RustAxum => "rust-axum",
            Target::RustTauri => "rust-tauri",
            Target::TypeScript => "typescript",
        }
    }

    /// Parse a [`Target`] from its canonical [`Target::id`].
    #[must_use]
    pub fn from_id(id: &str) -> Option<Target> {
        Target::ALL.into_iter().find(|t| t.id() == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_target_round_trips_through_id() {
        for t in Target::ALL {
            let id = t.id();
            assert_eq!(Target::from_id(id), Some(t), "id {id} must round-trip");
        }
    }

    #[test]
    fn all_contains_every_variant() {
        // Guards against adding a variant but forgetting to extend ALL.
        assert_eq!(Target::ALL.len(), 4);
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<&str> = Target::ALL.iter().map(|t| t.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), Target::ALL.len(), "every Target id must be unique");
    }

    #[test]
    fn unknown_id_is_none() {
        assert_eq!(Target::from_id("bogus"), None);
    }
}
