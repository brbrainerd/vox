//! Behavioral learning, pattern detection, command analytics, and training data export
//! for [`VoxDb`].
//!
//! Tables covered (all V3 schema):
//! - **`behavior_events`** — raw user/agent action log.
//! - **`learned_patterns`** — inferred habits and preferences with confidence scores.
//! - **`user_preferences`** — key-value preference store.
//! - **`llm_interactions`** + **`llm_feedback`** — read side for RLHF data export.

use turso::params;
use vox_db_types::DbUserId;

use crate::store::types::{
    BehaviorEventEntry, CommandFrequencyEntry, LearnedPatternEntry, SaveSnippetParams,
    SnippetEntry, StoreError, TrainingPair,
};

impl crate::VoxDb {
    // ── Behavior Events (behavior_events) ────────────────────────────────────

    /// Append a row to `behavior_events`. Returns the inserted `rowid`.
    ///
    /// Called from `vox-db/src/learning.rs` `BehavioralLearner::observe`.
    pub async fn record_behavior_event(
        &self,
        user_id: &str,
        event_type: &str,
        context: Option<&str>,
        metadata: Option<&str>,
    ) -> Result<i64, StoreError> {
        let user_id = user_id.to_string();
        let event_type = event_type.to_string();
        let context = context.map(str::to_string);
        let metadata = metadata.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO behavior_events (user_id, event_type, context, metadata)
                 VALUES (?1, ?2, ?3, ?4)",
                    params![
                        user_id.as_str(),
                        event_type.as_str(),
                        context.as_deref(),
                        metadata.as_deref()
                    ],
                )
                .await?;
                Ok::<_, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    /// Fetch `behavior_events` for `user_id`, newest first, optionally filtered by `event_type`.
    ///
    /// Called from `vox-db/src/learning.rs`.
    pub async fn get_behavior_events(
        &self,
        user_id: &str,
        event_type: Option<&str>,
        limit: i64,
    ) -> Result<Vec<BehaviorEventEntry>, StoreError> {
        let lim = limit.clamp(1, 50_000);
        let mut rows = match event_type {
            Some(t) => {
                self.conn
                    .query(
                        "SELECT id, user_id, event_type, context, metadata, created_at
                         FROM behavior_events
                         WHERE user_id = ?1 AND event_type = ?2
                         ORDER BY created_at DESC LIMIT ?3",
                        params![user_id, t, lim],
                    )
                    .await?
            }
            None => {
                self.conn
                    .query(
                        "SELECT id, user_id, event_type, context, metadata, created_at
                         FROM behavior_events
                         WHERE user_id = ?1
                         ORDER BY created_at DESC LIMIT ?2",
                        params![user_id, lim],
                    )
                    .await?
            }
        };
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(BehaviorEventEntry {
                id: row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                user_id: DbUserId::new(
                    row.get::<String>(1)
                        .map_err(|e| StoreError::Db(e.to_string()))?,
                ),
                event_type: row.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                context: row.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                metadata: row.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                created_at: row.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
            });
        }
        Ok(out)
    }

    /// Aggregate event-type counts for `user_id` — used for frequency-based pattern detection.
    ///
    /// Returns `(event_type, count)` pairs sorted by count descending.
    pub async fn get_behavior_summary(
        &self,
        user_id: &str,
    ) -> Result<Vec<(String, i64)>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT event_type, COUNT(*) AS cnt
                 FROM behavior_events WHERE user_id = ?1
                 GROUP BY event_type ORDER BY cnt DESC",
                params![user_id],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let t: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let c: i64 = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            out.push((t, c));
        }
        Ok(out)
    }

    // ── Learned Patterns (learned_patterns) ──────────────────────────────────

    /// Insert a `learned_patterns` row. Returns the inserted `rowid`.
    pub async fn store_learned_pattern(
        &self,
        user_id: &str,
        pattern_type: &str,
        category: &str,
        description: &str,
        confidence: f64,
        vcs_snapshot_id: Option<&str>,
    ) -> Result<i64, StoreError> {
        let user_id = user_id.to_string();
        let pattern_type = pattern_type.to_string();
        let category = category.to_string();
        let description = description.to_string();
        let vcs_snapshot_id = vcs_snapshot_id.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO learned_patterns
                     (user_id, pattern_type, category, description, confidence, vcs_snapshot_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        user_id.as_str(),
                        pattern_type.as_str(),
                        category.as_str(),
                        description.as_str(),
                        confidence,
                        vcs_snapshot_id.as_deref(),
                    ],
                )
                .await?;
                Ok::<_, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    /// Fetch all `learned_patterns` for `user_id`, sorted by confidence descending.
    pub async fn get_learned_patterns(
        &self,
        user_id: &str,
        limit: i64,
    ) -> Result<Vec<LearnedPatternEntry>, StoreError> {
        let lim = limit.clamp(1, 10_000);
        let mut rows = self
            .conn
            .query(
                "SELECT id, user_id, pattern_type, category, description, confidence, vcs_snapshot_id
                 FROM learned_patterns WHERE user_id = ?1
                 ORDER BY confidence DESC LIMIT ?2",
                params![user_id, lim],
            )
            .await?;
        Self::collect_pattern_rows(&mut rows).await
    }

    /// Fetch patterns for `user_id` filtered by `category`.
    pub async fn get_patterns_by_category(
        &self,
        user_id: &str,
        category: &str,
    ) -> Result<Vec<LearnedPatternEntry>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, user_id, pattern_type, category, description, confidence, vcs_snapshot_id
                 FROM learned_patterns WHERE user_id = ?1 AND category = ?2
                 ORDER BY confidence DESC",
                params![user_id, category],
            )
            .await?;
        Self::collect_pattern_rows(&mut rows).await
    }

    /// Update the `confidence` column for a single `learned_patterns` row by `id`.
    pub async fn update_pattern_confidence(
        &self,
        id: i64,
        confidence: f64,
    ) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE learned_patterns SET confidence = ?2 WHERE id = ?1",
                    params![id, confidence],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    // ── Command Analytics ─────────────────────────────────────────────────────

    /// Aggregate CLI command frequency from `behavior_events` where `event_type = 'command'`
    /// and `context` holds the command name.
    ///
    /// Called from `vox-db/src/learning.rs` `BehavioralLearner::frequency_analysis`.
    pub async fn get_command_frequency(
        &self,
        user_id: &str,
        limit: i64,
    ) -> Result<Vec<CommandFrequencyEntry>, StoreError> {
        let lim = limit.clamp(1, 1_000);
        let mut rows = self
            .conn
            .query(
                "SELECT context,
                        COUNT(*) AS total,
                        SUM(CASE WHEN json_extract(metadata, '$.success') = 1 THEN 1 ELSE 0 END),
                        AVG(CAST(json_extract(metadata, '$.duration_ms') AS REAL))
                 FROM behavior_events
                 WHERE user_id = ?1 AND event_type = 'command' AND context IS NOT NULL
                 GROUP BY context ORDER BY total DESC LIMIT ?2",
                params![user_id, lim],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(CommandFrequencyEntry {
                command: row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                count: row.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                success_count: row.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                avg_duration_ms: row.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
            });
        }
        Ok(out)
    }

    // ── User Preferences (user_preferences) ──────────────────────────────────

    /// Upsert a `user_preferences` key-value row.
    ///
    /// Called from `vox-db/src/learning.rs` `BehavioralLearner::preference_inference`.
    pub async fn set_user_preference(
        &self,
        user_id: &str,
        key: &str,
        value: &str,
    ) -> Result<(), StoreError> {
        let user_id = user_id.to_string();
        let key = key.to_string();
        let value = value.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO user_preferences (user_id, key, value, updated_at)
                 VALUES (?1, ?2, ?3, datetime('now'))
                 ON CONFLICT(user_id, key)
                 DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
                    params![user_id.as_str(), key.as_str(), value.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    pub async fn delete_user_preference(&self, user_id: &str, key: &str) -> Result<(), StoreError> {
        let user_id = user_id.to_string();
        let key = key.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "DELETE FROM user_preferences WHERE user_id = ?1 AND key = ?2",
                    params![user_id.as_str(), key.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    // ── Account Config (account_config) ──────────────────────────────────────

    /// Upsert an `account_config` key-value row.
    pub async fn set_account_config(
        &self,
        account_id: &str,
        key: &str,
        value: &str,
    ) -> Result<(), StoreError> {
        let account_id = account_id.to_string();
        let key = key.to_string();
        let value = value.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO account_config (account_id, key, value, updated_at)
                 VALUES (?1, ?2, ?3, datetime('now'))
                 ON CONFLICT(account_id, key)
                 DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
                    params![account_id.as_str(), key.as_str(), value.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Read a single `account_config` value by `(account_id, key)`, or `None` if absent.
    pub async fn get_account_config(
        &self,
        account_id: &str,
        key: &str,
    ) -> Result<Option<String>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT value FROM account_config WHERE account_id = ?1 AND key = ?2 LIMIT 1",
                params![account_id, key],
            )
            .await?;
        match rows.next().await? {
            Some(row) => {
                let v: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                Ok(Some(v))
            }
            None => Ok(None),
        }
    }

    /// Return all `(key, value)` pairs in `account_config` for `account_id`
    pub async fn list_account_configs(
        &self,
        account_id: &str,
        prefix: Option<&str>,
    ) -> Result<Vec<(String, String)>, StoreError> {
        let mut rows = match prefix {
            Some(pfx) => {
                let pattern = format!("{pfx}%");
                self.conn
                    .query(
                        "SELECT key, value FROM account_config
                         WHERE account_id = ?1 AND key LIKE ?2
                         ORDER BY key ASC",
                        params![account_id, pattern],
                    )
                    .await?
            }
            None => {
                self.conn
                    .query(
                        "SELECT key, value FROM account_config
                         WHERE account_id = ?1
                         ORDER BY key ASC",
                        params![account_id],
                    )
                    .await?
            }
        };
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let k: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let v: String = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            out.push((k, v));
        }
        Ok(out)
    }

    /// Read a single `user_preferences` value by `(user_id, key)`, or `None` if absent.
    ///
    /// Called from `vox-mcp/src/memory.rs` preference handler.
    pub async fn get_user_preference(
        &self,
        user_id: &str,
        key: &str,
    ) -> Result<Option<String>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT value FROM user_preferences WHERE user_id = ?1 AND key = ?2 LIMIT 1",
                params![user_id, key],
            )
            .await?;
        match rows.next().await? {
            Some(row) => {
                let v: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                Ok(Some(v))
            }
            None => Ok(None),
        }
    }

    /// Return all `(key, value)` pairs in `user_preferences` for `user_id`, optionally filtered by
    /// `prefix` (matched as `key LIKE 'prefix%'`).
    ///
    /// Called from `vox-mcp/src/memory.rs` preference list handler.
    pub async fn list_user_preferences(
        &self,
        user_id: &str,
        prefix: Option<&str>,
    ) -> Result<Vec<(String, String)>, StoreError> {
        let mut rows = match prefix {
            Some(pfx) => {
                let pattern = format!("{pfx}%");
                self.conn
                    .query(
                        "SELECT key, value FROM user_preferences
                         WHERE user_id = ?1 AND key LIKE ?2
                         ORDER BY key ASC",
                        params![user_id, pattern],
                    )
                    .await?
            }
            None => {
                self.conn
                    .query(
                        "SELECT key, value FROM user_preferences
                         WHERE user_id = ?1
                         ORDER BY key ASC",
                        params![user_id],
                    )
                    .await?
            }
        };
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let k: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let v: String = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            out.push((k, v));
        }
        Ok(out)
    }

    /// Join `llm_interactions` with `llm_feedback` to produce RLHF training pairs.
    ///
    /// Called from `vox-db/src/learning.rs` and `vox-package/src/feedback.rs`.
    pub async fn get_training_data(&self, limit: i64) -> Result<Vec<TrainingPair>, StoreError> {
        let lim = limit.clamp(1, 50_000);
        let mut rows = self
            .conn
            .query(
                "SELECT i.prompt, i.response, f.rating, f.correction_text, f.feedback_type
                 FROM llm_interactions i
                 LEFT JOIN llm_feedback f ON f.interaction_id = i.id
                 ORDER BY i.id DESC LIMIT ?1",
                params![lim],
            )
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(TrainingPair {
                prompt: row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                response: row.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                rating: row.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                correction: row.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                feedback_type: row
                    .get::<Option<String>>(4)
                    .map_err(|e| StoreError::Db(e.to_string()))?
                    .unwrap_or_else(|| "none".to_string()),
            });
        }
        Ok(out)
    }

    // ── Snippets (snippets) ───────────────────────────────────────────────────

    /// Insert a row into `snippets`. Returns the inserted `rowid`.
    ///
    /// Called from `vox-cli/src/commands/extras/snippet/mod.rs`.
    pub async fn save_snippet(&self, p: SaveSnippetParams<'_>) -> Result<i64, StoreError> {
        let language = p.language.to_string();
        let title = p.title.to_string();
        let code = p.code.to_string();
        let description = p.description.map(str::to_string);
        let tags = p.tags.map(str::to_string);
        let author_id = p.author_id.map(str::to_string);
        let source_ref = p.source_ref.map(str::to_string);
        let embedding_ref = p.embedding_ref.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO snippets (language, title, code, description, tags, author_id, source_ref, embedding_ref)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        language.as_str(),
                        title.as_str(),
                        code.as_str(),
                        description.as_deref(),
                        tags.as_deref(),
                        author_id.as_deref(),
                        source_ref.as_deref(),
                        embedding_ref.as_deref(),
                    ],
                )
                .await?;
                Ok::<_, StoreError>(conn.last_insert_rowid())
            })
            .await
    }

    /// Search `snippets` by keyword with an optional tag filter.
    ///
    /// `query` is matched as `LIKE '%query%'` against `title`, `code`, and `description`.
    /// `tag_filter` further narrows by `tags LIKE '%tag_filter%'`.
    /// Returns at most 500 rows, newest first.
    ///
    /// Called from `vox-cli/src/commands/extras/snippet/mod.rs`.
    pub async fn search_snippets(
        &self,
        query: &str,
        tag_filter: Option<&str>,
    ) -> Result<Vec<SnippetEntry>, StoreError> {
        let pattern = if query == "%" || query.is_empty() {
            "%".to_string()
        } else {
            format!("%{query}%")
        };
        let mut rows = match tag_filter {
            Some(tag) => {
                let tag_pat = format!("%{tag}%");
                self.conn
                    .query(
                        "SELECT id, language, title, code, description, tags
                         FROM snippets
                         WHERE (title LIKE ?1 OR code LIKE ?1 OR description LIKE ?1)
                           AND tags LIKE ?2
                         ORDER BY id DESC LIMIT 500",
                        params![pattern, tag_pat],
                    )
                    .await?
            }
            None => {
                self.conn
                    .query(
                        "SELECT id, language, title, code, description, tags
                         FROM snippets
                         WHERE title LIKE ?1 OR code LIKE ?1 OR description LIKE ?1
                         ORDER BY id DESC LIMIT 500",
                        params![pattern],
                    )
                    .await?
            }
        };
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(SnippetEntry {
                id: row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                language: row.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                title: row.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                code: row.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                description: row.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                tags: row.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
            });
        }
        Ok(out)
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    async fn collect_pattern_rows(
        rows: &mut crate::GuardedRows,
    ) -> Result<Vec<LearnedPatternEntry>, StoreError> {
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(LearnedPatternEntry {
                id: row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                user_id: DbUserId::new(
                    row.get::<String>(1)
                        .map_err(|e| StoreError::Db(e.to_string()))?,
                ),
                pattern_type: row.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                category: row.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                description: row.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                confidence: row.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
                vcs_snapshot_id: row.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
            });
        }
        Ok(out)
    }
}
