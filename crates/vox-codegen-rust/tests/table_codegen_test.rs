//! Integration tests for Rust codegen of @table and @index declarations.

use vox_hir::*;
use vox_ast::span::Span;

fn dummy_span() -> Span {
    Span { start: 0, end: 0 }
}

fn make_task_table() -> HirTable {
    HirTable {
        id: DefId(10),
        name: "Task".to_string(),
        fields: vec![
            HirTableField {
                name: "title".to_string(),
                type_ann: HirType::Named("str".to_string()),
                span: dummy_span(),
            },
            HirTableField {
                name: "done".to_string(),
                type_ann: HirType::Named("bool".to_string()),
                span: dummy_span(),
            },
            HirTableField {
                name: "priority".to_string(),
                type_ann: HirType::Named("int".to_string()),
                span: dummy_span(),
            },
        ],
        is_pub: false,
        span: dummy_span(),
    }
}

fn make_module_with_table() -> HirModule {
    HirModule {
        imports: vec![],
        functions: vec![],
        types: vec![],
        routes: vec![],
        actors: vec![],
        workflows: vec![],
        activities: vec![],
        tests: vec![],
        server_fns: vec![],
        tables: vec![make_task_table()],
        indexes: vec![HirIndex {
            table_name: "Task".to_string(),
            index_name: "by_done".to_string(),
            columns: vec!["done".to_string(), "priority".to_string()],
            span: dummy_span(),
        }],
        mcp_tools: vec![],
    }
}

#[test]
fn table_generates_create_table_ddl() {
    let table = make_task_table();
    let ddl = vox_codegen_rust::emit::emit_table_ddl(&table);

    assert!(ddl.contains("CREATE TABLE IF NOT EXISTS task"), "should use lowercase table name");
    assert!(ddl.contains("_id INTEGER PRIMARY KEY AUTOINCREMENT"), "should have auto-increment PK");
    assert!(ddl.contains("title TEXT NOT NULL"), "str -> TEXT NOT NULL");
    assert!(ddl.contains("done INTEGER NOT NULL"), "bool -> INTEGER NOT NULL");
    assert!(ddl.contains("priority INTEGER NOT NULL"), "int -> INTEGER NOT NULL");
}

#[test]
fn index_generates_create_index_ddl() {
    let index = HirIndex {
        table_name: "Task".to_string(),
        index_name: "by_done".to_string(),
        columns: vec!["done".to_string(), "priority".to_string()],
        span: dummy_span(),
    };
    let ddl = vox_codegen_rust::emit::emit_index_ddl(&index);

    assert!(ddl.contains("CREATE INDEX IF NOT EXISTS idx_task_by_done"), "index name");
    assert!(ddl.contains("ON task (done, priority)"), "columns listed");
}

#[test]
fn table_struct_in_lib() {
    let module = make_module_with_table();
    let output = vox_codegen_rust::generate(&module, "test_data").unwrap();

    let lib_rs = output.files.get("src/lib.rs").expect("lib.rs should exist");
    assert!(lib_rs.contains("pub struct Task {"), "Task struct should exist");
    assert!(lib_rs.contains("pub _id: Option<i64>,"), "should have _id field");
    assert!(lib_rs.contains("pub title: String,"), "str -> String");
    assert!(lib_rs.contains("pub done: bool,"), "bool");
    assert!(lib_rs.contains("pub priority: i64,"), "int -> i64");
}

#[test]
fn db_setup_in_main() {
    let module = make_module_with_table();
    let output = vox_codegen_rust::generate(&module, "test_data").unwrap();

    let main_rs = output.files.get("src/main.rs").expect("main.rs should exist");

    // DB imports
    assert!(main_rs.contains("use rusqlite::Connection;"), "rusqlite import");
    assert!(main_rs.contains("use std::sync::Arc;"), "Arc import");
    assert!(main_rs.contains("use tokio::sync::Mutex;"), "Mutex import");

    // DB initialization
    assert!(main_rs.contains("Connection::open(\"app.db\")"), "db open");
    assert!(main_rs.contains("PRAGMA journal_mode=WAL"), "WAL mode");
    assert!(main_rs.contains("CREATE TABLE IF NOT EXISTS task"), "DDL in main");
    assert!(main_rs.contains("CREATE INDEX IF NOT EXISTS idx_task_by_done"), "index DDL in main");
    assert!(main_rs.contains("Arc::new(Mutex::new(db))"), "wrapped in Arc<Mutex>");
}

#[test]
fn cargo_toml_includes_rusqlite() {
    let toml = vox_codegen_rust::emit::emit_cargo_toml("my_app");
    assert!(toml.contains("rusqlite"), "rusqlite dependency should be present");
    assert!(toml.contains("bundled"), "bundled feature flag");
}

#[test]
fn no_tables_no_db_setup() {
    let module = HirModule {
        imports: vec![],
        functions: vec![],
        types: vec![],
        routes: vec![],
        actors: vec![],
        workflows: vec![],
        activities: vec![],
        tests: vec![],
        server_fns: vec![],
        tables: vec![],
        indexes: vec![],
        mcp_tools: vec![],
    };
    let output = vox_codegen_rust::generate(&module, "test_empty").unwrap();
    let main_rs = output.files.get("src/main.rs").expect("main.rs should exist");

    assert!(!main_rs.contains("rusqlite"), "no db imports when no tables");
    assert!(!main_rs.contains("Connection::open"), "no db open when no tables");
}

#[test]
fn id_type_maps_to_i64() {
    // Verify that Id[Task] -> i64 in the Rust type system
    let table = HirTable {
        id: DefId(20),
        name: "Comment".to_string(),
        fields: vec![
            HirTableField {
                name: "text".to_string(),
                type_ann: HirType::Named("str".to_string()),
                span: dummy_span(),
            },
            HirTableField {
                name: "task_id".to_string(),
                type_ann: HirType::Generic("Id".to_string(), vec![HirType::Named("Task".to_string())]),
                span: dummy_span(),
            },
        ],
        is_pub: false,
        span: dummy_span(),
    };

    let module = HirModule {
        imports: vec![],
        functions: vec![],
        types: vec![],
        routes: vec![],
        actors: vec![],
        workflows: vec![],
        activities: vec![],
        tests: vec![],
        server_fns: vec![],
        tables: vec![table],
        indexes: vec![],
        mcp_tools: vec![],
    };
    let output = vox_codegen_rust::generate(&module, "test_id").unwrap();
    let lib_rs = output.files.get("src/lib.rs").expect("lib.rs should exist");

    assert!(lib_rs.contains("pub task_id: i64,"), "Id[Task] should map to i64");

    // Also verify DDL: Id[Task] -> INTEGER
    let ddl_output = output.files.get("src/main.rs").expect("main.rs should exist");
    assert!(ddl_output.contains("task_id INTEGER NOT NULL"), "Id[Task] -> INTEGER NOT NULL in DDL");
}

#[test]
fn optional_field_nullable() {
    let table = HirTable {
        id: DefId(30),
        name: "Profile".to_string(),
        fields: vec![
            HirTableField {
                name: "name".to_string(),
                type_ann: HirType::Named("str".to_string()),
                span: dummy_span(),
            },
            HirTableField {
                name: "bio".to_string(),
                type_ann: HirType::Generic("Option".to_string(), vec![HirType::Named("str".to_string())]),
                span: dummy_span(),
            },
        ],
        is_pub: false,
        span: dummy_span(),
    };

    let ddl = vox_codegen_rust::emit::emit_table_ddl(&table);
    assert!(ddl.contains("name TEXT NOT NULL"), "required field is NOT NULL");
    // Option fields should NOT have NOT NULL
    assert!(ddl.contains("bio TEXT") && !ddl.contains("bio TEXT NOT NULL"), "optional field should be nullable");
}
