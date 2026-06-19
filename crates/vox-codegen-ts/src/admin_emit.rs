//! Naked-objects admin codegen: HirTable → React list/edit views.
//! Opt-in only (see admin-registry.yaml). Design §2.2.
//!
//! **Framework-agnostic by design.** The emitted components depend only on
//! `React` (imported by the `forms.tsx` header in `emitter.rs`) and the DOM
//! `fetch` global — there is NO backend client (no Convex `useQuery(api.x.list)`,
//! no unimported `api`/`useQuery` symbols), so the output type-checks under the
//! `ts_emit_typecheck_test` gate. CRUD goes through a conventional REST endpoint
//! (`GET`/`POST /api/<table>`); a future endpoint-codegen plan can swap the URL
//! or inject a typed client without changing this view layer.
use vox_compiler::hir::HirTable;

/// Conventional lowercase REST collection segment for a table (`User` → `user`).
fn collection_path(table: &HirTable) -> String {
    table.name.to_lowercase()
}

/// Locally-defined submit helper name the edit form's `onSubmit` calls
/// (`User` → `upsert_user`). Kept distinct from any endpoint name so the
/// `forms.tsx` header does not try to import it from `./vox-client`.
fn upsert_fn_name(table: &HirTable) -> String {
    format!("upsert_{}", table.name.to_lowercase())
}

pub fn emit_admin_list(table: &HirTable) -> String {
    let name = &table.name;
    let path = collection_path(table);
    let headers: String = table
        .fields
        .iter()
        .map(|f| format!("<th>{}</th>", f.name))
        .collect();
    let cells: String = table
        .fields
        .iter()
        .map(|f| format!("<td>{{String(row[\"{}\"] ?? \"\")}}</td>", f.name))
        .collect();
    // React is imported by the forms.tsx header; `fetch` is a DOM global. No
    // backend client is referenced, so this is self-contained and type-checks.
    format!(
        "export function {name}List() {{\n\
         \x20 const [rows, setRows] = React.useState<Array<Record<string, unknown>>>([]);\n\
         \x20 React.useEffect(() => {{\n\
         \x20   fetch(\"/api/{path}\").then((r) => r.json()).then(setRows).catch(() => {{}});\n\
         \x20 }}, []);\n\
         \x20 return (<table className=\"vox-admin-list\">\n\
         \x20   <thead><tr>{headers}</tr></thead>\n\
         \x20   <tbody>{{rows.map((row, i) => (<tr key={{i}}>{cells}</tr>))}}</tbody>\n\
         \x20 </table>);\n}}\n",
    )
}

use vox_compiler::hir::nodes::form::{HirForm, HirFormField};

pub fn emit_admin_edit(table: &HirTable) -> String {
    let helper = upsert_fn_name(table);
    let path = collection_path(table);
    let fields: Vec<HirFormField> = table
        .fields
        .iter()
        .map(|f| HirFormField {
            name: f.name.clone(),
            ty: f.type_ann.clone(),
            label: None,
            required: false,
            hidden: false,
            default: None,
            constraints: vec![],
            span: f.span,
        })
        .collect();
    let form = HirForm {
        name: format!("{}Edit", table.name),
        fields,
        // Point onSubmit at the LOCAL helper below (not a Convex `api.x.upsert`),
        // so `form_emit` emits `await upsert_<t>({ ...fields })` against a defined fn.
        on_submit: Some(helper.clone()),
        success_redirect: None,
        error_message: None,
        span: table.span,
    };
    // Local, framework-agnostic submit helper. `form_emit` calls it as
    // `await {on_submit}({ field1, field2 })`, so it takes one payload object
    // and POSTs it to the REST collection. Depends only on `fetch`/`JSON`.
    let helper_src = format!(
        "async function {helper}(payload: Record<string, unknown>): Promise<void> {{\n\
         \x20 await fetch(\"/api/{path}\", {{ method: \"POST\", headers: {{ \"Content-Type\": \"application/json\" }}, body: JSON.stringify(payload) }});\n\
         }}\n",
    );
    format!("{helper_src}{}", super::form_emit::emit_form(&form))
}

pub fn emit_admin(table: &HirTable) -> String {
    let mut out = emit_admin_list(table);
    out.push('\n');
    out.push_str(&emit_admin_edit(table));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_compiler::ast::span::Span; // re-exported path used across vox-codegen-ts; NOT vox_ast (design §5b.1)
    use vox_compiler::hir::DefId;
    use vox_compiler::hir::HirType;
    use vox_compiler::hir::{HirTable, HirTableField};

    fn table() -> HirTable {
        HirTable {
            id: DefId(0),
            name: "User".into(),
            fields: vec![
                HirTableField {
                    name: "name".into(),
                    type_ann: HirType::Named("string".into()),
                    span: Span::new(0, 0),
                },
                HirTableField {
                    name: "email".into(),
                    type_ann: HirType::Named("email".into()),
                    span: Span::new(0, 0),
                },
            ],
            primary_key: None,
            is_extern: false,
            source: None,
            is_pub: true,
            is_deprecated: false,
            span: Span::new(0, 0),
        }
    }

    /// Guards against regressing to the Convex idiom (`useQuery(api.x.list)`,
    /// `api.x.upsert`, `row._id`) that referenced unimported symbols and would
    /// not type-check. The authoritative *compile* check is the integration
    /// test `admin_output_typechecks_when_gated` (runs `tsc --noEmit`).
    fn assert_no_unimported_backend(out: &str) {
        assert!(
            !out.contains("api."),
            "must not emit Convex api.* refs:\n{out}"
        );
        assert!(
            !out.contains("useQuery"),
            "must not emit useQuery (repo uses useVoxServerQuery / fetch):\n{out}"
        );
        assert!(
            !out.contains("_id"),
            "must not assume Convex row._id:\n{out}"
        );
    }

    #[test]
    fn list_view_has_component_columns_and_self_contained_fetch() {
        let out = emit_admin_list(&table());
        assert!(out.contains("export function UserList()"), "name:\n{out}");
        assert!(out.contains(">name<"), "name col:\n{out}");
        assert!(out.contains(">email<"), "email col:\n{out}");
        assert!(out.contains("<table"), "table:\n{out}");
        // Self-contained data load: React state + DOM fetch, no backend client.
        assert!(out.contains("React.useState"), "useState:\n{out}");
        assert!(out.contains("fetch(\"/api/user\")"), "rest fetch:\n{out}");
        assert_no_unimported_backend(&out);
    }

    #[test]
    fn edit_form_reuses_form_emit_with_local_submit_helper() {
        let out = emit_admin_edit(&table());
        assert!(out.contains("export function UserEdit()"), "name:\n{out}");
        assert!(
            out.contains("type=\"email\""),
            "typed email via form_emit:\n{out}"
        );
        // onSubmit binds to a LOCALLY-defined helper, not a Convex api call.
        assert!(
            out.contains("async function upsert_user("),
            "local submit helper defined:\n{out}"
        );
        assert!(
            out.contains("fetch(\"/api/user\""),
            "helper POSTs to REST:\n{out}"
        );
        assert_no_unimported_backend(&out);
    }

    #[test]
    fn emit_admin_composes_list_and_edit() {
        let out = emit_admin(&table());
        assert!(out.contains("export function UserList()"), "list:\n{out}");
        assert!(out.contains("export function UserEdit()"), "edit:\n{out}");
        assert_no_unimported_backend(&out);
    }
}
