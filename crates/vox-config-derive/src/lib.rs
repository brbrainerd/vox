//! `#[derive(VoxConfig)]` — generates `merge_env`/`get`/`catalog`/`config_keys`
//! for a domain config struct so a knob is declared once (an annotated field)
//! and read through `vox_config::env_parse::resolve_config_*` (env → config.toml
//! → default). Credentials are NOT config: a `#[config(secret)]` field is a
//! compile error directing it to `vox-secrets`.

use proc_macro::TokenStream;

mod model;

#[proc_macro_derive(VoxConfig, attributes(vox_config, config))]
pub fn derive_vox_config(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(input as syn::DeriveInput);
    match model::ConfigModel::from_ast(&ast).and_then(|m| m.codegen()) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
