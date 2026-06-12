//! `@form` declaration → React form component emit (Task C3).
//!
//! Each `@form Name { … }` decl emits a self-contained React component into
//! `forms.tsx` with:
//! - `React.useState` for every visible field
//! - client-side validation (required, max_len, min_len)
//! - async submit handler that calls the bound `@mutation` (or `@server`) function
//! - optional router-agnostic redirect on success (`history.pushState` +
//!   `popstate`, so any router that observes `popstate` navigates)
//! - accessible `<label>` / `<input>` pairs with ARIA error associations
//! - a banner error `<div>` for server-side failures

use vox_compiler::hir::HirType;
use vox_compiler::hir::nodes::form::{HirFieldConstraint, HirForm, HirFormField};

/// Emit a single `@form` as a React function component string.
pub fn emit_form(form: &HirForm) -> String {
    let mut out = String::new();
    let name = &form.name;
    let visible: Vec<&HirFormField> = form.fields.iter().filter(|f| !f.hidden).collect();

    out.push_str(&format!("export function {name}() {{\n"));

    // State variables
    for f in &visible {
        let init = field_initial_value(f);
        out.push_str(&format!(
            "  const [{fname}, set_{fname}] = React.useState({init});\n",
            fname = f.name
        ));
    }
    out.push_str("  const [errors, setErrors] = React.useState<Record<string, string>>({});\n");
    out.push_str("  const [submitting, setSubmitting] = React.useState(false);\n");
    out.push_str("  const [bannerError, setBannerError] = React.useState<string | null>(null);\n");
    // Redirect (if any) uses the history API directly — see the submit handler.
    // No router hook, so the form renders under any router (incl. the Vox-emitted
    // dependency-free one) without a provider.

    // Validation function
    out.push_str("  function validate(): Record<string, string> {\n");
    out.push_str("    const e: Record<string, string> = {};\n");
    for f in &visible {
        if f.required {
            let label = f.label.as_deref().unwrap_or(&f.name);
            // Numeric fields hold a `number` (`NaN` when empty); string fields use
            // the `=== ""` empty check.
            let empty_check = if hir_type_to_input_type(&f.ty) == "number" {
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
                HirFieldConstraint::MaxLen(n) => {
                    let label = f.label.as_deref().unwrap_or(&f.name);
                    out.push_str(&format!(
                        "    if (typeof {fname} === \"string\" && {fname}.length > {n}) e.{fname} = \"{label} too long (max {n})\";\n",
                        fname = f.name
                    ));
                }
                HirFieldConstraint::MinLen(n) => {
                    let label = f.label.as_deref().unwrap_or(&f.name);
                    out.push_str(&format!(
                        "    if (typeof {fname} === \"string\" && {fname}.length < {n}) e.{fname} = \"{label} too short (min {n})\";\n",
                        fname = f.name
                    ));
                }
                _ => {}
            }
        }
    }
    out.push_str("    return e;\n  }\n");

    // Submit handler
    let submit_fn = form.on_submit.as_deref().unwrap_or("_noSubmit");
    // vox-client mutations accept a single `args` object with named fields matching the endpoint
    // params. Emit `{ field1, field2 }` shorthand so the call matches the generated client signature.
    // Numeric fields hold `NaN` while empty (see `field_initial_value`). A
    // *required* empty numeric is caught by `validate()` before submit, but an
    // *optional* one would otherwise forward `NaN` into the client call (it
    // serializes to `null` and can break endpoint validation or persist a wrong
    // value). Coerce empty numerics to `undefined` so they're omitted from the
    // JSON body; non-numeric fields use object shorthand unchanged.
    let fields: Vec<String> = visible
        .iter()
        .map(|f| {
            if hir_type_to_input_type(&f.ty) == "number" {
                format!("{n}: Number.isNaN({n}) ? undefined : {n}", n = f.name)
            } else {
                f.name.clone()
            }
        })
        .collect();
    let submit_call_args = if fields.is_empty() {
        "{}".to_string()
    } else {
        format!("{{ {} }}", fields.join(", "))
    };
    let err_msg = form
        .error_message
        .as_ref()
        .map(|m| format!("\"{}\"", m.replace('"', "\\\"")))
        .unwrap_or_else(|| "String(err)".into());

    out.push_str(&format!(
        "  const onSubmit = async (ev: React.FormEvent) => {{\n\
         \x20   ev.preventDefault();\n\
         \x20   const errs = validate();\n\
         \x20   setErrors(errs);\n\
         \x20   if (Object.keys(errs).length > 0) return;\n\
         \x20   setSubmitting(true);\n\
         \x20   setBannerError(null);\n\
         \x20   try {{\n\
         \x20     await {submit_fn}({submit_call_args});\n"
    ));
    if let Some(r) = &form.success_redirect {
        // Router-agnostic redirect: push history + dispatch popstate so the
        // active router (Vox-emitted dependency-free, react-router, or @tanstack
        // — all observe popstate) navigates to the success route.
        out.push_str(&format!(
            "      window.history.pushState(null, \"\", \"{r}\");\n      window.dispatchEvent(new PopStateEvent(\"popstate\"));\n"
        ));
    }
    out.push_str(&format!(
        "    }} catch (err) {{\n\
         \x20     setBannerError({err_msg});\n\
         \x20   }} finally {{\n\
         \x20     setSubmitting(false);\n\
         \x20   }}\n\
         \x20 }};\n"
    ));

    // Render
    out.push_str("  return (\n    <form onSubmit={onSubmit} className=\"vox-form\">\n");
    out.push_str("      {bannerError && <div role=\"alert\" className=\"vox-form-error-banner\">{bannerError}</div>}\n");

    for f in &visible {
        let label = f.label.as_deref().unwrap_or(&f.name);
        let req_marker = if f.required { " *" } else { "" };
        let input_type = hir_type_to_input_type(&f.ty);
        let value_prop = match input_type {
            "checkbox" => "checked",
            "number" => "valueAsNumber",
            _ => "value",
        };
        let max_len_attr = f
            .constraints
            .iter()
            .find_map(|c| match c {
                HirFieldConstraint::MaxLen(n) => Some(format!(" maxLength={{{n}}}")),
                _ => None,
            })
            .unwrap_or_default();

        let bind_attr = if input_type == "checkbox" {
            format!("checked={{{fname}}}", fname = f.name)
        } else if input_type == "number" {
            // Numeric fields hold a `number` that is `NaN` when empty/unfilled —
            // show a blank input then (a `value` of NaN would render "NaN").
            format!(
                "value={{Number.isNaN({fname}) ? \"\" : {fname}}}",
                fname = f.name
            )
        } else {
            format!("value={{{fname} ?? \"\"}}", fname = f.name)
        };

        out.push_str(&format!(
            "      <label className=\"vox-form-field\">\n\
             \x20       <span>{label}{req_marker}</span>\n\
             \x20       <input type=\"{input_type}\" {bind_attr} onChange={{e => set_{fname}(e.target.{value_prop})}}{max_len_attr} aria-invalid={{!!errors.{fname}}} aria-describedby=\"{fname}-error\" />\n\
             \x20       {{errors.{fname} && <span id=\"{fname}-error\" role=\"alert\" className=\"vox-form-error\">{{errors.{fname}}}</span>}}\n\
             \x20     </label>\n",
            fname = f.name
        ));
    }

    out.push_str("      <button type=\"submit\" disabled={submitting}>{submitting ? \"Saving\u{2026}\" : \"Submit\"}</button>\n    </form>\n  );\n}\n");
    out
}

fn hir_type_to_input_type(ty: &HirType) -> &'static str {
    match ty {
        HirType::Named(t) if t == "int" || t == "float" || t == "decimal" => "number",
        HirType::Named(t) if t == "bool" => "checkbox",
        HirType::Named(t) if t == "timestamp" => "datetime-local",
        _ => "text",
    }
}

fn field_initial_value(f: &HirFormField) -> &'static str {
    match &f.ty {
        // Numeric fields start empty (NaN) so a required field isn't pre-satisfied
        // by a `0` default; the input renders blank (see the `value` binding).
        HirType::Named(t) if t == "int" || t == "float" || t == "decimal" => "NaN",
        HirType::Named(t) if t == "bool" => "false",
        _ => "\"\"",
    }
}
