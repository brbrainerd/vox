use std::collections::{BTreeMap, BTreeSet};

use vox_ast::decl::{CollectionDecl, IndexDecl, TableDecl};

use crate::ddl::{add_column_ddl, collection_to_ddl, index_to_ddl, live_table_name, table_to_ddl};
use crate::schema_model::IntrospectedTable;
use crate::type_map::UnsupportedTypePolicy;
use crate::{AnySqlBackend, SqlBackendError};

#[derive(Debug, Clone)]
pub enum MigrationAction {
    CreateTable {
        sql: String,
    },
    AddColumn {
        table: String,
        column: String,
        sql: String,
    },
    CreateIndex {
        sql: String,
    },
    CreateCollection {
        sql: String,
    },
    ManualDropColumn {
        table: String,
        column: String,
    },
    ManualDropTable {
        table: String,
    },
}

impl MigrationAction {
    #[must_use]
    pub fn is_auto_safe(&self) -> bool {
        matches!(
            self,
            Self::CreateTable { .. }
                | Self::AddColumn { .. }
                | Self::CreateIndex { .. }
                | Self::CreateCollection { .. }
        )
    }

    #[must_use]
    pub fn to_sql(&self) -> Option<String> {
        match self {
            Self::CreateTable { sql }
            | Self::CreateIndex { sql }
            | Self::CreateCollection { sql } => Some(sql.clone()),
            Self::AddColumn { sql, .. } => Some(sql.clone()),
            Self::ManualDropColumn { .. } | Self::ManualDropTable { .. } => None,
        }
    }

    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::CreateTable { .. } => "create new table".to_string(),
            Self::AddColumn { table, column, .. } => {
                format!("add column `{column}` to `{table}`")
            }
            Self::CreateIndex { .. } => "create index".to_string(),
            Self::CreateCollection { .. } => "create collection".to_string(),
            Self::ManualDropColumn { table, column } => {
                format!("manual review: drop column `{column}` from `{table}`")
            }
            Self::ManualDropTable { table } => {
                format!("manual review: drop table `{table}`")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct MigrationPlan {
    pub actions: Vec<MigrationAction>,
}

impl MigrationPlan {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    #[must_use]
    pub fn auto_actions(&self) -> Vec<&MigrationAction> {
        self.actions.iter().filter(|a| a.is_auto_safe()).collect()
    }

    #[must_use]
    pub fn describe(&self) -> String {
        if self.actions.is_empty() {
            return "Schema is up to date.".to_string();
        }
        self.actions
            .iter()
            .map(|a| {
                if a.is_auto_safe() {
                    format!("  ✓ {}", a.describe())
                } else {
                    format!("  ⚠ {}", a.describe())
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub struct AppAutoMigrator<'a> {
    backend: &'a AnySqlBackend,
}

impl<'a> AppAutoMigrator<'a> {
    #[must_use]
    pub fn new(backend: &'a AnySqlBackend) -> Self {
        Self { backend }
    }

    pub async fn plan(
        &self,
        tables: &[&TableDecl],
        collections: &[&CollectionDecl],
        indexes: &[&IndexDecl],
        policy: UnsupportedTypePolicy,
    ) -> Result<MigrationPlan, SqlBackendError> {
        let live = self.backend.introspect_schema().await?;
        let live_table_map: BTreeMap<String, &IntrospectedTable> =
            live.tables.iter().map(|t| (t.name.clone(), t)).collect();

        let mut actions = Vec::new();
        let backend_kind = self.backend.backend_kind();

        for table in tables {
            let table_name = live_table_name(table);
            if !live_table_map.contains_key(&table_name) {
                actions.push(MigrationAction::CreateTable {
                    sql: table_to_ddl(backend_kind, table, policy)?,
                });
                continue;
            }
            let live_cols: BTreeSet<&str> = live_table_map
                .get(&table_name)
                .map(|t| t.columns.iter().map(|c| c.name.as_str()).collect())
                .unwrap_or_default();
            for f in &table.fields {
                if !live_cols.contains(f.name.as_str()) {
                    actions.push(MigrationAction::AddColumn {
                        table: table_name.clone(),
                        column: f.name.clone(),
                        sql: add_column_ddl(
                            backend_kind,
                            &table_name,
                            &f.name,
                            &f.type_ann,
                            policy,
                        )?,
                    });
                }
            }

            if let Some(live_table) = live_table_map.get(&table_name) {
                let desired_cols: BTreeSet<&str> =
                    table.fields.iter().map(|f| f.name.as_str()).collect();
                for c in &live_table.columns {
                    if c.name != "_id" && !desired_cols.contains(c.name.as_str()) {
                        actions.push(MigrationAction::ManualDropColumn {
                            table: table_name.clone(),
                            column: c.name.clone(),
                        });
                    }
                }
            }
        }

        for collection in collections {
            let name = crate::ddl::to_snake_case(&collection.name);
            if !live_table_map.contains_key(&name) {
                actions.push(MigrationAction::CreateCollection {
                    sql: collection_to_ddl(backend_kind, collection)?,
                });
            }
        }

        for idx in indexes {
            actions.push(MigrationAction::CreateIndex {
                sql: index_to_ddl(backend_kind, idx),
            });
        }

        let desired_tables: BTreeSet<String> = tables
            .iter()
            .map(|t| live_table_name(t))
            .chain(
                collections
                    .iter()
                    .map(|c| crate::ddl::to_snake_case(&c.name)),
            )
            .collect();
        for lt in live_table_map.keys() {
            if !desired_tables.contains(lt) {
                actions.push(MigrationAction::ManualDropTable { table: lt.clone() });
            }
        }

        Ok(MigrationPlan { actions })
    }

    pub async fn apply(&self, plan: &MigrationPlan) -> Result<usize, SqlBackendError> {
        let mut applied = 0usize;
        for action in plan.auto_actions() {
            if let Some(sql) = action.to_sql() {
                let _ = self.backend.execute(&sql, &[]).await?;
                applied += 1;
            }
        }
        Ok(applied)
    }

    pub async fn sync_schema(
        &self,
        tables: &[&TableDecl],
        collections: &[&CollectionDecl],
        indexes: &[&IndexDecl],
        policy: UnsupportedTypePolicy,
    ) -> Result<MigrationPlan, SqlBackendError> {
        let plan = self.plan(tables, collections, indexes, policy).await?;
        let _ = self.apply(&plan).await?;
        Ok(plan)
    }
}
