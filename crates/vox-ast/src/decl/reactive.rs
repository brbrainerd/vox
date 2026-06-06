//! Decl metadata mutators for schema / reactive hints (OP-0207).

use super::types::Decl;

impl Decl {
    pub fn set_description(&mut self, desc: String) {
        match self {
            Decl::Table(t) => t.description = Some(desc),
            Decl::Collection(c) => c.description = Some(desc),
            Decl::McpTool(m) => m.description = desc,
            Decl::McpResource(m) => m.description = desc,
            _ => {}
        }
    }
    /// Stores an optional JSON-schema–style layout hint on [`Decl::TypeDef`] and [`Decl::Table`].
    pub fn set_json_layout(&mut self, layout: String) {
        match self {
            Decl::TypeDef(t) => t.json_layout = Some(layout),
            Decl::Table(t) => t.json_layout = Some(layout),
            _ => {}
        }
    }
}
