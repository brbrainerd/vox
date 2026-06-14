/// Adversarial semantic-coverage tests — Wave 18 — vox-db pure functions.
///
/// Module: semcov_wave18_tests
/// Targets: sql_util::validate_identifier, migration::validate_migrations,
///          ddl::emit::{to_snake_case, vox_type_to_sqlite_type, table_to_ddl,
///                      index_to_ddl, collection_index_to_ddl, table_info_to_ddl},
///          ddl::diff::{diff_schemas, diff_to_sql, describe_diff},
///          normalize::normalize_and_hash
///
/// Every test carries a `// Catches:` annotation describing the specific bug it guards against.
#[cfg(test)]
mod semcov_wave18_tests {
    // ── imports ──────────────────────────────────────────────────────────────
    use crate::ddl::{
        collection_index_to_ddl, index_to_ddl, table_info_to_ddl, to_snake_case,
        vox_type_to_sqlite_type,
    };
    use crate::ddl::{describe_diff, diff_schemas, diff_to_sql};
    use crate::migration::{Migration, validate_migrations};
    use crate::normalize::normalize_and_hash;
    use crate::schema_digest::{FieldInfo, TableInfo};
    use crate::sql_util::validate_identifier;
    use vox_ast::decl::{IndexDecl, TableDecl, TableField};
    use vox_ast::span::Span;
    use vox_ast::types::TypeExpr;

    // ── helpers ───────────────────────────────────────────────────────────────

    fn sp() -> Span {
        Span { start: 0, end: 0 }
    }

    fn named_field(name: &str, ty: &str) -> TableField {
        TableField {
            name: name.to_string(),
            type_ann: TypeExpr::Named {
                name: ty.to_string(),
                span: sp(),
            },
            description: None,
            span: sp(),
        }
    }

    fn opt_field(name: &str, inner: &str) -> TableField {
        TableField {
            name: name.to_string(),
            type_ann: TypeExpr::Generic {
                name: "Option".to_string(),
                args: vec![TypeExpr::Named {
                    name: inner.to_string(),
                    span: sp(),
                }],
                span: sp(),
            },
            description: None,
            span: sp(),
        }
    }

    fn bare_table(name: &str, fields: Vec<TableField>) -> TableDecl {
        TableDecl {
            name: name.to_string(),
            fields,
            primary_key: None,
            description: None,
            json_layout: None,
            auth_provider: None,
            roles: Vec::new(),
            cors: None,
            is_pub: false,
            is_deprecated: false,
            is_extern: false,
            source: None,
            span: sp(),
        }
    }

    fn idx(table: &str, name: &str, cols: &[&str]) -> IndexDecl {
        IndexDecl {
            table_name: table.to_string(),
            index_name: name.to_string(),
            columns: cols.iter().map(|s| s.to_string()).collect(),
            span: sp(),
        }
    }

    fn field_info(name: &str, type_str: &str, is_optional: bool) -> FieldInfo {
        FieldInfo {
            name: name.to_string(),
            type_str: type_str.to_string(),
            is_optional,
            references_table: None,
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    //  sql_util::validate_identifier
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn identifier_empty_is_rejected() {
        // Catches: caller passes "" as column name → unquoted empty identifier produces broken SQL.
        assert!(validate_identifier("").is_err());
    }

    #[test]
    fn identifier_exactly_64_bytes_is_accepted() {
        // Catches: off-by-one in the ≤64 guard lets a 64-char name slip through as error.
        let name = "a".repeat(64);
        assert!(
            validate_identifier(&name).is_ok(),
            "64-byte identifier must be accepted"
        );
    }

    #[test]
    fn identifier_65_bytes_is_rejected() {
        // Catches: off-by-one in the >64 guard allows overlong identifiers into SQL.
        let name = "a".repeat(65);
        assert!(validate_identifier(&name).is_err());
    }

    #[test]
    fn identifier_leading_digit_is_rejected() {
        // Catches: "1col" accepted → SQLite parses it as numeric literal, not identifier.
        assert!(validate_identifier("1col").is_err());
    }

    #[test]
    fn identifier_sql_keyword_with_space_is_rejected() {
        // Catches: "drop table" accepted → enables classic SQL injection via column name.
        assert!(validate_identifier("drop table").is_err());
    }

    #[test]
    fn identifier_semicolon_is_rejected() {
        // Catches: "col; DROP TABLE users--" accepted → DDL statement injection.
        assert!(validate_identifier("col; DROP TABLE users--").is_err());
    }

    #[test]
    fn identifier_hyphen_is_rejected() {
        // Catches: "my-col" accepted → parsed as subtraction in SQL, not an identifier.
        assert!(validate_identifier("my-col").is_err());
    }

    #[test]
    fn identifier_dot_is_rejected() {
        // Catches: "schema.table" accepted → two-part name bypasses single-identifier validation.
        assert!(validate_identifier("schema.table").is_err());
    }

    #[test]
    fn identifier_underscore_start_is_accepted() {
        // Catches: underscore-prefixed identifiers (common for internal columns like _id) rejected by overly strict guard.
        assert!(validate_identifier("_private").is_ok());
    }

    #[test]
    fn identifier_single_letter_is_accepted() {
        // Catches: minimum-length acceptance boundary regression.
        assert!(validate_identifier("x").is_ok());
    }

    #[test]
    fn identifier_returns_same_str_on_ok() {
        // Catches: validate_identifier returns a different/modified string instead of the original slice.
        let s = "valid_name";
        assert_eq!(validate_identifier(s).unwrap(), s);
    }

    // ══════════════════════════════════════════════════════════════════════════
    //  migration::validate_migrations
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn migrations_empty_slice_is_ok() {
        // Catches: vacuous-truth regression — empty migrations panic or return Err.
        assert!(validate_migrations(&[]).is_ok());
    }

    #[test]
    fn migrations_version_zero_is_rejected() {
        // Catches: version 0 treated as valid → collides with the "last=0" sentinel in the validator.
        let m = vec![Migration::new(0, "zero", "SELECT 1;")];
        assert!(validate_migrations(&m).is_err());
    }

    #[test]
    fn migrations_negative_version_is_rejected() {
        // Catches: negative version accepted → −1 passes the >0 guard if the guard uses >= instead.
        let m = vec![Migration::new(-1, "neg", "SELECT 1;")];
        assert!(validate_migrations(&m).is_err());
    }

    #[test]
    fn migrations_duplicate_version_is_rejected() {
        // Catches: two migrations at the same version applied twice → double-migration data corruption.
        let m = vec![
            Migration::new(1, "first", "CREATE TABLE a(id INTEGER);"),
            Migration::new(1, "again", "CREATE TABLE b(id INTEGER);"),
        ];
        assert!(validate_migrations(&m).is_err());
    }

    #[test]
    fn migrations_out_of_order_is_rejected() {
        // Catches: descending version list accepted → older migration runs after newer, destroying schema.
        let m = vec![
            Migration::new(2, "two", "CREATE TABLE b(id INTEGER);"),
            Migration::new(1, "one", "CREATE TABLE a(id INTEGER);"),
        ];
        assert!(validate_migrations(&m).is_err());
    }

    #[test]
    fn migrations_same_version_repeated_is_not_idempotent() {
        // Catches: BTreeSet dedup masking a duplicate that slips past the strictly-increasing guard.
        let m = vec![
            Migration::new(5, "five_a", "CREATE TABLE x(id INTEGER);"),
            Migration::new(5, "five_b", "CREATE TABLE y(id INTEGER);"),
        ];
        let result = validate_migrations(&m);
        assert!(
            result.is_err(),
            "duplicate version must be rejected, not silently deduplicated"
        );
    }

    #[test]
    fn migrations_large_gap_is_ok() {
        // Catches: validator incorrectly requires consecutive versions (no gap allowed).
        let m = vec![
            Migration::new(1, "one", "SELECT 1;"),
            Migration::new(1_000_000, "million", "SELECT 2;"),
        ];
        assert!(validate_migrations(&m).is_ok());
    }

    #[test]
    fn migrations_single_entry_is_ok() {
        // Catches: single-element slice triggers off-by-one in loop-based comparison.
        let m = vec![Migration::new(1, "only", "SELECT 1;")];
        assert!(validate_migrations(&m).is_ok());
    }

    #[test]
    fn migration_new_stores_fields_verbatim() {
        // Catches: Migration::new truncates or transforms version/name/sql before storage.
        let sql = "CREATE TABLE z(_id INTEGER PRIMARY KEY);";
        let m = Migration::new(42, "create_z", sql);
        assert_eq!(m.version, 42);
        assert_eq!(m.name, "create_z");
        assert_eq!(m.up_sql, sql);
    }

    // ══════════════════════════════════════════════════════════════════════════
    //  ddl::emit::to_snake_case
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn snake_case_already_lowercase_unchanged() {
        // Catches: to_snake_case inserts leading underscore for a lowercase input.
        assert_eq!(to_snake_case("task"), "task");
    }

    #[test]
    fn snake_case_empty_string_is_empty() {
        // Catches: to_snake_case on "" panics due to enumerate/unwrap.
        assert_eq!(to_snake_case(""), "");
    }

    #[test]
    fn snake_case_single_uppercase_letter_has_no_leading_underscore() {
        // Catches: "A" → "_a" because the i>0 guard fires on a non-zero char index somehow.
        assert_eq!(to_snake_case("A"), "a");
    }

    #[test]
    fn snake_case_consecutive_uppercase_inserts_separator_per_char() {
        // Catches: "AB" → "ab" (run collapsed) instead of the documented "a_b" per-char expansion.
        // The existing test documents HTTPRoute → "h_t_t_p_route", so this validates the invariant.
        let result = to_snake_case("AB");
        assert_eq!(result, "a_b");
    }

    #[test]
    fn snake_case_pascal_case_two_words() {
        // Catches: "UserProfile" → "user profile" (space) or "userprofile" (no separator).
        assert_eq!(to_snake_case("UserProfile"), "user_profile");
    }

    // ══════════════════════════════════════════════════════════════════════════
    //  ddl::emit::vox_type_to_sqlite_type
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn vox_type_int_maps_to_integer() {
        // Catches: "int" → "TEXT" (fallthrough to default branch).
        assert_eq!(vox_type_to_sqlite_type("int"), "INTEGER");
    }

    #[test]
    fn vox_type_str_maps_to_text() {
        // Catches: "str" → "BLOB" or some other affinity mismatch.
        assert_eq!(vox_type_to_sqlite_type("str"), "TEXT");
    }

    #[test]
    fn vox_type_float_maps_to_real() {
        // Catches: "float" → "INTEGER" due to wrong branch ordering.
        assert_eq!(vox_type_to_sqlite_type("float"), "REAL");
    }

    #[test]
    fn vox_type_bool_maps_to_integer() {
        // Catches: "bool" → "TEXT" (not mapped through VoxScalar).
        assert_eq!(vox_type_to_sqlite_type("bool"), "INTEGER");
    }

    #[test]
    fn vox_type_bytes_maps_to_blob() {
        // Catches: "bytes" → "TEXT" (alias not handled separately from VoxScalar path).
        assert_eq!(vox_type_to_sqlite_type("bytes"), "BLOB");
    }

    #[test]
    fn vox_type_option_int_strips_wrapper() {
        // Catches: "Option[int]" → "TEXT" because Option stripping branch uses wrong prefix.
        assert_eq!(vox_type_to_sqlite_type("Option[int]"), "INTEGER");
    }

    #[test]
    fn vox_type_nested_option_strips_both_layers() {
        // Catches: double-nested Option[Option[str]] → "TEXT" from outer, inner never consulted.
        assert_eq!(vox_type_to_sqlite_type("Option[Option[str]]"), "TEXT");
    }

    #[test]
    fn vox_type_list_maps_to_text() {
        // Catches: "List[int]" → "INTEGER" (inner type leaks through).
        assert_eq!(vox_type_to_sqlite_type("List[int]"), "TEXT");
    }

    #[test]
    fn vox_type_id_reference_maps_to_text() {
        // Catches: "Id[User]" → some non-TEXT affinity, breaking FK storage as UUID.
        assert_eq!(vox_type_to_sqlite_type("Id[User]"), "TEXT");
    }

    #[test]
    fn vox_type_unknown_maps_to_text() {
        // Catches: unknown type string panics or returns non-TEXT affinity.
        assert_eq!(vox_type_to_sqlite_type("SomeCustomType"), "TEXT");
    }

    // ══════════════════════════════════════════════════════════════════════════
    //  ddl::emit::index_to_ddl  (SQL string generation)
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn index_ddl_contains_if_not_exists() {
        // Catches: generated index DDL lacks IF NOT EXISTS → re-migration crashes on existing DB.
        let i = idx("Task", "by_status", &["status"]);
        let ddl = index_to_ddl(&i);
        assert!(
            ddl.contains("IF NOT EXISTS"),
            "missing IF NOT EXISTS: {ddl}"
        );
    }

    #[test]
    fn index_ddl_table_name_is_snake_cased() {
        // Catches: PascalCase table name written verbatim → mismatch with CREATE TABLE snake_case name.
        let i = idx("UserProfile", "by_email", &["email"]);
        let ddl = index_to_ddl(&i);
        assert!(
            ddl.contains("ON user_profile"),
            "table name not snake-cased in index DDL: {ddl}"
        );
    }

    #[test]
    fn index_ddl_index_name_preserved_verbatim() {
        // Catches: index name lowercased/modified, breaking name lookup in diff_to_sql.
        let i = idx("Task", "byDone", &["done"]);
        let ddl = index_to_ddl(&i);
        assert!(ddl.contains("idx_task_byDone"), "index name mangled: {ddl}");
    }

    #[test]
    fn index_ddl_multi_column_joined_with_comma() {
        // Catches: multi-column index columns joined without comma → invalid SQL.
        let i = idx("Task", "by_prio_done", &["priority", "done"]);
        let ddl = index_to_ddl(&i);
        assert!(
            ddl.contains("(priority, done)"),
            "multi-column index columns not comma-joined: {ddl}"
        );
    }

    #[test]
    fn collection_index_uses_json_extract() {
        // Catches: collection index references plain column instead of json_extract(_data, '$.field').
        let i = idx("Events", "by_name", &["name"]);
        let ddl = collection_index_to_ddl(&i);
        assert!(
            ddl.contains("json_extract(_data, '$.name')"),
            "collection index not using json_extract: {ddl}"
        );
    }

    #[test]
    fn collection_index_multi_column_all_use_json_extract() {
        // Catches: only first column wrapped in json_extract, subsequent columns left bare.
        let i = idx("Events", "by_type_ts", &["event_type", "timestamp"]);
        let ddl = collection_index_to_ddl(&i);
        assert!(
            ddl.contains("json_extract(_data, '$.event_type')")
                && ddl.contains("json_extract(_data, '$.timestamp')"),
            "not all columns use json_extract: {ddl}"
        );
    }

    // ══════════════════════════════════════════════════════════════════════════
    //  ddl::emit::table_info_to_ddl  (schema_digest path)
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn table_info_ddl_required_field_has_not_null() {
        // Catches: required field emitted as nullable → data integrity constraint dropped.
        let info = TableInfo {
            name: "Project".to_string(),
            fields: vec![field_info("title", "str", false)],
            description: None,
            example_insert: String::new(),
            example_query: String::new(),
            is_public: false,
            auth_provider: None,
            sample_data: vec![],
        };
        let ddl = table_info_to_ddl(&info);
        assert!(
            ddl.contains("title TEXT NOT NULL"),
            "required field missing NOT NULL: {ddl}"
        );
    }

    #[test]
    fn table_info_ddl_optional_field_has_no_not_null() {
        // Catches: optional field emitted with NOT NULL → inserting NULL fails at runtime.
        let info = TableInfo {
            name: "Project".to_string(),
            fields: vec![field_info("notes", "str", true)],
            description: None,
            example_insert: String::new(),
            example_query: String::new(),
            is_public: false,
            auth_provider: None,
            sample_data: vec![],
        };
        let ddl = table_info_to_ddl(&info);
        // "notes TEXT" should appear but not "notes TEXT NOT NULL"
        assert!(
            !ddl.contains("notes TEXT NOT NULL"),
            "optional field must not have NOT NULL: {ddl}"
        );
        assert!(
            ddl.contains("notes TEXT"),
            "optional field column missing: {ddl}"
        );
    }

    #[test]
    fn table_info_ddl_has_id_and_creation_time() {
        // Catches: schema_digest path forgets sentinel columns present in table_to_ddl AST path, causing schema divergence.
        let info = TableInfo {
            name: "Run".to_string(),
            fields: vec![],
            description: None,
            example_insert: String::new(),
            example_query: String::new(),
            is_public: false,
            auth_provider: None,
            sample_data: vec![],
        };
        let ddl = table_info_to_ddl(&info);
        assert!(
            ddl.contains("_id TEXT PRIMARY KEY NOT NULL"),
            "missing _id: {ddl}"
        );
        assert!(
            ddl.contains("_creationTime TEXT NOT NULL"),
            "missing _creationTime: {ddl}"
        );
    }

    #[test]
    fn table_info_ddl_contains_if_not_exists() {
        // Catches: table_info_to_ddl omits IF NOT EXISTS → re-migration crashes on existing table.
        let info = TableInfo {
            name: "Widget".to_string(),
            fields: vec![],
            description: None,
            example_insert: String::new(),
            example_query: String::new(),
            is_public: false,
            auth_provider: None,
            sample_data: vec![],
        };
        let ddl = table_info_to_ddl(&info);
        assert!(
            ddl.contains("IF NOT EXISTS"),
            "missing IF NOT EXISTS: {ddl}"
        );
    }

    #[test]
    fn table_info_ddl_name_is_snake_cased() {
        // Catches: PascalCase table name used verbatim in CREATE TABLE, diverging from AST path.
        let info = TableInfo {
            name: "TrainingRun".to_string(),
            fields: vec![],
            description: None,
            example_insert: String::new(),
            example_query: String::new(),
            is_public: false,
            auth_provider: None,
            sample_data: vec![],
        };
        let ddl = table_info_to_ddl(&info);
        assert!(
            ddl.contains("CREATE TABLE IF NOT EXISTS training_run"),
            "table name not snake_cased: {ddl}"
        );
    }

    // ══════════════════════════════════════════════════════════════════════════
    //  ddl::diff::{diff_schemas, diff_to_sql, describe_diff}
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn diff_empty_to_empty_has_no_changes() {
        // Catches: diff_schemas on two empty schemas produces spurious added/removed entries.
        let diff = diff_schemas(&[], &[], &[], &[], &[], &[]);
        assert!(diff.added_tables.is_empty());
        assert!(diff.removed_tables.is_empty());
        assert!(diff.added_columns.is_empty());
        assert!(diff.removed_columns.is_empty());
        assert!(diff.added_indexes.is_empty());
        assert!(diff.removed_indexes.is_empty());
    }

    #[test]
    fn diff_added_table_detected() {
        // Catches: new table not recognized as added when old schema is empty.
        let new_t = bare_table("Widget", vec![named_field("color", "str")]);
        let diff = diff_schemas(&[], &[&new_t], &[], &[], &[], &[]);
        assert!(
            diff.added_tables.contains(&"Widget".to_string()),
            "added table not detected: {:?}",
            diff.added_tables
        );
    }

    #[test]
    fn diff_removed_table_detected() {
        // Catches: removed table not recognized → no warning SQL generated, silent data loss.
        let old_t = bare_table("Legacy", vec![named_field("val", "str")]);
        let diff = diff_schemas(&[&old_t], &[], &[], &[], &[], &[]);
        assert!(
            diff.removed_tables.contains(&"Legacy".to_string()),
            "removed table not detected: {:?}",
            diff.removed_tables
        );
    }

    #[test]
    fn diff_added_column_detected() {
        // Catches: added column not detected when a shared table gains a field.
        let old_t = bare_table("Task", vec![named_field("title", "str")]);
        let new_t = bare_table(
            "Task",
            vec![named_field("title", "str"), named_field("done", "bool")],
        );
        let diff = diff_schemas(&[&old_t], &[&new_t], &[], &[], &[], &[]);
        assert_eq!(
            diff.added_columns.len(),
            1,
            "expected 1 added column: {:?}",
            diff.added_columns
        );
        assert_eq!(diff.added_columns[0].1, "done");
    }

    #[test]
    fn diff_removed_column_detected() {
        // Catches: removed column not detected → no warning comment emitted, schema drift silent.
        let old_t = bare_table(
            "Task",
            vec![named_field("title", "str"), named_field("old_field", "str")],
        );
        let new_t = bare_table("Task", vec![named_field("title", "str")]);
        let diff = diff_schemas(&[&old_t], &[&new_t], &[], &[], &[], &[]);
        assert_eq!(
            diff.removed_columns.len(),
            1,
            "expected 1 removed column: {:?}",
            diff.removed_columns
        );
        assert_eq!(diff.removed_columns[0].1, "old_field");
    }

    #[test]
    fn diff_to_sql_add_column_uses_alter_table() {
        // Catches: added column emitted as CREATE TABLE instead of ALTER TABLE ADD COLUMN.
        let old_t = bare_table("Task", vec![named_field("title", "str")]);
        let new_t = bare_table(
            "Task",
            vec![named_field("title", "str"), named_field("score", "int")],
        );
        let diff = diff_schemas(&[&old_t], &[&new_t], &[], &[], &[], &[]);
        let sql = diff_to_sql(&diff, &[&new_t], &[]);
        let alter = sql.iter().find(|s| s.contains("ALTER TABLE"));
        assert!(
            alter.is_some(),
            "no ALTER TABLE statement generated: {sql:?}"
        );
        let alter = alter.unwrap();
        assert!(
            alter.contains("ADD COLUMN score"),
            "ALTER TABLE missing ADD COLUMN: {alter}"
        );
    }

    #[test]
    fn diff_to_sql_removed_table_is_comment_not_drop() {
        // Catches: removed table generates DROP TABLE → destructive migration without user confirmation.
        let old_t = bare_table("Deprecated", vec![named_field("x", "str")]);
        let diff = diff_schemas(&[&old_t], &[], &[], &[], &[], &[]);
        let sql = diff_to_sql(&diff, &[], &[]);
        for stmt in &sql {
            assert!(
                !stmt.to_uppercase().starts_with("DROP TABLE"),
                "unsafe DROP TABLE generated for removed table: {stmt}"
            );
        }
        // Must generate a WARNING comment instead
        let has_warning = sql
            .iter()
            .any(|s| s.contains("WARNING") && s.contains("Deprecated"));
        assert!(has_warning, "no WARNING comment for removed table: {sql:?}");
    }

    #[test]
    fn diff_to_sql_added_index_generates_create_index() {
        // Catches: added index silently dropped in diff_to_sql (only tables/columns handled).
        let new_i = idx("Task", "by_score", &["score"]);
        let diff = diff_schemas(&[], &[], &[], &[], &[], &[&new_i]);
        let new_t = bare_table("Task", vec![]);
        let sql = diff_to_sql(&diff, &[&new_t], &[&new_i]);
        let has_create = sql.iter().any(|s| s.contains("CREATE INDEX"));
        assert!(has_create, "no CREATE INDEX for added index: {sql:?}");
    }

    #[test]
    fn diff_to_sql_removed_index_generates_drop_index() {
        // Catches: removed index not generating DROP INDEX IF EXISTS → stale index never cleaned up.
        let old_i = idx("Task", "by_old", &["old_col"]);
        let diff = diff_schemas(&[], &[], &[], &[], &[&old_i], &[]);
        let sql = diff_to_sql(&diff, &[], &[]);
        let has_drop = sql.iter().any(|s| s.contains("DROP INDEX IF EXISTS"));
        assert!(
            has_drop,
            "no DROP INDEX IF EXISTS for removed index: {sql:?}"
        );
    }

    #[test]
    fn describe_diff_no_changes_returns_sentinel() {
        // Catches: describe_diff returns empty string instead of the documented "No schema changes detected." sentinel.
        let diff = diff_schemas(&[], &[], &[], &[], &[], &[]);
        let desc = describe_diff(&diff);
        assert_eq!(desc, "No schema changes detected.");
    }

    #[test]
    fn describe_diff_mentions_added_table_name() {
        // Catches: describe_diff mentions "Added table(s)" but omits the actual table name.
        let new_t = bare_table("Gadget", vec![]);
        let diff = diff_schemas(&[], &[&new_t], &[], &[], &[], &[]);
        let desc = describe_diff(&diff);
        assert!(
            desc.contains("Gadget"),
            "table name missing from describe_diff output: {desc}"
        );
    }

    #[test]
    fn describe_diff_ordering_add_before_remove() {
        // Catches: describe_diff emits removals before additions, confusing human reviewers who rely on stable ordering.
        let old_t = bare_table("Old", vec![]);
        let new_t = bare_table("New", vec![]);
        let diff = diff_schemas(&[&old_t], &[&new_t], &[], &[], &[], &[]);
        let desc = describe_diff(&diff);
        let add_pos = desc.find("Added");
        let rem_pos = desc.find("Removed");
        if let (Some(a), Some(r)) = (add_pos, rem_pos) {
            assert!(
                a < r,
                "Removed appears before Added in describe_diff output:\n{desc}"
            );
        }
        // If either is absent the test passes vacuously (content is fine).
    }

    // ══════════════════════════════════════════════════════════════════════════
    //  normalize::normalize_and_hash
    // ══════════════════════════════════════════════════════════════════════════

    #[test]
    fn normalize_same_content_same_hash() {
        // Catches: non-deterministic hashing (e.g. HashMap key ordering) produces different digests for identical input.
        let h1 = normalize_and_hash("let x = 1");
        let h2 = normalize_and_hash("let x = 1");
        assert_eq!(h1, h2);
    }

    #[test]
    fn normalize_different_content_different_hash() {
        // Catches: hash function collision for trivially distinct inputs → cache false-positive.
        let h1 = normalize_and_hash("let x = 1");
        let h2 = normalize_and_hash("let x = 2");
        assert_ne!(h1, h2);
    }

    #[test]
    fn normalize_comment_stripped_hash_equals_plain() {
        // Catches: inline comment not stripped → cache miss when comment changes but logic doesn't.
        let with_comment = normalize_and_hash("let x = 5 # trailing comment");
        let without = normalize_and_hash("let x = 5");
        assert_eq!(with_comment, without);
    }

    #[test]
    fn normalize_comment_only_line_stripped() {
        // Catches: comment-only lines preserved → different hash from equivalent code without the comment line.
        let with_header = normalize_and_hash("# file header\nlet x = 5");
        let without = normalize_and_hash("let x = 5");
        assert_eq!(with_header, without);
    }

    #[test]
    fn normalize_empty_string_does_not_panic() {
        // Catches: normalize_and_hash("") panics on empty-line iteration or content_hash("").
        let _ = normalize_and_hash("");
    }

    #[test]
    fn normalize_only_comments_same_as_empty() {
        // Catches: a file of only comments hashes differently from empty string → spurious cache invalidation.
        let all_comments = normalize_and_hash("# comment\n# another");
        let empty = normalize_and_hash("");
        assert_eq!(all_comments, empty);
    }

    #[test]
    fn normalize_whitespace_only_lines_removed() {
        // Catches: blank lines preserved in normalized form → "a\n\nb" hashes differently from "a\nb".
        let with_blank = normalize_and_hash("let a = 1\n\nlet b = 2");
        let without_blank = normalize_and_hash("let a = 1\nlet b = 2");
        assert_eq!(with_blank, without_blank);
    }

    #[test]
    fn normalize_hash_is_non_empty_string() {
        // Catches: content_hash returns "" for non-empty input (broken hash impl).
        let h = normalize_and_hash("let x = 1");
        assert!(!h.is_empty());
    }
}
