use crate::web_ir::WebIrModule;
use serde::{Deserialize, Serialize};
use vox_compiler::hir::HirStateMachineDecl;
use vox_compiler::hir::{
    HirAgent, HirEndpointFn, HirFn, HirImport, HirMcpResource, HirMcpTool, HirRustImport, HirTable,
    HirTypeDef, HirUrlDecl,
};

/// The General Vox IR module structure, representing a machine-verifiable
/// and platform-agnostic view of a Vox program.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoxIrModule {
    pub version: String,
    pub metadata: VoxIrMetadata,
    pub module: VoxIrContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoxIrMetadata {
    pub compiler_version: String,
    pub generated_at: String,
    pub source_hash: String,
}

/// The internal structure containing the lowered program logic.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VoxIrContent {
    pub imports: Vec<HirImport>,
    pub rust_imports: Vec<HirRustImport>,
    pub functions: Vec<HirFn>,
    pub types: Vec<HirTypeDef>,

    pub endpoint_fns: Vec<HirEndpointFn>,
    pub tables: Vec<HirTable>,
    pub mcp_tools: Vec<HirMcpTool>,
    pub mcp_resources: Vec<HirMcpResource>,
    pub agents: Vec<HirAgent>,
    pub url_decls: Vec<HirUrlDecl>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_machines: Vec<HirStateMachineDecl>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_ir: Option<WebIrModule>,
}

pub mod lower;
