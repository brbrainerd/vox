//! Cold-tier compaction: emit a synthetic `OperationKind::Checkpoint` op encoding
//! projection state and prune warm rows below the checkpoint's op_id_lo.

use std::sync::Arc;

use super::persist::{PersistContext, PersistError};
use super::{OperationId, OperationKind};

/// Snapshot all projections, write a Checkpoint op, and prune warm rows below `up_to`.
///
/// This is a stub — full implementation lands in P3-T9 when the `Projection` trait is defined.
pub async fn compact_now(ctx: Arc<PersistContext>, up_to: OperationId) -> Result<(), PersistError> {
    let _kind = OperationKind::Checkpoint {
        op_id_lo: 0,
        op_id_hi: up_to.0,
        projection_blake3: [0u8; 32],
        payload_blob_id: 0,
    };
    // Full implementation: snapshot projections → blake3 → insert blob → prune warm rows.
    let _ = ctx;
    Ok(())
}
