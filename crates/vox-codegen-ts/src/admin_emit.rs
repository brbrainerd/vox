//! Naked-objects admin codegen: HirTable → React list/detail/edit views.
//! Opt-in only (see admin-registry.yaml). Design §2.2.
use vox_compiler::hir::HirTable;

pub fn emit_admin_list(table: &HirTable) -> String {
    let name = &table.name;
    let headers: String = table
        .fields
        .iter()
        .map(|f| format!("<th>{}</th>", f.name))
        .collect();
    let cells: String = table
        .fields
        .iter()
        .map(|f| format!("<td>{{String(row.{} ?? \"\")}}</td>", f.name))
        .collect();
    format!(
        "export function {name}List() {{\n\
         \x20 const rows = useQuery(api.{nl}.list) ?? [];\n\
         \x20 return (<table className=\"vox-admin-list\">\n\
         \x20   <thead><tr>{headers}</tr></thead>\n\
         \x20   <tbody>{{rows.map((row: any) => (<tr key={{row._id}}>{cells}</tr>))}}</tbody>\n\
         \x20 </table>);\n}}\n",
        name = name,
        nl = name.to_lowercase()
    )
}

use vox_compiler::hir::nodes::form::{HirForm, HirFormField};

pub fn emit_admin_edit(table: &HirTable) -> String {
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
        on_submit: Some(format!("api.{}.upsert", table.name.to_lowercase())),
        success_redirect: None,
        error_message: None,
        span: table.span,
    };
    super::form_emit::emit_form(&form)
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
    #[test]
    fn list_view_has_component_and_columns() {
        let out = emit_admin_list(&table());
        assert!(out.contains("export function UserList()"), "name:\n{out}");
        assert!(out.contains(">name<"), "name col:\n{out}");
        assert!(out.contains(">email<"), "email col:\n{out}");
        assert!(out.contains("<table"), "table:\n{out}");
    }

    #[test]
    fn edit_form_reuses_form_emit_and_typed_inputs() {
        let out = emit_admin_edit(&table());
        assert!(out.contains("export function UserEdit()"), "name:\n{out}");
        assert!(
            out.contains("type=\"email\""),
            "typed email via form_emit:\n{out}"
        );
    }

    #[test]
    fn emit_admin_composes_list_and_edit() {
        let out = emit_admin(&table());
        assert!(out.contains("export function UserList()"), "list:\n{out}");
        assert!(out.contains("export function UserEdit()"), "edit:\n{out}");
    }
}
