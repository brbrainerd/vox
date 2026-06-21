//! GC sweep: delete CAS objects with zero live references (design §5.4, Rev 2 Correction 3).
//!
//! An object is eligible for deletion when:
//!   - No rows in `archive_membership` reference it (all archiving windows have been deleted)
//!   - AND it is not referenced by any live (non-trimmed) `context_window_items.content_hash`

use crate::VoxDb;
use crate::store::types::StoreError;

/// Delete every object that has no archive_membership edges AND no live context_window_items reference.
/// "Live" means `trimmed_at IS NULL` — trimmed items no longer protect their content.
/// Also cascades: removes chunk_members rows pointing to deleted objects.
/// Returns the number of objects deleted.
pub async fn sweep_unreferenced(db: &VoxDb) -> Result<i64, StoreError> {
    let breaker = db.breaker.clone();
    let conn = db.conn.clone();
    breaker
        .call(|| async move {
            // Identify unreferenced hashes:
            // not in archive_membership AND not in live (non-trimmed) context_window_items
            let unreferenced_subquery =
                "SELECT o.hash FROM objects o \
                 WHERE o.hash NOT IN (SELECT ref_hash FROM archive_membership) \
                   AND o.hash NOT IN (SELECT content_hash FROM context_window_items WHERE trimmed_at IS NULL)";

            // Cascade: remove chunk_members rows whose chunk_hash is being deleted.
            conn.execute(
                &format!(
                    "DELETE FROM chunk_members WHERE chunk_hash IN ({unreferenced_subquery})"
                ),
                (),
            )
            .await?;

            // Delete the objects themselves and capture the count.
            conn.execute(
                &format!("DELETE FROM objects WHERE hash IN ({unreferenced_subquery})"),
                (),
            )
            .await?;

            Ok::<(), StoreError>(())
        })
        .await?;

    // `changes()` reflects the last statement (DELETE FROM objects).
    let mut rows = db.conn.query("SELECT changes()", ()).await?;
    let row = rows
        .next()
        .await?
        .ok_or_else(|| StoreError::Db("no row from changes()".into()))?;
    Ok(row
        .get::<i64>(0)
        .map_err(|e| StoreError::Db(e.to_string()))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::membership;

    #[tokio::test]
    async fn shared_object_survives_until_no_references() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory)
            .await
            .expect("db");

        // Store an object and add two archive_membership edges.
        db.store("k", b"shared").await.unwrap();
        let h = crate::hash::content_hash(b"shared");
        membership::add_edge(&db, "w1", &h).await.unwrap();
        membership::add_edge(&db, "w2", &h).await.unwrap();

        // Remove w1's edge — object still referenced by w2.
        membership::drop_window_edges(&db, "w1").await.unwrap();
        sweep_unreferenced(&db).await.unwrap();
        assert!(
            db.get(&h).await.is_ok(),
            "must survive at refs=1 (w2 still references it)"
        );

        // Remove w2's edge — no references remain.
        membership::drop_window_edges(&db, "w2").await.unwrap();
        sweep_unreferenced(&db).await.unwrap();
        assert!(db.get(&h).await.is_err(), "must be GC'd when refs=0");
    }

    #[tokio::test]
    async fn live_context_window_item_protects_object() {
        use crate::context_window_store;
        let db = crate::VoxDb::connect(crate::DbConfig::Memory)
            .await
            .expect("db");

        let payload = b"live item content";
        // Create a window and add an item — this stores the content in objects and
        // references it via context_window_items.content_hash (trimmed_at IS NULL).
        context_window_store::create_window(&db, "wlive", "repo", "chat", "wlive", 1)
            .await
            .unwrap();
        context_window_store::add_item(&db, "ilive", "wlive", 0, "user", "msg", payload, 1)
            .await
            .unwrap();

        // No archive_membership edges — but the object is still live via context_window_items.
        let h = crate::hash::content_hash(payload);
        sweep_unreferenced(&db).await.unwrap();
        assert!(
            db.get(&h).await.is_ok(),
            "live item must be protected from GC"
        );
    }

    #[tokio::test]
    async fn trimmed_item_does_not_protect_object() {
        use crate::context_window_store;
        let db = crate::VoxDb::connect(crate::DbConfig::Memory)
            .await
            .expect("db");

        let payload = b"trimmed item content";
        context_window_store::create_window(&db, "wtrim", "repo", "chat", "wtrim", 1)
            .await
            .unwrap();
        context_window_store::add_item(&db, "itrim", "wtrim", 0, "user", "msg", payload, 1)
            .await
            .unwrap();
        let h = crate::hash::content_hash(payload);

        // Trim the item — it is now a soft-deleted reference.
        context_window_store::mark_item_trimmed(&db, "itrim", 2)
            .await
            .unwrap();

        // No archive_membership edges and the only reference is trimmed → eligible for GC.
        sweep_unreferenced(&db).await.unwrap();
        assert!(
            db.get(&h).await.is_err(),
            "trimmed item must not protect object from GC"
        );
    }
}
