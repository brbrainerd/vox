//! Parse a `#[derive(VoxConfig)]` struct + its `#[vox_config(...)]`/`#[config(...)]`
//! attrs into a `ConfigModel`, then generate the `VoxConfigDomain` impl + `get()`.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Lit, LitStr, Type};

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Bool,
    Int,  // signed
    Uint, // unsigned
    Float32,
    Float64,
    Str,
    Parse, // any other T: FromStr (enums, …)
}

pub struct Field {
    pub ident: syn::Ident,
    pub kind: Kind,
    pub is_option: bool,
    pub env: String,
    pub default: String,
    pub bound: Option<(f64, f64)>,
    pub label: String,
    pub hint: String,
    pub secret: bool,
    /// A field is a config knob only if it carries a `#[config(...)]` attr (opt-in)
    /// and is not `#[config(skip)]`. Un-annotated fields are ignored — safe for big structs.
    pub include: bool,
}

pub struct ConfigModel {
    pub struct_ident: syn::Ident,
    pub group: String,
    pub fields: Vec<Field>,
}

fn screaming(ident: &syn::Ident) -> String {
    ident.to_string().to_uppercase()
}

/// (kind, is_option). `Option<T>` unwraps one level; the inner kind drives parsing.
fn classify(ty: &Type) -> (Kind, bool) {
    if let Type::Path(p) = ty {
        let seg = p.path.segments.last().unwrap();
        let name = seg.ident.to_string();
        if name == "Option"
            && let syn::PathArguments::AngleBracketed(a) = &seg.arguments
            && let Some(syn::GenericArgument::Type(inner)) = a.args.first()
        {
            let (k, _) = classify(inner);
            return (k, true);
        }
        let k = match name.as_str() {
            "bool" => Kind::Bool,
            "i8" | "i16" | "i32" | "i64" | "isize" => Kind::Int,
            "u8" | "u16" | "u32" | "u64" | "usize" => Kind::Uint,
            "f32" => Kind::Float32,
            "f64" => Kind::Float64,
            "String" => Kind::Str,
            _ => Kind::Parse,
        };
        return (k, false);
    }
    (Kind::Parse, false)
}

fn render_lit(lit: &Lit) -> String {
    match lit {
        Lit::Str(s) => s.value(),
        Lit::Int(i) => i.base10_digits().to_string(),
        Lit::Float(f) => f.base10_digits().to_string(),
        Lit::Bool(b) => b.value().to_string(),
        _ => String::new(),
    }
}

fn parse_bound(s: &str) -> Option<(f64, f64)> {
    let s = s.replace("..=", "..");
    let (a, b) = s.split_once("..")?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
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
                    } else {
                        return Err(m.error("unknown vox_config key (expected prefix/group)"));
                    }
                    Ok(())
                })?;
            }
        }
        let Data::Struct(data) = &ast.data else {
            return Err(syn::Error::new_spanned(ast, "VoxConfig requires a struct"));
        };
        let Fields::Named(named) = &data.fields else {
            return Err(syn::Error::new_spanned(
                ast,
                "VoxConfig requires named fields",
            ));
        };

        let mut fields = Vec::new();
        for f in &named.named {
            let ident = f.ident.clone().unwrap();
            let (kind, is_option) = classify(&f.ty);
            let mut env = format!("{}_{}", prefix, screaming(&ident));
            let mut default = String::new();
            let mut bound = None;
            let mut label = ident.to_string();
            let mut hint = String::new();
            let mut secret = false;
            let mut skip = false;
            let mut has_config_attr = false;
            for attr in &f.attrs {
                if attr.path().is_ident("config") {
                    has_config_attr = true;
                    attr.parse_nested_meta(|m| {
                        if m.path.is_ident("env") {
                            env = m.value()?.parse::<LitStr>()?.value();
                        } else if m.path.is_ident("default") {
                            default = render_lit(&m.value()?.parse::<Lit>()?);
                        } else if m.path.is_ident("bound") {
                            bound = parse_bound(&m.value()?.parse::<LitStr>()?.value());
                        } else if m.path.is_ident("label") {
                            label = m.value()?.parse::<LitStr>()?.value();
                        } else if m.path.is_ident("hint") {
                            hint = m.value()?.parse::<LitStr>()?.value();
                        } else if m.path.is_ident("secret") {
                            secret = true;
                        } else if m.path.is_ident("skip") {
                            skip = true;
                        } else {
                            return Err(m.error("unknown config key"));
                        }
                        Ok(())
                    })?;
                }
            }
            fields.push(Field {
                ident,
                kind,
                is_option,
                env,
                default,
                bound,
                label,
                hint,
                secret,
                include: has_config_attr && !skip,
            });
        }
        Ok(ConfigModel {
            struct_ident,
            group,
            fields,
        })
    }

    pub fn codegen(&self) -> syn::Result<TokenStream> {
        // Credentials are not config — a secret field is a hard error (credential boundary).
        if let Some(f) = self.fields.iter().find(|f| f.secret) {
            return Err(syn::Error::new(
                f.ident.span(),
                "VoxConfig fields cannot be secrets; put credentials in vox-secrets (SecretId) and read via resolve_secret",
            ));
        }
        let ty = &self.struct_ident;
        let group_tok = group_token(&self.group)?;
        let group_lit = &self.group;
        let active: Vec<&Field> = self.fields.iter().filter(|f| f.include).collect();

        let merges = active.iter().map(|f| {
            let id = &f.ident;
            let expr = resolver_call(f);
            quote! { self.#id = #expr; }
        });

        let keys = active.iter().map(|f| {
            let env = &f.env;
            let kind = kind_tokens(f.kind);
            let default = &f.default;
            let label = &f.label;
            let hint = &f.hint;
            let bound = match f.bound {
                Some((lo, hi)) => quote!(Some((#lo, #hi))),
                None => quote!(None),
            };
            quote! {
                ::vox_config::config_key::ConfigKey {
                    key: #env,
                    kind: #kind,
                    default: ::vox_config::config_key::DefaultValue::Literal(#default),
                    bound: #bound,
                    group: #group_tok,
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

        let cat = active.iter().map(|f| {
            let id = &f.ident;
            let env = &f.env;
            let kind = kind_tokens(f.kind);
            let default = &f.default;
            let label = &f.label;
            let hint = &f.hint;
            // Plain `String` fields render without Debug quotes (clean for the GUI);
            // everything else uses {:?} (Debug — universal across enums/Option/numeric).
            let current = if f.kind == Kind::Str && !f.is_option {
                quote!(self.#id.clone())
            } else {
                quote!(format!("{:?}", self.#id))
            };
            quote! {
                ::vox_config::ConfigField {
                    key: #env,
                    kind: #kind,
                    current: #current,
                    default: #default.to_string(),
                    group: #group_lit,
                    label: #label,
                    hint: #hint,
                }
            }
        });

        Ok(quote! {
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
                /// Process-lifetime snapshot. ponytail: OnceLock — env read once;
                /// re-read in tests via `from_env_uncached`.
                pub fn get() -> &'static Self where Self: Default {
                    static CELL: ::std::sync::OnceLock<#ty> = ::std::sync::OnceLock::new();
                    CELL.get_or_init(Self::from_env_uncached)
                }
            }
        })
    }
}

fn kind_tokens(k: Kind) -> TokenStream {
    match k {
        Kind::Bool => quote!(::vox_config::config_key::ConfigKind::Bool),
        Kind::Int | Kind::Uint => quote!(::vox_config::config_key::ConfigKind::Int),
        Kind::Float32 | Kind::Float64 => quote!(::vox_config::config_key::ConfigKind::Float),
        Kind::Str | Kind::Parse => quote!(::vox_config::config_key::ConfigKind::String),
    }
}

/// The env+toml read for one field, defaulting to the field's current value.
fn resolver_call(f: &Field) -> TokenStream {
    let id = &f.ident;
    let env = &f.env;
    if f.is_option {
        // Option<T: FromStr>: override only when env/toml present.
        return quote! {{
            match ::vox_config::env_parse::resolve_config_opt_str(#env) {
                Some(__s) => __s.parse().ok(),
                None => self.#id.clone(),
            }
        }};
    }
    match f.kind {
        Kind::Bool => quote!(::vox_config::env_parse::resolve_config_bool(#env, self.#id)),
        Kind::Uint => {
            quote!(::vox_config::env_parse::resolve_config_u64(#env, self.#id as u64) as _)
        }
        Kind::Int => {
            quote!(::vox_config::env_parse::resolve_config_i64(#env, self.#id as i64) as _)
        }
        Kind::Float32 => quote!(::vox_config::env_parse::resolve_config_f32(#env, self.#id)),
        Kind::Float64 => quote!(::vox_config::env_parse::resolve_config_f64(#env, self.#id)),
        Kind::Str => quote!(::vox_config::env_parse::resolve_config_str(#env, &self.#id)),
        // enum / other FromStr: override only when env/toml present (no Debug round-trip).
        Kind::Parse => quote! {{
            match ::vox_config::env_parse::resolve_config_opt_str(#env) {
                Some(__s) => __s.parse().unwrap_or_else(|_| self.#id.clone()),
                None => self.#id.clone(),
            }
        }},
    }
}

/// Map the `#[vox_config(group=…)]` string to a `Group` variant (compile error if unknown).
fn group_token(s: &str) -> syn::Result<TokenStream> {
    let variant = match s {
        "General" | "ModelsAndEndpoints" | "Tuning" | "Training" | "Orchestrator" | "Runtime"
        | "Storage" | "Mesh" | "Security" | "Telemetry" => s,
        other => {
            return Err(syn::Error::new(
                Span::call_site(),
                format!(
                    "unknown config group {other:?}; add a variant to vox_config::config_key::Group"
                ),
            ));
        }
    };
    let id = format_ident!("{variant}");
    Ok(quote!(::vox_config::config_key::Group::#id))
}
