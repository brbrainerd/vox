//! Mobile primitive emit: `@back_button`, `@deep_link`, `@push` → `mobile.ts`.
//!
//! Targets **Tauri 2** shell integration: the Rust wrapper emits `vox-back-button` / `vox-deep-link`
//! events that mirror hardware-back / URL-open semantics.
//!
//! **Capability SSOT:** Android/iOS permission hints for broader `@uses(...)` capabilities are defined in
//! `contracts/capability/runtime-capabilities.v1.yaml` and projected to JSON as
//! `runtime-capabilities.projection.json` next to Tauri packaging hints when **`vox compile`** runs
//! `vox_tauri_codegen::emit_tauri_packaging_hints` with the repository contracts root discovered from the manifest path.

use vox_compiler::shell_projection::ShellProjectionModule;

/// Emit a `mobile.ts` file containing mobile setup helpers for any mobile
/// primitives declared in the module. Returns `None` when no mobile
/// primitives are present so the caller can skip creating the file.
pub fn emit_mobile_setup(shell: &ShellProjectionModule) -> Option<String> {
    let has_back = shell.back_button.is_some();
    let has_deep = shell.deep_link.is_some();
    let has_push = shell.push.is_some();

    if !has_back && !has_deep && !has_push {
        return None;
    }

    let mut imports: Vec<&'static str> = vec!["import * as endpoints from './vox-client';"];
    if has_back || has_deep || has_push {
        imports.push("import { listen } from '@tauri-apps/api/event';");
    }
    if has_back {
        imports.push("import { invoke } from '@tauri-apps/api/core';");
    }
    if has_deep {
        imports.push("import { useEffect } from 'react';");
        imports.push("import { useNavigate } from '@tanstack/react-router';");
    }

    let header = concat!(
        "// Capability-driven Android/iOS manifest hints share SSOT:\n",
        "// contracts/capability/runtime-capabilities.v1.yaml → runtime-capabilities.projection.json\n",
        "// (emitted beside Tauri packaging hints on `vox compile` when contracts root is found).\n",
        "//\n",
    );
    let mut parts: Vec<String> = vec![format!("{header}{}", imports.join("\n"))];

    if let Some(back) = &shell.back_button {
        let on_press = &back.on_press;
        if !on_press.is_empty() {
            let fallback_call = back
                .fallback
                .as_ref()
                .map(|f| format!("await endpoints.{f}();"))
                .unwrap_or_else(|| "await invoke('plugin:process|exit');".into());
            parts.push(format!(
                "let __backHandlerRegistered = false;
export function installBackButtonHandler() {{
  if (__backHandlerRegistered) return;
  __backHandlerRegistered = true;
  void listen('vox-back-button', async () => {{
    const handled = await endpoints.{on_press}();
    if (!handled) {{
      {fallback_call}
    }}
  }});
}}"
            ));
        }
    }

    if let Some(dl) = &shell.deep_link {
        let on_link = &dl.on_link;
        parts.push(format!(
            "export function useDeepLinkRouting() {{
  const navigate = useNavigate();
  useEffect(() => {{
    let unlisten: (() => void) | undefined;
    void listen<string>('vox-deep-link', async (event) => {{
      const target = await endpoints.{on_link}(event.payload);
      navigate({{ to: target }});
    }}).then((fn) => {{ unlisten = fn; }});
    return () => {{ unlisten?.(); }};
  }}, [navigate]);
}}"
        ));
    }

    if let Some(push) = &shell.push {
        let on_reg = push.on_register.as_deref().unwrap_or("");
        let on_notif = push.on_notification.as_deref().unwrap_or("");
        let on_action = push.on_action.as_deref().unwrap_or("");

        let reg_listener = if on_reg.is_empty() {
            String::new()
        } else {
            format!(
                "  void listen<string>('vox-push-registration', async (e) => {{ await endpoints.{on_reg}(e.payload); }});\n"
            )
        };
        let notif_listener = if on_notif.is_empty() {
            String::new()
        } else {
            format!(
                "  void listen<string>('vox-push-notification', async (e) => {{ await endpoints.{on_notif}(e.payload); }});\n"
            )
        };
        let action_listener = if on_action.is_empty() {
            String::new()
        } else {
            format!(
                "  void listen<string>('vox-push-action', async (e) => {{ await endpoints.{on_action}(e.payload); }});\n"
            )
        };
        parts.push(format!(
            "export async function installPushNotifications() {{
{reg_listener}{notif_listener}{action_listener}}}"
        ));
    }

    Some(parts.join("\n\n"))
}
