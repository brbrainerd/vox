#[inline]
fn env_var_explicitly_disabled(res: Result<String, std::env::VarError>) -> bool {
    match res {
        Ok(v) => {
            v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("no")
                || v.eq_ignore_ascii_case("off")
        }
        Err(_) => false,
    }
}

/// Web IR lower + validate runs at the end of `vox_codegen::codegen_ts::generate` unless disabled.
///
/// **Default:** validation is **on** (unset). Set `VOX_WEBIR_VALIDATE=0`, `false`, `no`, or `off` to skip.
#[must_use]
pub(crate) fn web_ir_validate_gate_enabled() -> bool {
    !env_var_explicitly_disabled(std::env::var("VOX_WEBIR_VALIDATE"))
}

/// When set (`1`, `true`, `yes`), TypeScript codegen fails if the module contains AI fixtures that are not lowered to TS yet.
#[must_use]
pub(crate) fn ts_strict_ai_gate_enabled() -> bool {
    matches!(
        std::env::var("VOX_TS_STRICT_AI")
            .ok()
            .as_deref()
            .map(str::trim),
        Some("1") | Some("true") | Some("yes")
    )
}

/// When set (`1`/`true`), the web target skips emitting the bootstrap
/// (`entry.tsx` / router / `runtime-install.ts`) for external-React consumers
/// that own their own root. Centralized here (not read ad hoc in consumer code)
/// alongside the other web-migration flags. Re-exported for the CLI via
/// [`crate::codegen_ts::emitter::no_emit_entry_from_env`].
#[must_use]
pub(crate) fn no_emit_entry_gate_enabled() -> bool {
    matches!(
        std::env::var("VOX_WEB_NO_EMIT_ENTRY")
            .ok()
            .as_deref()
            .map(str::trim),
        Some("1") | Some("true") | Some("yes")
    )
}
