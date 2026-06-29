# Config Standardization SP-A: `#[derive(VoxConfig)]` Foundation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `#[derive(VoxConfig)]` proc-macro + a `vox-cli` registry aggregator + a standardization lint, and prove them by re-deriving the orchestrator's non-secret config fields.

**Architecture:** A new proc-macro crate `vox-config-derive` generates, from one annotated struct, a `merge_env()` (reading via `vox_config::env_parse::resolve_config_*`), a `get()` `OnceLock` snapshot, a `catalog()`, and a `config_keys()` `&[ConfigKey]`. A `vox-cli` aggregator concatenates every domain's `config_keys()` for the `config-registry-parity` gate. A `config-hygiene` lint flags new non-credential `resolve_secret` point-of-use reads.

**Tech Stack:** Rust, `syn` 2 / `quote` / `proc-macro2`, `vox-config` (`env_parse`, `config_key`), `trybuild` for compile-fail tests.

**Spec:** `docs/superpowers/specs/2026-06-29-config-standardization-vox-config-derive-design.md`

**Deviation from spec (deliberate):** the spec's SP-A says "re-derive the orchestrator config." The orchestrator's `merge_env_overrides` mixes migrated `cfg_opt` reads with ~17 still-secret `secrets_opt(SecretId::...)` reads (multi-consumer credentials). This plan re-derives **only the non-secret (`cfg_opt`) fields** and keeps the secret reads on a small retained `merge_secret_overrides()`. This preserves the spec's intent (orchestrator validates the macro on real fields) without forcing credentials into the config struct (which the spec's own credential boundary forbids).

**House rules (apply throughout):** never `cargo fmt --all` (use `cargo fmt -p <crate>`); exclude `vox-gui` from `clippy --all-targets`; on Windows, `Stop-Process -Name vox -Force` before a `cargo` run if a prior run left `target\debug\vox.exe` locked. Run `cargo` for one crate at a time.

---

## File Structure

| File | Responsibility |
|------|----------------|
| `crates/vox-config-derive/Cargo.toml` | proc-macro crate manifest (`proc-macro = true`; deps syn/quote/proc-macro2) |
| `crates/vox-config-derive/src/lib.rs` | `#[proc_macro_derive(VoxConfig, attributes(vox_config, config))]` entry + codegen |
| `crates/vox-config-derive/src/model.rs` | parse the struct + attrs into a `ConfigModel` (fields, env names, defaults, kinds) |
| `crates/vox-config-derive/tests/expand.rs` | unit tests: a derived test struct's `merge_env`/`get`/`config_keys` behave |
| `crates/vox-config-derive/tests/compile_fail/secret_field.rs` + `.stderr` | trybuild: `#[config(secret)]` is a compile error |
| `crates/vox-config-derive/tests/trybuild.rs` | trybuild harness |
| `crates/vox-config/src/lib.rs` | re-export `VoxConfig` derive; add `ConfigField` catalog type + `VoxConfigDomain` trait |
| `crates/vox-config/src/config_field.rs` | the generic `ConfigField` catalog row type |
| `crates/vox-cli/src/commands/ci/config_aggregate.rs` | collect all domains' `config_keys()` into one slice |
| `crates/vox-cli/src/commands/ci/config_hygiene.rs` | add the standardization lint check |
| `crates/vox-orchestrator/src/config/orchestrator_fields.rs` | add `#[derive(VoxConfig)]` + field attrs |
| `crates/vox-orchestrator/src/config/impl_env.rs` | drop the `cfg_opt` blocks; keep `merge_secret_overrides()` for the 17 secret reads |

---

## Task 1: Scaffold the `vox-config-derive` proc-macro crate

**Files:**
- Create: `crates/vox-config-derive/Cargo.toml`
- Create: `crates/vox-config-derive/src/lib.rs`
- Modify: root `Cargo.toml` (workspace `members`)

- [ ] **Step 1: Add the crate manifest**

Create `crates/vox-config-derive/Cargo.toml`:
```toml
[package]
name = "vox-config-derive"
version.workspace = true
edition.workspace = true

[lib]
proc-macro = true

[dependencies]
syn = { version = "2", features = ["full"] }
quote = "1"
proc-macro2 = "1"

[dev-dependencies]
trybuild = "1"
vox-config = { path = "../vox-config" }
```

- [ ] **Step 2: Minimal derive entry that compiles**

Create `crates/vox-config-derive/src/lib.rs`:
```rust
use proc_macro::TokenStream;

mod model;

/// Derive `merge_env`/`get`/`catalog`/`config_keys` for a domain config struct.
#[proc_macro_derive(VoxConfig, attributes(vox_config, config))]
pub fn derive_vox_config(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(input as syn::DeriveInput);
    match model::ConfigModel::from_ast(&ast).map(|m| m.codegen()) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
```

- [ ] **Step 3: Add to workspace members**

In root `Cargo.toml`, add `"crates/vox-config-derive"` to `[workspace] members` (keep the list alphabetically sorted with the other `vox-config*` entries).

- [ ] **Step 4: Stub `model.rs` so it compiles**

Create `crates/vox-config-derive/src/model.rs`:
```rust
use proc_macro2::TokenStream;

pub struct ConfigModel;

impl ConfigModel {
    pub fn from_ast(_ast: &syn::DeriveInput) -> syn::Result<Self> {
        Ok(ConfigModel)
    }
    pub fn codegen(&self) -> TokenStream {
        TokenStream::new()
    }
}
```

- [ ] **Step 5: Build**

Run: `cargo build -p vox-config-derive`
Expected: compiles (an empty derive that generates nothing).

- [ ] **Step 6: Commit**
```bash
git add crates/vox-config-derive Cargo.toml
git commit -m "feat(config-derive): scaffold vox-config-derive proc-macro crate"
```

---

## Task 2: The `ConfigField` catalog type + `VoxConfigDomain` trait in vox-config

**Files:**
- Create: `crates/vox-config/src/config_field.rs`
- Modify: `crates/vox-config/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-config/src/config_field.rs`:
```rust
//! Catalog row for a single config knob — the GUI/introspection view of a
//! `ConfigKey` plus its *current* value. Produced by `#[derive(VoxConfig)]`.

use crate::config_key::ConfigKind;

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigField {
    pub key: &'static str,
    pub kind: ConfigKind,
    pub current: String,
    pub default: String,
    pub group: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_field_roundtrips() {
        let f = ConfigField {
            key: "VOX_X_FOO",
            kind: ConfigKind::Int,
            current: "5".into(),
            default: "3".into(),
            group: "General",
            label: "Foo",
            hint: "",
        };
        assert_eq!(f.current, "5");
        assert_ne!(f.current, f.default);
    }
}
```

- [ ] **Step 2: Wire the module + the domain trait**

In `crates/vox-config/src/lib.rs` add:
```rust
pub mod config_field;
pub use config_field::ConfigField;
pub use vox_config_derive::VoxConfig;

/// Implemented by every `#[derive(VoxConfig)]` struct. The aggregator collects
/// `config_keys()` across domains; `catalog()` feeds the GUI.
pub trait VoxConfigDomain: Sized {
    fn merge_env(&mut self);
    fn config_keys() -> &'static [crate::config_key::ConfigKey];
    fn catalog(&self) -> Vec<ConfigField>;
}
```
Add to `crates/vox-config/Cargo.toml` `[dependencies]`: `vox-config-derive = { path = "../vox-config-derive" }`.

- [ ] **Step 3: Run the test**

Run: `cargo test -p vox-config config_field`
Expected: PASS (1 test). (The `vox_config_derive::VoxConfig` re-export resolves to the empty derive from Task 1.)

- [ ] **Step 4: Commit**
```bash
git add crates/vox-config/src/config_field.rs crates/vox-config/src/lib.rs crates/vox-config/Cargo.toml
git commit -m "feat(config): add ConfigField catalog type + VoxConfigDomain trait"
```

---

## Task 3: Parse the struct + attributes into `ConfigModel`

**Files:**
- Modify: `crates/vox-config-derive/src/model.rs`

- [ ] **Step 1: Define the parsed model + field kind**

Replace `crates/vox-config-derive/src/model.rs` with:
```rust
use proc_macro2::TokenStream;
use quote::format_ident;
use syn::{Data, DeriveInput, Fields, LitStr, Type};

#[derive(Clone, Copy, PartialEq)]
pub enum Kind { Bool, Int, Uint, Float, Str, Parse } // Parse = FromStr (enums, etc.)

pub struct Field {
    pub ident: syn::Ident,
    pub ty: Type,
    pub kind: Kind,
    pub is_option: bool,
    pub env: String,
    pub default: String, // rendered default for ConfigKey/catalog
    pub label: String,
    pub hint: String,
    pub secret: bool,
}

pub struct ConfigModel {
    pub struct_ident: syn::Ident,
    pub prefix: String,
    pub group: String,
    pub fields: Vec<Field>,
}

fn screaming(ident: &syn::Ident) -> String {
    ident.to_string().to_uppercase()
}

fn classify(ty: &Type) -> (Kind, bool) {
    // returns (kind, is_option)
    if let Type::Path(p) = ty {
        let seg = p.path.segments.last().unwrap();
        let name = seg.ident.to_string();
        if name == "Option" {
            if let syn::PathArguments::AngleBracketed(a) = &seg.arguments {
                if let Some(syn::GenericArgument::Type(inner)) = a.args.first() {
                    let (k, _) = classify(inner);
                    return (k, true);
                }
            }
        }
        let k = match name.as_str() {
            "bool" => Kind::Bool,
            "i8" | "i16" | "i32" | "i64" | "isize" => Kind::Int,
            "u8" | "u16" | "u32" | "u64" | "usize" => Kind::Uint,
            "f32" | "f64" => Kind::Float,
            "String" => Kind::Str,
            _ => Kind::Parse,
        };
        return (k, false);
    }
    (Kind::Parse, false)
}

impl ConfigModel {
    pub fn from_ast(ast: &DeriveInput) -> syn::Result<Self> {
        let struct_ident = ast.ident.clone();
        let mut prefix = String::new();
        let mut group = "General".to_string();
        for attr in &ast.attrs {
            if attr.path().is_ident("vox_config") {
                attr.parse_nested_meta(|m| {
                    if m.path.is_ident("prefix") {
                        prefix = m.value()?.parse::<LitStr>()?.value();
                    } else if m.path.is_ident("group") {
                        group = m.value()?.parse::<LitStr>()?.value();
                    }
                    Ok(())
                })?;
            }
        }
        let data = match &ast.data {
            Data::Struct(s) => s,
            _ => return Err(syn::Error::new_spanned(ast, "VoxConfig requires a struct")),
        };
        let named = match &data.fields {
            Fields::Named(n) => n,
            _ => return Err(syn::Error::new_spanned(ast, "VoxConfig requires named fields")),
        };
        let mut fields = Vec::new();
        for f in &named.named {
            let ident = f.ident.clone().unwrap();
            let (kind, is_option) = classify(&f.ty);
            let mut env = format!("{}_{}", prefix, screaming(&ident));
            let mut default = String::new();
            let mut label = ident.to_string();
            let mut hint = String::new();
            let mut secret = false;
            for attr in &f.attrs {
                if attr.path().is_ident("config") {
                    attr.parse_nested_meta(|m| {
                        if m.path.is_ident("env") {
                            env = m.value()?.parse::<LitStr>()?.value();
                        } else if m.path.is_ident("default") {
                            // accept string or literal; render to string
                            let lit: syn::Lit = m.value()?.parse()?;
                            default = render_lit(&lit);
                        } else if m.path.is_ident("label") {
                            label = m.value()?.parse::<LitStr>()?.value();
                        } else if m.path.is_ident("hint") {
                            hint = m.value()?.parse::<LitStr>()?.value();
                        } else if m.path.is_ident("secret") {
                            secret = true;
                        }
                        Ok(())
                    })?;
                }
            }
            fields.push(Field { ident, ty: f.ty.clone(), kind, is_option, env, default, label, hint, secret });
        }
        Ok(ConfigModel { struct_ident, prefix, group, fields })
    }

    pub fn codegen(&self) -> TokenStream {
        let _ = format_ident!("placeholder"); // replaced in Task 4
        TokenStream::new()
    }
}

fn render_lit(lit: &syn::Lit) -> String {
    match lit {
        syn::Lit::Str(s) => s.value(),
        syn::Lit::Int(i) => i.base10_digits().to_string(),
        syn::Lit::Float(f) => f.base10_digits().to_string(),
        syn::Lit::Bool(b) => b.value().to_string(),
        _ => String::new(),
    }
}
```

- [ ] **Step 2: Build**

Run: `cargo build -p vox-config-derive`
Expected: compiles. (Parsing only; codegen still empty — verified end-to-end in Task 4's tests.)

- [ ] **Step 3: Commit**
```bash
git add crates/vox-config-derive/src/model.rs
git commit -m "feat(config-derive): parse struct + #[config(...)] attrs into ConfigModel"
```

---

## Task 4: Generate `merge_env`, `get`, `config_keys`, `catalog` + the secret-field error

**Files:**
- Modify: `crates/vox-config-derive/src/model.rs` (`codegen`)
- Create: `crates/vox-config-derive/tests/expand.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-config-derive/tests/expand.rs`:
```rust
use vox_config::VoxConfigDomain;

#[derive(Default, Clone, vox_config::VoxConfig)]
#[vox_config(prefix = "VOX_TESTDOMAIN", group = "General")]
struct TestDomain {
    #[config(default = 3, label = "Max things")]
    max_things: u32,
    #[config(default = false)]
    verbose: bool,
    #[config(env = "VOX_LEGACY_NAME", default = "info")]
    log_level: String,
}

#[test]
fn merge_env_reads_env() {
    // ponytail: set_var in a single-threaded test is fine; std deprecation is nightly-only here.
    unsafe { std::env::set_var("VOX_TESTDOMAIN_MAX_THINGS", "9"); }
    let mut c = TestDomain::default();
    c.merge_env();
    assert_eq!(c.max_things, 9);
    unsafe { std::env::remove_var("VOX_TESTDOMAIN_MAX_THINGS"); }
}

#[test]
fn config_keys_cover_fields_with_env_names() {
    let keys = TestDomain::config_keys();
    assert_eq!(keys.len(), 3);
    let names: Vec<_> = keys.iter().map(|k| k.key).collect();
    assert!(names.contains(&"VOX_TESTDOMAIN_MAX_THINGS"));
    assert!(names.contains(&"VOX_LEGACY_NAME")); // explicit env override honored
    assert!(keys.iter().all(|k| !k.secret));
}

#[test]
fn catalog_reports_current_and_default() {
    let c = TestDomain { max_things: 7, verbose: true, log_level: "debug".into() };
    let cat = c.catalog();
    let mt = cat.iter().find(|f| f.key == "VOX_TESTDOMAIN_MAX_THINGS").unwrap();
    assert_eq!(mt.current, "7");
    assert_eq!(mt.default, "3");
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p vox-config-derive --test expand`
Expected: FAIL — `merge_env`/`config_keys`/`catalog` not generated (trait not satisfied / methods missing).

- [ ] **Step 3: Implement `codegen`**

Replace `ConfigModel::codegen` in `model.rs` with:
```rust
    pub fn codegen(&self) -> TokenStream {
        // Secret fields are a compile error — credentials belong in vox-secrets.
        if let Some(f) = self.fields.iter().find(|f| f.secret) {
            return syn::Error::new_spanned(
                &f.ident,
                "VoxConfig fields cannot be secrets; put credentials in vox-secrets (SecretId) and read via resolve_secret",
            ).to_compile_error();
        }
        let ty = &self.struct_ident;
        let group_lit = &self.group;

        let merges = self.fields.iter().map(|f| {
            let id = &f.ident;
            let env = &f.env;
            let resolver = resolver_call(f, quote::quote!(self.#id.clone()));
            quote::quote! { self.#id = #resolver; let _ = #env; }
        });

        let keys = self.fields.iter().map(|f| {
            let env = &f.env;
            let kind = kind_tokens(f.kind);
            let default = &f.default;
            let label = &f.label;
            let hint = &f.hint;
            quote::quote! {
                ::vox_config::config_key::ConfigKey {
                    key: #env,
                    kind: #kind,
                    default: ::vox_config::config_key::DefaultValue::Literal(#default),
                    bound: None,
                    group: ::vox_config::config_key::Group::General,
                    class: ::vox_config::operator_registry::ConfigClass::NodeLocal,
                    home: ::vox_config::config_key::Home::Env,
                    gui: None,
                    secret: false,
                    status: ::vox_config::config_key::Status::Active,
                    label: #label,
                    hint: #hint,
                }
            }
        });

        let cat = self.fields.iter().map(|f| {
            let id = &f.ident;
            let env = &f.env;
            let kind = kind_tokens(f.kind);
            let default = &f.default;
            let label = &f.label;
            let hint = &f.hint;
            quote::quote! {
                ::vox_config::ConfigField {
                    key: #env,
                    kind: #kind,
                    current: format!("{}", self.#id),
                    default: #default.to_string(),
                    group: #group_lit,
                    label: #label,
                    hint: #hint,
                }
            }
        });

        quote::quote! {
            impl ::vox_config::VoxConfigDomain for #ty {
                fn merge_env(&mut self) { #(#merges)* }
                fn config_keys() -> &'static [::vox_config::config_key::ConfigKey] {
                    &[ #(#keys),* ]
                }
                fn catalog(&self) -> ::std::vec::Vec<::vox_config::ConfigField> {
                    vec![ #(#cat),* ]
                }
            }
            impl #ty {
                /// Default + env/toml overlay, uncached (test/hot-reload path).
                pub fn from_env_uncached() -> Self where Self: Default {
                    let mut c = Self::default();
                    <Self as ::vox_config::VoxConfigDomain>::merge_env(&mut c);
                    c
                }
                /// Process-lifetime snapshot. ponytail: OnceLock — env read once; tests needing
                /// re-read use `from_env_uncached`.
                pub fn get() -> &'static Self where Self: Default {
                    static CELL: ::std::sync::OnceLock<#ty> = ::std::sync::OnceLock::new();
                    CELL.get_or_init(Self::from_env_uncached)
                }
            }
        }
    }
```

Add these helpers at the bottom of `model.rs`:
```rust
fn kind_tokens(k: Kind) -> TokenStream {
    match k {
        Kind::Bool => quote::quote!(::vox_config::config_key::ConfigKind::Bool),
        Kind::Int | Kind::Uint => quote::quote!(::vox_config::config_key::ConfigKind::Int),
        Kind::Float => quote::quote!(::vox_config::config_key::ConfigKind::Float),
        Kind::Str | Kind::Parse => quote::quote!(::vox_config::config_key::ConfigKind::String),
    }
}

// Build the resolve_config_* call. `cur` is a TokenStream for the current value
// (used as the default arg so precedence is env -> toml -> current/struct-default).
fn resolver_call(f: &Field, cur: TokenStream) -> TokenStream {
    let env = &f.env;
    if f.is_option {
        // Option<T>: read string; Some(parsed) if env/toml present else keep current.
        return quote::quote! {{
            match ::vox_config::env_parse::resolve_config_opt_str(#env) {
                Some(s) => s.parse().ok(),
                None => #cur,
            }
        }};
    }
    match f.kind {
        Kind::Bool => quote::quote!(::vox_config::env_parse::resolve_config_bool(#env, #cur)),
        Kind::Uint => quote::quote!(::vox_config::env_parse::resolve_config_u64(#env, #cur as u64) as _),
        Kind::Int => quote::quote!(::vox_config::env_parse::resolve_config_i64(#env, #cur as i64) as _),
        Kind::Float => quote::quote!(::vox_config::env_parse::resolve_config_f32(#env, #cur as f32) as _),
        Kind::Str => quote::quote!(::vox_config::env_parse::resolve_config_str(#env, &#cur)),
        Kind::Parse => quote::quote! {{
            let s = ::vox_config::env_parse::resolve_config_str(#env, &format!("{}", #cur));
            s.parse().unwrap_or(#cur)
        }},
    }
}
```

- [ ] **Step 4: Add the missing `env_parse` helpers it calls**

The macro references `resolve_config_opt_str` and `resolve_config_i64`. Confirm/add them in `crates/vox-config/src/env_parse.rs`:
```rust
/// Resolve an optional string: env var → `~/.vox/config.toml` → `None`.
#[must_use]
pub fn resolve_config_opt_str(name: &str) -> Option<String> {
    if let Ok(v) = std::env::var(name) {
        let t = v.trim();
        if !t.is_empty() { return Some(t.to_string()); }
    }
    if let Some(v) = toml_config::load_user_config().values.get(name) {
        if let Some(s) = v.as_str() { return Some(s.to_string()); }
    }
    None
}

/// Resolve an i64 config value using layered precedence.
#[must_use]
pub fn resolve_config_i64(name: &str, default: i64) -> i64 {
    if let Ok(v) = std::env::var(name) && let Ok(p) = v.trim().parse::<i64>() { return p; }
    if let Some(v) = toml_config::load_user_config().values.get(name) {
        if let Some(i) = v.as_integer() { return i; }
        if let Some(s) = v.as_str() && let Ok(p) = s.trim().parse::<i64>() { return p; }
    }
    default
}
```
(Skip either if `grep` shows it already exists.)

- [ ] **Step 5: Run the tests**

Run: `cargo test -p vox-config-derive --test expand`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**
```bash
git add crates/vox-config-derive/src/model.rs crates/vox-config-derive/tests/expand.rs crates/vox-config/src/env_parse.rs
git commit -m "feat(config-derive): generate merge_env/get/config_keys/catalog + secret-field error"
```

---

## Task 5: Compile-fail test for `#[config(secret)]`

**Files:**
- Create: `crates/vox-config-derive/tests/trybuild.rs`
- Create: `crates/vox-config-derive/tests/compile_fail/secret_field.rs`
- Create: `crates/vox-config-derive/tests/compile_fail/secret_field.stderr`

- [ ] **Step 1: Write the trybuild harness**

Create `crates/vox-config-derive/tests/trybuild.rs`:
```rust
#[test]
fn compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
```

- [ ] **Step 2: Write the failing fixture**

Create `crates/vox-config-derive/tests/compile_fail/secret_field.rs`:
```rust
#[derive(Default, vox_config::VoxConfig)]
#[vox_config(prefix = "VOX_X", group = "General")]
struct Bad {
    #[config(secret, default = "")]
    api_key: String,
}
fn main() {}
```

- [ ] **Step 3: Run to capture stderr**

Run: `cargo test -p vox-config-derive --test trybuild`
Expected: FAIL the first time, printing a "wip" `.stderr`. Copy the emitted expected error (containing "fields cannot be secrets") into `tests/compile_fail/secret_field.stderr`.

- [ ] **Step 4: Re-run to verify it passes**

Run: `cargo test -p vox-config-derive --test trybuild`
Expected: PASS (the compile error matches the recorded `.stderr`).

- [ ] **Step 5: Commit**
```bash
git add crates/vox-config-derive/tests/trybuild.rs crates/vox-config-derive/tests/compile_fail
git commit -m "test(config-derive): compile-fail test for #[config(secret)]"
```

---

## Task 6: The `vox-cli` registry aggregator

**Files:**
- Create: `crates/vox-cli/src/commands/ci/config_aggregate.rs`
- Modify: `crates/vox-cli/src/commands/ci/mod.rs` (add `pub mod config_aggregate;`)

- [ ] **Step 1: Write the failing test**

Create `crates/vox-cli/src/commands/ci/config_aggregate.rs`:
```rust
//! Collects every `#[derive(VoxConfig)]` domain's `config_keys()` into one slice
//! so the config-registry-parity gate sees domain-owned knobs without manual rows.

use vox_config::config_key::ConfigKey;
use vox_config::VoxConfigDomain;

/// All domain config keys. Add one line per `#[derive(VoxConfig)]` struct.
/// ponytail: explicit list (not linkme) — layering-safe; the test below stops drift.
#[must_use]
pub fn all_domain_config_keys() -> Vec<ConfigKey> {
    let mut keys = Vec::new();
    keys.extend_from_slice(vox_orchestrator::config::OrchestratorConfig::config_keys());
    keys
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aggregate_is_nonempty_and_unique() {
        let keys = all_domain_config_keys();
        assert!(!keys.is_empty());
        let mut names: Vec<_> = keys.iter().map(|k| k.key).collect();
        let n = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), n, "duplicate config key across domains");
    }
}
```

- [ ] **Step 2: Wire the module + dep**

In `crates/vox-cli/src/commands/ci/mod.rs` add `pub mod config_aggregate;`. Ensure `crates/vox-cli/Cargo.toml` depends on `vox-orchestrator` and `vox-config` (it already depends on both — confirm with `grep`).

- [ ] **Step 3: Run the test**

Run: `cargo test -p vox-cli config_aggregate`
Expected: FAIL — `OrchestratorConfig::config_keys` does not exist yet (Task 9 adds the derive). This is the expected ordering; proceed to Step 4.

- [ ] **Step 4: Temporarily make it compile**

So later tasks can build, stub the orchestrator line with an empty seed until Task 9:
```rust
pub fn all_domain_config_keys() -> Vec<ConfigKey> {
    // ponytail: seeded empty until OrchestratorConfig derives VoxConfig (Task 9).
    Vec::new()
}
```
And relax the test to `assert!(keys.is_empty() || true)`-style is NOT allowed (no placeholders). Instead mark the real test `#[ignore]` with a reason until Task 9:
```rust
    #[test]
    #[ignore = "enabled in Task 9 once OrchestratorConfig derives VoxConfig"]
    fn aggregate_is_nonempty_and_unique() { /* body unchanged */ }
```

- [ ] **Step 5: Run**

Run: `cargo test -p vox-cli config_aggregate`
Expected: PASS (the live test is ignored; compiles).

- [ ] **Step 6: Commit**
```bash
git add crates/vox-cli/src/commands/ci/config_aggregate.rs crates/vox-cli/src/commands/ci/mod.rs
git commit -m "feat(ci): config_aggregate scaffold for domain config keys"
```

---

## Task 7: Wire the aggregate into the config-registry-parity gate

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/config_registry_parity.rs` (the parity check — confirm exact filename with `grep`)

- [ ] **Step 1: Locate the parity check's "known keys" set**

Run: `grep -rn "CONFIG_KEYS" crates/vox-cli/src/commands/ci/`
Identify where the gate builds its set of registered names from `vox_config::config_registry::CONFIG_KEYS`.

- [ ] **Step 2: Add the domain keys to that set**

At the point where the gate collects registered names, union in the aggregate:
```rust
let mut registered: std::collections::HashSet<&str> =
    vox_config::config_registry::CONFIG_KEYS.iter().map(|k| k.key).collect();
for k in crate::commands::ci::config_aggregate::all_domain_config_keys() {
    registered.insert(k.key);
}
```
(Adapt the variable name to the existing code; the point is: domain keys count as registered.)

- [ ] **Step 3: Build**

Run: `cargo build -p vox-cli`
Expected: compiles. (No behavior change yet — aggregate is empty until Task 9.)

- [ ] **Step 4: Commit**
```bash
git add crates/vox-cli/src/commands/ci/config_registry_parity.rs
git commit -m "feat(ci): config-registry-parity counts derived domain keys as registered"
```

---

## Task 8: The standardization lint (new non-credential `resolve_secret` reads)

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/config_hygiene.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `config_hygiene.rs`:
```rust
#[test]
fn flags_nonsecret_resolve_secret_point_of_use() {
    // A read of a clearly-config (non-credential) SecretId outside vox-secrets.
    let src = r#"let x = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSpeechMaxBiasPhrases).expose();"#;
    let hits = super::scan_nonsecret_resolve_secret(src, "crates/vox-speech/src/foo.rs");
    assert_eq!(hits.len(), 1);
}

#[test]
fn ignores_resolve_secret_inside_vox_secrets_crate() {
    let src = r#"resolve_secret(SecretId::VoxSpeechMaxBiasPhrases)"#;
    let hits = super::scan_nonsecret_resolve_secret(src, "crates/vox-secrets/src/lib.rs");
    assert!(hits.is_empty());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-cli config_hygiene::tests::flags_nonsecret`
Expected: FAIL — `scan_nonsecret_resolve_secret` undefined.

- [ ] **Step 3: Implement the scanner**

Add to `config_hygiene.rs`:
```rust
/// Heuristic lint: a `resolve_secret(SecretId::Vox<Domain>...)` read at point-of-use
/// outside the `vox-secrets` crate, where the id name does NOT look like a credential.
/// ponytail: regex/substring heuristic — credential-shaped names (KEY/TOKEN/SECRET/
/// PASSWORD/CREDENTIAL) are exempt; tighten only if a false positive shows up.
pub(crate) fn scan_nonsecret_resolve_secret(src: &str, path: &str) -> Vec<String> {
    if path.replace('\\', "/").contains("crates/vox-secrets/") {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for line in src.lines() {
        if let Some(idx) = line.find("SecretId::") {
            if !line.contains("resolve_secret") { continue; }
            let name = line[idx + "SecretId::".len()..]
                .chars().take_while(|c| c.is_alphanumeric()).collect::<String>();
            let upper = name.to_uppercase();
            let credentialish = ["KEY", "TOKEN", "SECRET", "PASSWORD", "PWD", "CREDENTIAL"]
                .iter().any(|m| upper.contains(m));
            if !credentialish {
                hits.push(name);
            }
        }
    }
    hits
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p vox-cli config_hygiene::tests`
Expected: PASS (both new tests).

- [ ] **Step 5: Wire into the gate as a warning-only baseline (no new violations)**

In the `config_hygiene` check body, after the existing checks, scan changed files and report `scan_nonsecret_resolve_secret` hits as a *new-violations-only* category (mirror the existing baseline-ratchet pattern in this file; do NOT fail on the grandfathered ~252 — only on additions). Find the existing baseline mechanism with `grep -n "baseline" crates/vox-cli/src/commands/ci/config_hygiene.rs` and follow it.

- [ ] **Step 6: Run the gate**

Run: `Stop-Process -Name vox -Force; cargo run -p vox-cli -- ci config-hygiene` (PowerShell)
Expected: exit 0 (green) — no NEW non-credential reads introduced by this branch.

- [ ] **Step 7: Commit**
```bash
git add crates/vox-cli/src/commands/ci/config_hygiene.rs
git commit -m "feat(ci): lint new non-credential resolve_secret point-of-use reads"
```

---

## Task 9: Re-derive the orchestrator's non-secret fields (proving ground)

**Files:**
- Modify: `crates/vox-orchestrator/src/config/orchestrator_fields.rs`
- Modify: `crates/vox-orchestrator/src/config/impl_env.rs`
- Modify: `crates/vox-config/src/config_registry.rs` (remove the 76 manual orchestrator rows)
- Modify: `crates/vox-cli/src/commands/ci/config_aggregate.rs` (un-ignore the test)

- [ ] **Step 1: Write the regression-pin test**

In `crates/vox-orchestrator/src/config/` add a test (e.g. in `orchestrator_fields.rs` `tests`):
```rust
#[test]
fn derived_config_keys_cover_migrated_env_names() {
    use vox_config::VoxConfigDomain;
    let keys = OrchestratorConfig::config_keys();
    let names: std::collections::HashSet<_> = keys.iter().map(|k| k.key).collect();
    // The cfg_opt-migrated knobs (sample the load-bearing ones).
    for expected in ["VOX_ORCHESTRATOR_MAX_AGENTS", "VOX_ORCHESTRATOR_LOCK_TIMEOUT_MS"] {
        assert!(names.contains(expected), "missing derived key {expected}");
    }
    assert!(keys.iter().all(|k| !k.secret));
}

#[test]
fn env_override_applies_via_derive() {
    use vox_config::VoxConfigDomain;
    unsafe { std::env::set_var("VOX_ORCHESTRATOR_MAX_AGENTS", "3"); }
    let mut c = OrchestratorConfig::default();
    c.merge_env();
    assert_eq!(c.max_agents, 3);
    unsafe { std::env::remove_var("VOX_ORCHESTRATOR_MAX_AGENTS"); }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-orchestrator derived_config_keys_cover`
Expected: FAIL — `OrchestratorConfig` doesn't derive `VoxConfig` yet.

- [ ] **Step 3: Add the derive + per-field attrs for the cfg_opt fields**

In `orchestrator_fields.rs`, add `vox_config::VoxConfig` to the struct's derive list and `#[vox_config(prefix = "VOX_ORCHESTRATOR", group = "Orchestrator")]`. For each field currently read via `cfg_opt(...)` in `impl_env.rs`, add `#[config(default = <its Default value>, env = "<exact VOX_ORCHESTRATOR_ name>")]` (use the explicit `env` to match the existing names exactly — derive-from-prefix may not match e.g. abbreviations). For fields NOT read from env (or read via `secrets_opt`), add `#[config(skip)]`.

Add `skip` handling to the macro: in `model.rs` `from_ast`, when parsing field attrs, recognize `m.path.is_ident("skip")` → set a `skip: bool` on `Field`; in `codegen`, filter `self.fields` to `!f.skip` for merges/keys/catalog. (One-line additions in three places.)

- [ ] **Step 4: Replace the cfg_opt blocks with the derived merge_env**

In `impl_env.rs`, rename the current method to `merge_secret_overrides` and **delete every `if let Some(val) = cfg_opt(...)` block** (keep only the `secrets_opt(...)` blocks for the ~17 multi-consumer secret reads). At the orchestrator's config-load call site, call both:
```rust
use vox_config::VoxConfigDomain;
config.merge_env();              // derived: non-secret cfg knobs (env -> toml -> default)
config.merge_secret_overrides(); // retained: the 17 SecretId-backed knobs
```
Find the load site with `grep -rn "merge_env_overrides" crates/`.

- [ ] **Step 5: Remove the 76 manual orchestrator rows from `CONFIG_KEYS`**

In `crates/vox-config/src/config_registry.rs`, delete the 76 `ConfigKey` rows added by commit `53f5a3967a` (the `Group::Orchestrator`, `VOX_ORCHESTRATOR_*` rows) — they are now produced by `OrchestratorConfig::config_keys()` via the aggregator. Identify them with `grep -n "VOX_ORCHESTRATOR_" crates/vox-config/src/config_registry.rs`.

- [ ] **Step 6: Re-enable the aggregator + its test**

In `config_aggregate.rs`, restore the real body:
```rust
pub fn all_domain_config_keys() -> Vec<ConfigKey> {
    let mut keys = Vec::new();
    keys.extend_from_slice(vox_orchestrator::config::OrchestratorConfig::config_keys());
    keys
}
```
and remove the `#[ignore]` from `aggregate_is_nonempty_and_unique`.

- [ ] **Step 7: Run the orchestrator + aggregator tests**

Run: `Stop-Process -Name vox -Force; cargo test -p vox-orchestrator config && cargo test -p vox-cli config_aggregate`
Expected: PASS — derived keys present, env override works, aggregate non-empty + unique.

- [ ] **Step 8: Run both config gates**

Run (PowerShell): `Stop-Process -Name vox -Force; cargo run -p vox-cli -- ci config-hygiene; cargo run -p vox-cli -- ci config-registry-parity`
Expected: both exit 0. (Parity: the 76 names are now covered by the aggregate instead of manual rows — no NEW unregistered, no phantom.)

- [ ] **Step 9: Commit**
```bash
git add crates/vox-orchestrator crates/vox-config/src/config_registry.rs crates/vox-cli/src/commands/ci/config_aggregate.rs crates/vox-config-derive/src/model.rs
git commit -m "refactor(orchestrator): derive non-secret config via #[derive(VoxConfig)]; retire 76 manual CONFIG_KEYS rows"
```

---

## Task 10: Full verification + clippy

- [ ] **Step 1: Test the touched crates**

Run: `Stop-Process -Name vox -Force; cargo test -p vox-config-derive -p vox-config -p vox-orchestrator -p vox-cli`
Expected: all PASS.

- [ ] **Step 2: Clippy the touched crates (not --all-targets workspace-wide)**

Run: `cargo clippy -p vox-config-derive -p vox-config -p vox-orchestrator -- -D warnings`
Expected: no warnings. Fix any inline.

- [ ] **Step 3: Confirm gates green together**

Run (PowerShell): `Stop-Process -Name vox -Force; cargo run -p vox-cli -- ci config-hygiene; cargo run -p vox-cli -- ci config-registry-parity`
Expected: both exit 0.

- [ ] **Step 4: Commit any clippy fixes**
```bash
git add -A
git commit -m "chore(config-standardization): clippy + final SP-A verification"
```

---

## Self-Review notes

- **Spec coverage:** §1 standard → Tasks 1–5; §2 credential boundary → Task 4 Step 3 + Task 5; §3 registry convergence → Tasks 6–7, 9; §4 point-of-use migration → Task 9 (orchestrator pilot); standardization lint (Testing strategy) → Task 8; oratio (§5) + other domains → **deferred to SP-B…E** (separate plans, by design).
- **Type consistency:** `VoxConfigDomain::{merge_env, config_keys, catalog}`, `ConfigField`, `all_domain_config_keys`, `scan_nonsecret_resolve_secret`, `from_env_uncached`, `get` used consistently across tasks.
- **Known ceilings (ponytail):** macro covers bool/int/uint/float/String/FromStr + `Option`; no nested `flatten` yet (add in the first domain that needs it — oratio's 6 sub-structs will force it in SP-B). The `get()` `OnceLock` is process-lifetime; `from_env_uncached()` is the test/hot-reload escape hatch. The lint is a substring heuristic with a credential-name allowlist.
