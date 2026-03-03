use crate::expr::Expr;
use crate::span::Span;
use crate::types::TypeExpr;

/// Constant declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstDecl {
    pub name: String,
    pub type_ann: Option<TypeExpr>,
    pub value: Expr,
    pub is_pub: bool,
    pub is_deprecated: bool,
    pub is_build_const: bool,
    pub span: Span,
}

/// A block of typed configuration / secrets.
/// `@config env: DATABASE_URL: str, API_KEY: str`
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigDecl {
    pub name: String,
    pub fields: Vec<crate::decl::db::TableField>,
    pub is_deprecated: bool,
    pub span: Span,
}

/// Declarative container environment specification.
///
/// Allows users to define their deployment environment entirely in Vox:
/// ```vox
/// environment production:
///     base "node:22-alpine"
///     packages ["curl", "git"]
///     env NODE_ENV = "production"
///     env PORT = "3000"
///     expose [3000, 443]
///     volumes ["/data"]
///     cmd ["npx", "tsx", "server.ts"]
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentDecl {
    /// Environment name, e.g. `"production"`, `"staging"`, `"dev"`.
    pub name: String,
    /// Base container image, e.g. `"node:22-alpine"`.
    pub base_image: Option<String>,
    /// System packages to install.
    pub packages: Vec<String>,
    /// Environment variable key-value pairs.
    pub env_vars: Vec<(String, String)>,
    /// Ports to expose.
    pub exposed_ports: Vec<u16>,
    /// Volume mount points.
    pub volumes: Vec<String>,
    /// Working directory inside the container.
    pub workdir: Option<String>,
    /// Default CMD for the container.
    pub cmd: Vec<String>,
    /// Files/directories to copy into the container as `(src, dest)`.
    pub copy_instructions: Vec<(String, String)>,
    /// RUN commands to execute during build.
    pub run_commands: Vec<String>,
    /// Whether this declaration is deprecated.
    pub is_deprecated: bool,
    /// Source span.
    pub span: Span,
}
