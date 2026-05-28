use super::{VoxIrContent, VoxIrMetadata, VoxIrModule};
use chrono::Utc;
use vox_compiler::hir::HirModule;

use sha3::{Digest, Sha3_256};

/// Lower a HirModule into the stable VoxIrModule representation.
pub fn lower_hir_to_vox_ir(hir: &HirModule, source: Option<&str>) -> VoxIrModule {
    let source_hash = if let Some(src) = source {
        let mut hasher = Sha3_256::new();
        hasher.update(src.as_bytes());
        format!("{:x}", hasher.finalize())
    } else {
        "".to_string()
    };

    let web_ir = crate::web_ir::lower::lower_hir_to_web_ir(hir);

    VoxIrModule {
        version: "2.0.0".to_string(),
        metadata: VoxIrMetadata {
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
            generated_at: Utc::now().to_rfc3339(),
            source_hash,
        },
        module: VoxIrContent {
            imports: hir.imports.clone(),
            rust_imports: hir.rust_imports.clone(),
            functions: hir.functions.clone(),
            types: hir.types.clone(),

            endpoint_fns: hir.endpoint_fns.clone(),
            tables: hir.tables.clone(),
            mcp_tools: hir.mcp_tools.clone(),
            mcp_resources: hir.mcp_resources.clone(),
            agents: hir.agents.clone(),
            url_decls: hir.url_decls.clone(),
            state_machines: hir.state_machines.clone(),
            web_ir: Some(web_ir),
        },
    }
}
