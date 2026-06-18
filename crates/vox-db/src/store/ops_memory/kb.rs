use turso::params;

impl crate::VoxDb {
    /// Insert a new knowledge base row.
    pub async fn kb_create(
        &self,
        id: &str,
        name: &str,
        description: &str,
        now_ms: i64,
    ) -> Result<(), crate::store::types::StoreError> {
        let id = id.to_string();
        let name = name.to_string();
        let description = description.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO knowledge_bases (id, name, description, created_at_ms, updated_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?4)",
                    params![id.as_str(), name.as_str(), description.as_str(), now_ms],
                )
                .await?;
                Ok(())
            })
            .await
    }

    /// List all knowledge bases ordered by name.
    pub async fn kb_list(&self) -> Result<Vec<crate::KbRow>, crate::store::types::StoreError> {
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, name, description, created_at_ms, updated_at_ms, entry_count
                         FROM knowledge_bases ORDER BY name",
                        params![],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(row) = rows.next().await? {
                    out.push(crate::KbRow {
                        id: row.get::<String>(0)?,
                        name: row.get::<String>(1)?,
                        description: row.get::<String>(2)?,
                        created_at_ms: row.get::<i64>(3)?,
                        updated_at_ms: row.get::<i64>(4)?,
                        entry_count: row.get::<i64>(5)?,
                    });
                }
                Ok(out)
            })
            .await
    }

    /// Delete a knowledge base by id (cascades to entries and rules via FK).
    pub async fn kb_delete(&self, id: &str) -> Result<(), crate::store::types::StoreError> {
        let id = id.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "DELETE FROM knowledge_bases WHERE id = ?1",
                    params![id.as_str()],
                )
                .await?;
                Ok(())
            })
            .await
    }

    /// Insert a KB entry and increment `entry_count` on the parent KB.
    /// Both SQL statements run in a transaction to prevent drift.
    pub async fn kb_add_entry(
        &self,
        entry_id: &str,
        kb_id: &str,
        content: &str,
        source_signal: &str,
        source_ref: Option<&str>,
        routing_confidence: f64,
        tags: &str,
        now_ms: i64,
    ) -> Result<(), crate::store::types::StoreError> {
        let entry_id = entry_id.to_string();
        let kb_id = kb_id.to_string();
        let content = content.to_string();
        let source_signal = source_signal.to_string();
        let source_ref = source_ref.map(str::to_string);
        let tags = tags.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                conn.execute_batch("BEGIN").await?;
                conn.execute(
                    "INSERT INTO kb_entries
                         (id, kb_id, content, source_signal, source_ref, routing_confidence,
                          tags, created_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        entry_id.as_str(),
                        kb_id.as_str(),
                        content.as_str(),
                        source_signal.as_str(),
                        source_ref.as_deref(),
                        routing_confidence,
                        tags.as_str(),
                        now_ms,
                    ],
                )
                .await?;
                conn.execute(
                    "UPDATE knowledge_bases
                     SET entry_count = entry_count + 1, updated_at_ms = ?2
                     WHERE id = ?1",
                    params![kb_id.as_str(), now_ms],
                )
                .await?;
                conn.execute_batch("COMMIT").await?;
                Ok(())
            })
            .await
    }

    /// List entries for a KB, newest first.
    pub async fn kb_list_entries(
        &self,
        kb_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<crate::KbEntryRow>, crate::store::types::StoreError> {
        let kb_id = kb_id.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, kb_id, content, source_signal, source_ref, routing_confidence,
                                tags, created_at_ms, last_accessed_at_ms, access_count,
                                accepted, mens_queued
                         FROM kb_entries WHERE kb_id = ?1
                         ORDER BY created_at_ms DESC LIMIT ?2 OFFSET ?3",
                        params![kb_id.as_str(), limit, offset],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(r) = rows.next().await? {
                    out.push(crate::KbEntryRow {
                        id: r.get::<String>(0)?,
                        kb_id: r.get::<String>(1)?,
                        content: r.get::<String>(2)?,
                        source_signal: r.get::<String>(3)?,
                        source_ref: r.get::<Option<String>>(4)?,
                        routing_confidence: r.get::<f64>(5)?,
                        tags: r.get::<String>(6)?,
                        created_at_ms: r.get::<i64>(7)?,
                        last_accessed_at_ms: r.get::<Option<i64>>(8)?,
                        access_count: r.get::<i64>(9)?,
                        accepted: r.get::<i64>(10)?,
                        mens_queued: r.get::<i64>(11)?,
                    });
                }
                Ok(out)
            })
            .await
    }

    /// Set the `accepted` flag on an entry and optionally mark it for MENS queuing.
    pub async fn kb_review_entry(
        &self,
        entry_id: &str,
        accepted: bool,
        queue_mens: bool,
    ) -> Result<(), crate::store::types::StoreError> {
        let entry_id = entry_id.to_string();
        let accepted_int: i64 = if accepted { 1 } else { 0 };
        let mens_int: i64 = if queue_mens { 1 } else { 0 };
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE kb_entries SET accepted = ?2, mens_queued = ?3 WHERE id = ?1",
                    params![entry_id.as_str(), accepted_int, mens_int],
                )
                .await?;
                Ok(())
            })
            .await
    }

    /// Delete a specific entry and decrement the parent KB's `entry_count`.
    pub async fn kb_delete_entry(
        &self,
        entry_id: &str,
    ) -> Result<(), crate::store::types::StoreError> {
        let entry_id = entry_id.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                conn.execute_batch("BEGIN").await?;
                // Fetch the kb_id first so we can decrement entry_count
                let mut rows = conn
                    .query(
                        "SELECT kb_id FROM kb_entries WHERE id = ?1",
                        params![entry_id.as_str()],
                    )
                    .await?;
                if let Some(row) = rows.next().await? {
                    let kb_id: String = row.get::<String>(0)?;
                    conn.execute(
                        "DELETE FROM kb_entries WHERE id = ?1",
                        params![entry_id.as_str()],
                    )
                    .await?;
                    conn.execute(
                        "UPDATE knowledge_bases
                         SET entry_count = MAX(0, entry_count - 1)
                         WHERE id = ?1",
                        params![kb_id.as_str()],
                    )
                    .await?;
                }
                conn.execute_batch("COMMIT").await?;
                Ok(())
            })
            .await
    }

    /// List recent entries across all KBs (the "knowledge feed"), newest first.
    pub async fn kb_get_feed(
        &self,
        limit: i64,
    ) -> Result<Vec<crate::KbEntryRow>, crate::store::types::StoreError> {
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, kb_id, content, source_signal, source_ref, routing_confidence,
                                tags, created_at_ms, last_accessed_at_ms, access_count,
                                accepted, mens_queued
                         FROM kb_entries ORDER BY created_at_ms DESC LIMIT ?1",
                        params![limit],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(r) = rows.next().await? {
                    out.push(crate::KbEntryRow {
                        id: r.get::<String>(0)?,
                        kb_id: r.get::<String>(1)?,
                        content: r.get::<String>(2)?,
                        source_signal: r.get::<String>(3)?,
                        source_ref: r.get::<Option<String>>(4)?,
                        routing_confidence: r.get::<f64>(5)?,
                        tags: r.get::<String>(6)?,
                        created_at_ms: r.get::<i64>(7)?,
                        last_accessed_at_ms: r.get::<Option<i64>>(8)?,
                        access_count: r.get::<i64>(9)?,
                        accepted: r.get::<i64>(10)?,
                        mens_queued: r.get::<i64>(11)?,
                    });
                }
                Ok(out)
            })
            .await
    }

    /// Insert a routing rule.
    pub async fn kb_add_rule(
        &self,
        rule_id: &str,
        kb_id: &str,
        rule_type: &str,
        pattern: &str,
        priority: i64,
        now_ms: i64,
    ) -> Result<(), crate::store::types::StoreError> {
        let rule_id = rule_id.to_string();
        let kb_id = kb_id.to_string();
        let rule_type = rule_type.to_string();
        let pattern = pattern.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO kb_routing_rules
                         (id, kb_id, rule_type, pattern, priority, created_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        rule_id.as_str(),
                        kb_id.as_str(),
                        rule_type.as_str(),
                        pattern.as_str(),
                        priority,
                        now_ms,
                    ],
                )
                .await?;
                Ok(())
            })
            .await
    }

    /// List routing rules for a KB, ordered by priority descending.
    pub async fn kb_list_rules(
        &self,
        kb_id: &str,
    ) -> Result<Vec<crate::KbRuleRow>, crate::store::types::StoreError> {
        let kb_id = kb_id.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, kb_id, rule_type, pattern, priority, created_at_ms
                         FROM kb_routing_rules WHERE kb_id = ?1
                         ORDER BY priority DESC",
                        params![kb_id.as_str()],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(r) = rows.next().await? {
                    out.push(crate::KbRuleRow {
                        id: r.get::<String>(0)?,
                        kb_id: r.get::<String>(1)?,
                        rule_type: r.get::<String>(2)?,
                        pattern: r.get::<String>(3)?,
                        priority: r.get::<i64>(4)?,
                        created_at_ms: r.get::<i64>(5)?,
                    });
                }
                Ok(out)
            })
            .await
    }

    /// Substring search over accepted KB entries.
    /// Used for BM25-style routing tier and retrieval bundle injection.
    pub async fn kb_search_entries(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<crate::KbEntryRow>, crate::store::types::StoreError> {
        let query_lower = format!("%{}%", query.to_ascii_lowercase());
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, kb_id, content, source_signal, source_ref, routing_confidence,
                                tags, created_at_ms, last_accessed_at_ms, access_count,
                                accepted, mens_queued
                         FROM kb_entries
                         WHERE accepted = 1 AND lower(content) LIKE ?1
                         ORDER BY routing_confidence DESC, created_at_ms DESC LIMIT ?2",
                        params![query_lower.as_str(), limit],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(r) = rows.next().await? {
                    out.push(crate::KbEntryRow {
                        id: r.get::<String>(0)?,
                        kb_id: r.get::<String>(1)?,
                        content: r.get::<String>(2)?,
                        source_signal: r.get::<String>(3)?,
                        source_ref: r.get::<Option<String>>(4)?,
                        routing_confidence: r.get::<f64>(5)?,
                        tags: r.get::<String>(6)?,
                        created_at_ms: r.get::<i64>(7)?,
                        last_accessed_at_ms: r.get::<Option<i64>>(8)?,
                        access_count: r.get::<i64>(9)?,
                        accepted: r.get::<i64>(10)?,
                        mens_queued: r.get::<i64>(11)?,
                    });
                }
                Ok(out)
            })
            .await
    }

    /// Fetch entries not yet queued for MENS training (both accepted and rejected).
    /// Accepted entries → SFT pairs; rejected entries → DPO pairs.
    pub async fn kb_unqueued_training_entries(
        &self,
        limit: i64,
    ) -> Result<Vec<crate::KbEntryRow>, crate::store::types::StoreError> {
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, kb_id, content, source_signal, source_ref, routing_confidence,
                                tags, created_at_ms, last_accessed_at_ms, access_count,
                                accepted, mens_queued
                         FROM kb_entries
                         WHERE mens_queued = 0
                         ORDER BY created_at_ms ASC LIMIT ?1",
                        params![limit],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(r) = rows.next().await? {
                    out.push(crate::KbEntryRow {
                        id: r.get::<String>(0)?,
                        kb_id: r.get::<String>(1)?,
                        content: r.get::<String>(2)?,
                        source_signal: r.get::<String>(3)?,
                        source_ref: r.get::<Option<String>>(4)?,
                        routing_confidence: r.get::<f64>(5)?,
                        tags: r.get::<String>(6)?,
                        created_at_ms: r.get::<i64>(7)?,
                        last_accessed_at_ms: r.get::<Option<i64>>(8)?,
                        access_count: r.get::<i64>(9)?,
                        accepted: r.get::<i64>(10)?,
                        mens_queued: r.get::<i64>(11)?,
                    });
                }
                Ok(out)
            })
            .await
    }

    /// Mark a batch of entries as MENS-queued.
    pub async fn kb_mark_mens_queued(
        &self,
        ids: &[String],
    ) -> Result<(), crate::store::types::StoreError> {
        for id in ids {
            let id = id.clone();
            let conn = self.conn.clone();
            let breaker = self.breaker.clone();
            breaker
                .call(|| async move {
                    conn.execute(
                        "UPDATE kb_entries SET mens_queued = 1 WHERE id = ?1",
                        params![id.as_str()],
                    )
                    .await?;
                    Ok::<_, crate::store::types::StoreError>(())
                })
                .await?;
        }
        Ok(())
    }

    /// Check if content already exists in a KB (content-hash deduplication).
    /// Returns the ID of an existing identical entry, or None.
    pub async fn kb_find_duplicate(
        &self,
        kb_id: &str,
        content: &str,
    ) -> Result<Option<String>, crate::store::types::StoreError> {
        let kb_id = kb_id.to_string();
        let content = content.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id FROM kb_entries WHERE kb_id = ?1 AND content = ?2 LIMIT 1",
                        params![kb_id.as_str(), content.as_str()],
                    )
                    .await?;
                if let Some(r) = rows.next().await? {
                    Ok(Some(r.get::<String>(0)?))
                } else {
                    Ok(None)
                }
            })
            .await
    }

    /// Fetch a single KB entry by ID.
    pub async fn kb_get_entry(
        &self,
        entry_id: &str,
    ) -> Result<Option<crate::KbEntryRow>, crate::store::types::StoreError> {
        let entry_id = entry_id.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, kb_id, content, source_signal, source_ref, routing_confidence,
                                tags, created_at_ms, last_accessed_at_ms, access_count,
                                accepted, mens_queued
                         FROM kb_entries WHERE id = ?1 LIMIT 1",
                        params![entry_id.as_str()],
                    )
                    .await?;
                if let Some(r) = rows.next().await? {
                    Ok(Some(crate::KbEntryRow {
                        id: r.get::<String>(0)?,
                        kb_id: r.get::<String>(1)?,
                        content: r.get::<String>(2)?,
                        source_signal: r.get::<String>(3)?,
                        source_ref: r.get::<Option<String>>(4)?,
                        routing_confidence: r.get::<f64>(5)?,
                        tags: r.get::<String>(6)?,
                        created_at_ms: r.get::<i64>(7)?,
                        last_accessed_at_ms: r.get::<Option<i64>>(8)?,
                        access_count: r.get::<i64>(9)?,
                        accepted: r.get::<i64>(10)?,
                        mens_queued: r.get::<i64>(11)?,
                    }))
                } else {
                    Ok(None)
                }
            })
            .await
    }
}
