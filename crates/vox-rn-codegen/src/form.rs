//! `@form` declaration → React Native form component emit.
//!
//! Parallel to `crates/vox-codegen/src/codegen_ts/form_emit.rs` (web target).
//! Same validation logic, same state declarations, same submit semantics —
//! only the leaf rendering differs:
//!
//!   Web:  `<form onSubmit><label><span><input/></span></label><button/></form>`
//!   RN:   `<View><Text/><TextInput/><Pressable><Text/></Pressable></View>`
//!
//! Single source of truth: every field iteration, validation rule, and submit
//! call shape comes from the same `HirForm` HIR node that the web emit consumes.
//! Adding a new field constraint (e.g. `pattern`, `email`) requires updating both
//! `form_emit.rs` and this file together; the harness's
//! `mobile_and_web_emit_differ_in_leaf_shape_not_in_logic`-style gates catch
//! drift at CI time.

use vox_compiler::hir::HirType;
use vox_compiler::hir::nodes::form::{HirFieldConstraint, HirForm, HirFormField};

/// Emit a single `@form` as an RN function component string.
pub fn emit_form(form: &HirForm) -> String {
    let mut out = String::new();
    let name = &form.name;
    let visible: Vec<&HirFormField> = form.fields.iter().filter(|f| !f.hidden).collect();

    out.push_str(&format!(
        "export function {name}(): React.ReactElement {{\n"
    ));

    // State per visible field.
    for f in &visible {
        let init = field_initial_value(f);
        out.push_str(&format!(
            "  const [{fname}, set_{fname}] = useState<{ts_ty}>({init});\n",
            fname = f.name,
            ts_ty = hir_type_to_ts(&f.ty)
        ));
    }
    out.push_str("  const [errors, setErrors] = useState<Record<string, string>>({});\n");
    out.push_str("  const [submitting, setSubmitting] = useState(false);\n");
    out.push_str("  const [bannerError, setBannerError] = useState<string | null>(null);\n");

    // Validation — identical to the web emit's logic.
    out.push_str("  function validate(): Record<string, string> {\n");
    out.push_str("    const e: Record<string, string> = {};\n");
    for f in &visible {
        let label = f.label.as_deref().unwrap_or(&f.name);
        if f.required {
            // Numeric fields hold a `number` (never `""`); use NaN as the
            // "empty" sentinel so the check type-checks. String fields keep the
            // `=== ""` empty check.
            let empty_check = if field_is_numeric(&f.ty) {
                format!("Number.isNaN({n})", n = f.name)
            } else {
                format!("{n} === \"\"", n = f.name)
            };
            out.push_str(&format!(
                "    if ({n} === undefined || {n} === null || {empty_check}) e.{n} = \"{label} is required\";\n",
                n = f.name
            ));
        }
        for c in &f.constraints {
            match c {
                HirFieldConstraint::MaxLen(max) => {
                    out.push_str(&format!(
                        "    if (typeof {fname} === \"string\" && {fname}.length > {max}) e.{fname} = \"{label} too long (max {max})\";\n",
                        fname = f.name
                    ));
                }
                HirFieldConstraint::MinLen(min) => {
                    out.push_str(&format!(
                        "    if (typeof {fname} === \"string\" && {fname}.length < {min}) e.{fname} = \"{label} too short (min {min})\";\n",
                        fname = f.name
                    ));
                }
                _ => {}
            }
        }
    }
    out.push_str("    return e;\n  }\n");

    // Submit handler — async, mirrors web emit.
    let submit_fn = form.on_submit.as_deref().unwrap_or("_noSubmit");
    let args_obj = visible
        .iter()
        .map(|f| f.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let submit_call_args = if args_obj.is_empty() {
        "{}".to_string()
    } else {
        format!("{{ {args_obj} }}")
    };
    let err_msg = form
        .error_message
        .as_ref()
        .map(|m| format!("\"{}\"", m.replace('"', "\\\"")))
        .unwrap_or_else(|| "String(err)".into());

    out.push_str(
        "  const onSubmit = async () => {\n\
         \x20   const errs = validate();\n\
         \x20   setErrors(errs);\n\
         \x20   if (Object.keys(errs).length > 0) return;\n\
         \x20   setSubmitting(true);\n\
         \x20   setBannerError(null);\n\
         \x20   try {\n",
    );
    out.push_str(&format!("      await {submit_fn}({submit_call_args});\n"));
    out.push_str(&format!(
        "    }} catch (err) {{\n\
         \x20     setBannerError({err_msg});\n\
         \x20   }} finally {{\n\
         \x20     setSubmitting(false);\n\
         \x20   }}\n\
         \x20 }};\n"
    ));

    // Render: View > (banner) > per-field View(label + input + error) > submit Pressable.
    out.push_str("  return (\n    <View style={styles.form}>\n");
    out.push_str(
        "      {bannerError && (\n        <Text style={styles.banner_error} accessibilityRole=\"alert\">{bannerError}</Text>\n      )}\n",
    );

    for f in &visible {
        let label = f.label.as_deref().unwrap_or(&f.name);
        let req_marker = if f.required { " *" } else { "" };
        let kb_type = hir_type_to_keyboard_type(&f.ty);
        let kb_attr = if kb_type.is_empty() {
            String::new()
        } else {
            format!(" keyboardType=\"{kb_type}\"")
        };
        let max_len_attr = f
            .constraints
            .iter()
            .find_map(|c| match c {
                HirFieldConstraint::MaxLen(n) => Some(format!(" maxLength={{{n}}}")),
                _ => None,
            })
            .unwrap_or_default();

        // For boolean fields we'd need <Switch>, not <TextInput>. For now we
        // skip boolean rendering for RN until @switch is wired; emit a hint
        // comment so future-us doesn't silently drop the field.
        if let HirType::Named(t) = &f.ty
            && t == "bool"
        {
            out.push_str(&format!(
                "      {{/* TODO(rn): boolean field `{name}` requires <Switch>; not yet emitted */}}\n",
                name = f.name
            ));
            continue;
        }

        // RN <TextInput> always works in strings. Numeric fields therefore
        // stringify on the way out (`value`) and parse on the way in
        // (`onChangeText`), so the `useState<number>` binding stays a number.
        let (value_expr, on_change_expr) = if field_is_numeric(&f.ty) {
            (
                format!("String({fname} ?? \"\")", fname = f.name),
                format!("(text) => set_{fname}(Number(text))", fname = f.name),
            )
        } else {
            (
                format!("{fname} ?? \"\"", fname = f.name),
                format!("(text) => set_{fname}(text)", fname = f.name),
            )
        };
        out.push_str(&format!(
            "      <View style={{styles.field}}>\n\
             \x20       <Text style={{styles.label}}>{label}{req_marker}</Text>\n\
             \x20       <TextInput\n\
             \x20         style={{styles.input}}\n\
             \x20         value={{{value_expr}}}\n\
             \x20         onChangeText={{{on_change_expr}}}\n\
             \x20         accessibilityLabel={{\"{label}\"}}{kb_attr}{max_len_attr}\n\
             \x20       />\n\
             \x20       {{errors.{fname} && (\n\
             \x20         <Text style={{styles.field_error}} accessibilityRole=\"alert\">{{errors.{fname}}}</Text>\n\
             \x20       )}}\n\
             \x20     </View>\n",
            fname = f.name,
        ));
    }

    out.push_str(
        "      <Pressable style={styles.submit} onPress={onSubmit} disabled={submitting}>\n\
         \x20       <Text style={styles.submit_text}>{submitting ? \"Saving\u{2026}\" : \"Submit\"}</Text>\n\
         \x20     </Pressable>\n\
         \x20   </View>\n  );\n}\n",
    );
    out
}

/// Boolean → Switch (deferred), int/float → keyboard "numeric", str → default.
fn hir_type_to_keyboard_type(ty: &HirType) -> &'static str {
    match ty {
        HirType::Named(t) if t == "int" || t == "float" || t == "decimal" => "numeric",
        _ => "",
    }
}

/// True for numeric field types whose `useState` binding is a `number` — these
/// need string⇄number marshalling around the RN `<TextInput>`.
fn field_is_numeric(ty: &HirType) -> bool {
    matches!(ty, HirType::Named(t) if t == "int" || t == "float" || t == "decimal")
}

fn hir_type_to_ts(ty: &HirType) -> &'static str {
    match ty {
        HirType::Named(t) if t == "int" || t == "float" || t == "decimal" => "number",
        HirType::Named(t) if t == "bool" => "boolean",
        _ => "string",
    }
}

fn field_initial_value(f: &HirFormField) -> &'static str {
    match &f.ty {
        HirType::Named(t) if t == "int" || t == "float" || t == "decimal" => "0",
        HirType::Named(t) if t == "bool" => "false",
        _ => "\"\"",
    }
}

/// StyleSheet block appended to `forms.tsx` when any form is emitted. Stable
/// shape keyed by the style names this module references (`form`, `field`,
/// `label`, `input`, `field_error`, `banner_error`, `submit`, `submit_text`).
pub const RN_FORM_STYLESHEET: &str = "const styles = StyleSheet.create({\n\
     \x20 form: { gap: 12 },\n\
     \x20 field: { gap: 4 },\n\
     \x20 label: { fontSize: 14, fontWeight: \"500\" },\n\
     \x20 input: {\n\
     \x20   borderWidth: 1,\n\
     \x20   borderColor: \"#d4d4d8\",\n\
     \x20   borderRadius: 6,\n\
     \x20   paddingVertical: 8,\n\
     \x20   paddingHorizontal: 12,\n\
     \x20   fontSize: 16,\n\
     \x20 },\n\
     \x20 field_error: { color: \"#dc2626\", fontSize: 12 },\n\
     \x20 banner_error: {\n\
     \x20   backgroundColor: \"#fee2e2\",\n\
     \x20   color: \"#991b1b\",\n\
     \x20   padding: 12,\n\
     \x20   borderRadius: 6,\n\
     \x20 },\n\
     \x20 submit: {\n\
     \x20   backgroundColor: \"#0a7ea4\",\n\
     \x20   paddingVertical: 10,\n\
     \x20   paddingHorizontal: 16,\n\
     \x20   borderRadius: 6,\n\
     \x20   alignItems: \"center\",\n\
     \x20 },\n\
     \x20 submit_text: { color: \"white\", fontWeight: \"500\" },\n\
     });\n";
