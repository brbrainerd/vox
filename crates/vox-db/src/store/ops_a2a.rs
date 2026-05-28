//! A2A messaging, agent OpLog, actor state, file locks, and heartbeats for [`crate::VoxDb`].
//!
//! These coordination primitives were previously mixed into `ops_ludus/gamify_extended.rs`.
//! They are not gamification-domain ops; they belong to the coordination / orchestration layer.

use std::time::{SystemTime, UNIX_EPOCH};

use turso::params;

use crate::store::types::{A2AMessageRow, StoreError};

fn a2a_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn default_a2a_consumer_id() -> String {
    std::env::var("VOX_A2A_CONSUMER_ID").unwrap_or_else(|_| format!("pid:{}", std::process::id()))
}

impl crate::VoxDb {
    // ── A2A Messages (a2a_messages) ───────────────────────────────────────────

    /// Acknowledge an A2A message by row id.
    pub async fn acknowledge_a2a_message_by_id(&self, id: i64) -> Result<(), StoreError> {
        let now_ms = a2a_now_ms();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE a2a_messages SET acknowledged=1, claim_owner=NULL, claim_until_ms=NULL,
                 processed_at_ms=?1 WHERE id=?2",
                    params![now_ms, id],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Send an A2A message.
    #[allow(clippy::too_many_arguments)]
    pub async fn send_a2a_message(
        &self,
        message_uuid: &str,
        sender_agent: &str,
        receiver_agent: &str,
        msg_type: &str,
        payload: &str,
        priority: i64,
        thread_id: Option<&str>,
        repository_id: &str,
    ) -> Result<(), StoreError> {
        let message_uuid = message_uuid.to_string();
        let sender_agent = sender_agent.to_string();
        let receiver_agent = receiver_agent.to_string();
        let msg_type = msg_type.to_string();
        let payload = payload.to_string();
        let thread_id = thread_id.map(str::to_string);
        let repository_id = repository_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO a2a_messages
             (message_uuid, sender_agent, receiver_agent, msg_type, payload, priority, thread_id, repository_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        message_uuid.as_str(),
                        sender_agent.as_str(),
                        receiver_agent.as_str(),
                        msg_type.as_str(),
                        payload.as_str(),
                        priority,
                        thread_id.as_deref(),
                        repository_id.as_str(),
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Poll unacknowledged messages for an agent in a repository.
    ///
    /// Claims a bounded batch for `default_a2a_consumer_id` so concurrent poll workers do not
    /// receive duplicate rows until the claim lease expires. Override the consumer via
    /// `VOX_A2A_CONSUMER_ID` or call [`Self::poll_a2a_inbox_claimed`].
    pub async fn poll_a2a_inbox(
        &self,
        agent_id: &str,
        repository_id: &str,
    ) -> Result<Vec<A2AMessageRow>, StoreError> {
        self.poll_a2a_inbox_claimed(
            agent_id,
            repository_id,
            &default_a2a_consumer_id(),
            256,
            120_000,
        )
        .await
    }

    /// Claim up to `limit` inbox rows for `consumer_id`, leasing each for `lease_ms` milliseconds.
    pub async fn poll_a2a_inbox_claimed(
        &self,
        receiver_agent: &str,
        repository_id: &str,
        consumer_id: &str,
        limit: i64,
        lease_ms: i64,
    ) -> Result<Vec<A2AMessageRow>, StoreError> {
        let limit = limit.clamp(1, 2048);
        let now_ms = a2a_now_ms();
        let claim_deadline = now_ms.saturating_add(lease_ms);
        let receiver_agent = receiver_agent.to_string();
        let repository_id = repository_id.to_string();
        let consumer_id = consumer_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();

        breaker
            .call(|| async move {
                conn.execute("BEGIN IMMEDIATE", ())
                    .await
                    .map_err(StoreError::from)?;

                let result = async {
                    let mut id_rows = conn
                        .query(
                            "SELECT id FROM a2a_messages
                     WHERE receiver_agent = ?1 AND repository_id = ?2 AND acknowledged = 0
                       AND (claim_until_ms IS NULL OR claim_until_ms < ?3 OR claim_owner = ?4)
                     ORDER BY priority DESC, id ASC
                     LIMIT ?5",
                            params![
                                receiver_agent.as_str(),
                                repository_id.as_str(),
                                now_ms,
                                consumer_id.as_str(),
                                limit
                            ],
                        )
                        .await?;

                    let mut ids: Vec<i64> = Vec::new();
                    while let Some(row) = id_rows.next().await? {
                        ids.push(row.get(0)?);
                    }

                    for id in &ids {
                        conn.execute(
                            "UPDATE a2a_messages SET
                           claim_owner = ?1,
                           claim_until_ms = ?2,
                           delivery_attempts = delivery_attempts + IIF(
                             COALESCE(claim_owner, '') = ?1 AND COALESCE(claim_until_ms, 0) >= ?3,
                             0,
                             1
                           ),
                           last_claim_error = NULL
                         WHERE id = ?4",
                            params![consumer_id.as_str(), claim_deadline, now_ms, id],
                        )
                        .await?;
                    }

                    let mut out = Vec::new();
                    for id in ids {
                        let mut rows = conn
                            .query(
                                "SELECT id, message_uuid, sender_agent, receiver_agent, msg_type, payload,
                            priority, thread_id, acknowledged, created_at, repository_id,
                            claim_owner, claim_until_ms, COALESCE(delivery_attempts, 0),
                            last_claim_error, processed_at_ms
                     FROM a2a_messages WHERE id = ?1",
                                params![id],
                            )
                            .await?;
                        if let Some(row) = rows.next().await? {
                            let ack: i64 = row.get(8).unwrap_or(0);
                            out.push(A2AMessageRow {
                                id: row.get(0)?,
                                message_uuid: row.get(1)?,
                                sender_agent: row.get(2)?,
                                receiver_agent: row.get(3)?,
                                msg_type: row.get(4)?,
                                payload: row.get(5)?,
                                priority: row.get(6)?,
                                thread_id: row.get(7)?,
                                acknowledged: ack != 0,
                                created_at: row.get(9)?,
                                repository_id: row.get(10)?,
                                claim_owner: row.get(11)?,
                                claim_until_ms: row.get(12)?,
                                delivery_attempts: row.get(13)?,
                                last_claim_error: row.get(14)?,
                                processed_at_ms: row.get(15)?,
                            });
                        }
                    }

                    conn.execute("COMMIT", ()).await?;
                    Ok::<Vec<A2AMessageRow>, StoreError>(out)
                }
                .await;

                if result.is_err() {
                    let _ = conn.execute("ROLLBACK", ()).await;
                }
                result
            })
            .await
    }

    /// Acknowledge an A2A message by UUID.
    pub async fn acknowledge_a2a_message_by_uuid(&self, uuid: &str) -> Result<(), StoreError> {
        let now_ms = a2a_now_ms();
        let uuid = uuid.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE a2a_messages SET acknowledged=1, claim_owner=NULL, claim_until_ms=NULL,
                 processed_at_ms=?1 WHERE message_uuid=?2",
                    params![now_ms, uuid.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Prune old acknowledged A2A messages older than N days.
    pub async fn prune_a2a_messages(&self, older_than_days: u32) -> Result<u64, StoreError> {
        let sql = format!(
            "DELETE FROM a2a_messages WHERE acknowledged=1 AND created_at < datetime('now', '-{} days')",
            older_than_days
        );
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let affected = conn.execute(&sql, ()).await?;
                Ok::<_, StoreError>(affected as u64)
            })
            .await
    }

    // ── OpLog (agent_oplog) ───────────────────────────────────────────────────

    /// Append an operation log entry.
    #[allow(clippy::too_many_arguments)]
    pub async fn append_oplog_entry(
        &self,
        agent_id: &str,
        operation_id: &str,
        kind_json: &str,
        description: &str,
        predecessor_hash: Option<&str>,
        model_id: Option<&str>,
        change_id: Option<i64>,
        timestamp_ms: i64,
        repository_id: &str,
    ) -> Result<(), StoreError> {
        let agent_id = agent_id.to_string();
        let operation_id = operation_id.to_string();
        let kind_json = kind_json.to_string();
        let description = description.to_string();
        let predecessor_hash = predecessor_hash.map(str::to_string);
        let model_id = model_id.map(str::to_string);
        let repository_id = repository_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO agent_oplog
             (agent_id, operation_id, kind, description, predecessor_hash, model_id, change_id, timestamp_ms, repository_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        agent_id.as_str(),
                        operation_id.as_str(),
                        kind_json.as_str(),
                        description.as_str(),
                        predecessor_hash.as_deref(),
                        model_id.as_deref(),
                        change_id,
                        timestamp_ms,
                        repository_id.as_str(),
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// List oplog entries for a repository (optionally filtered by agent), newest first.
    pub async fn list_oplog_entries(
        &self,
        agent_id: Option<&str>,
        repository_id: &str,
        limit: u32,
    ) -> Result<Vec<Vec<Option<String>>>, StoreError> {
        let mut cursor = if let Some(aid) = agent_id {
            self.conn
                .query(
                    "SELECT operation_id, agent_id, kind, description, predecessor_hash, model_id,
                              CAST(change_id AS TEXT), CAST(timestamp_ms AS TEXT), CAST(undone AS TEXT)
                       FROM agent_oplog WHERE repository_id=?1 AND agent_id=?2
                       ORDER BY timestamp_ms DESC LIMIT ?3",
                    params![repository_id, aid, limit as i64],
                )
                .await?
        } else {
            self.conn
                .query(
                    "SELECT operation_id, agent_id, kind, description, predecessor_hash, model_id,
                              CAST(change_id AS TEXT), CAST(timestamp_ms AS TEXT), CAST(undone AS TEXT)
                       FROM agent_oplog WHERE repository_id=?1
                       ORDER BY timestamp_ms DESC LIMIT ?2",
                    params![repository_id, limit as i64],
                )
                .await?
        };
        let mut out = Vec::new();
        while let Some(row) = cursor.next().await? {
            let cols: Vec<Option<String>> = (0..9)
                .map(|i| row.get::<Option<String>>(i).unwrap_or(None))
                .collect();
            out.push(cols);
        }
        Ok(out)
    }

    /// Mark an oplog entry as undone (or re-done).
    pub async fn set_oplog_undone(
        &self,
        operation_id: &str,
        undone: bool,
    ) -> Result<(), StoreError> {
        let operation_id = operation_id.to_string();
        let undone_flag = if undone { 1i64 } else { 0i64 };
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE agent_oplog SET undone=?1 WHERE operation_id=?2",
                    params![undone_flag, operation_id.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    // ── Actor State (actor_state) ─────────────────────────────────────────────

    /// Save a JSON-serialized actor state value under a key (upsert).
    pub async fn save_actor_state(&self, key: &str, value_json: &str) -> Result<(), StoreError> {
        let key = key.to_string();
        let value_json = value_json.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO actor_state (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=datetime('now')",
                    params![key.as_str(), value_json.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Load a JSON-serialized actor state value by key.
    pub async fn load_actor_state(&self, key: &str) -> Result<Option<String>, StoreError> {
        let mut rows = self
            .conn
            .query("SELECT value FROM actor_state WHERE key=?1", params![key])
            .await?;
        Ok(rows
            .next()
            .await?
            .and_then(|r| r.get::<Option<String>>(0).unwrap_or(None)))
    }

    /// Delete an actor state entry.
    pub async fn delete_actor_state(&self, key: &str) -> Result<(), StoreError> {
        let key = key.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "DELETE FROM actor_state WHERE key=?1",
                    params![key.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    // ── Agent File Locks (agent_locks) ────────────────────────────────────────

    /// Try to acquire a file lock for an agent. Returns true if acquired.
    pub async fn acquire_file_lock(
        &self,
        path: &str,
        agent_id: &str,
        repository_id: &str,
    ) -> Result<bool, StoreError> {
        let path = path.to_string();
        let agent_id = agent_id.to_string();
        let repository_id = repository_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let affected = conn
                    .execute(
                        "INSERT OR IGNORE INTO agent_locks (path, agent_id, repository_id, acquired_at)
             VALUES (?1, ?2, ?3, datetime('now'))",
                        params![path.as_str(), agent_id.as_str(), repository_id.as_str()],
                    )
                    .await?;
                Ok::<_, StoreError>(affected > 0)
            })
            .await
    }

    /// Release a file lock held by an agent.
    pub async fn release_file_lock(&self, path: &str, agent_id: &str) -> Result<(), StoreError> {
        let path = path.to_string();
        let agent_id = agent_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "DELETE FROM agent_locks WHERE path=?1 AND agent_id=?2",
                    params![path.as_str(), agent_id.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// List current file locks for a repository.
    pub async fn list_file_locks(
        &self,
        repository_id: &str,
    ) -> Result<Vec<(String, String, String)>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT path, agent_id, acquired_at FROM agent_locks WHERE repository_id=?1",
                params![repository_id],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push((
                row.get::<String>(0)?,
                row.get::<String>(1)?,
                row.get::<String>(2)?,
            ));
        }
        Ok(out)
    }

    // ── Agent Heartbeats (agent_heartbeats) ───────────────────────────────────

    /// Upsert a heartbeat record for an agent.
    pub async fn upsert_heartbeat(
        &self,
        agent_id: &str,
        repository_id: &str,
        status: &str,
    ) -> Result<(), StoreError> {
        let agent_id = agent_id.to_string();
        let repository_id = repository_id.to_string();
        let status = status.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO agent_heartbeats (agent_id, repository_id, status, last_seen)
             VALUES (?1, ?2, ?3, datetime('now'))
             ON CONFLICT(agent_id, repository_id) DO UPDATE SET
               status=excluded.status, last_seen=datetime('now')",
                    params![agent_id.as_str(), repository_id.as_str(), status.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// List heartbeats for a repository.
    pub async fn list_heartbeats(
        &self,
        repository_id: &str,
    ) -> Result<Vec<(String, String, String)>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT agent_id, status, last_seen FROM agent_heartbeats WHERE repository_id=?1",
                params![repository_id],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push((
                row.get::<String>(0)?,
                row.get::<String>(1)?,
                row.get::<String>(2)?,
            ));
        }
        Ok(out)
    }

    /// Delete heartbeats not updated within `timeout_secs` seconds.
    pub async fn prune_stale_heartbeats(&self, timeout_secs: i64) -> Result<u64, StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let affected = conn
                    .execute(
                        "DELETE FROM agent_heartbeats
             WHERE last_seen < datetime('now', '-' || ?1 || ' seconds')",
                        params![timeout_secs],
                    )
                    .await?;
                Ok::<_, StoreError>(affected as u64)
            })
            .await
    }
}
