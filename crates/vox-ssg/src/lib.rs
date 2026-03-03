//! `vox-ssg` — Static Site Generation for Vox.
//!
//! Converts Vox modules with `routes:` declarations into static HTML shells
//! ready for Vite SSR pre-rendering.

use vox_ast::decl::{Decl, Module};


/// Generate static HTML files for all `@page` components and route entries.
///
/// Returns a `Vec<(filename, html_content)>` where each entry is a relative
/// output path (e.g. `"index.html"`, `"about/index.html"`) and its HTML.
pub fn generate_static_site(module: &Module) -> Vec<(String, String)> {
    let mut pages = Vec::new();

    // Collect route entries from `routes:` declarations
    for decl in &module.declarations {
        match decl {
            Decl::Routes(r) => {
                for entry in &r.entries {
                    let path = &entry.path;
                    let component = &entry.component_name;
                    let filename = path_to_filename(path);
                    let html = render_html_shell(component, path);
                    pages.push((filename, html));
                }
            }
            Decl::Page(p) => {
                let path = &p.path;
                let component = &p.func.name;
                let filename = path_to_filename(path);
                let html = render_html_shell(component, path);
                pages.push((filename, html));
            }
            _ => {}
        }
    }

    // If no routes declared, emit a minimal index.html
    if pages.is_empty() {
        pages.push(("index.html".to_string(), render_html_shell("App", "/")));
    }

    pages
}

/// Convert a Vox route path like `/about/team` to `about/team/index.html`.
fn path_to_filename(path: &str) -> String {
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() || trimmed == "/" {
        "index.html".to_string()
    } else {
        // Dynamic segments like :id become [id]
        let normalized = trimmed
            .split('/')
            .map(|seg| {
                if seg.starts_with(':') {
                    format!("[{}]", &seg[1..])
                } else {
                    seg.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("/");
        format!("{normalized}/index.html")
    }
}

/// Render a minimal HTML shell for a given component / route.
///
/// The shell includes:
/// - `<!DOCTYPE html>` + meta tags
/// - A `<div id="root">` for React hydration
/// - A `<script>` referencing the Vite entry point
fn render_html_shell(component: &str, path: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>{component}</title>
  <script type="module" src="/src/main.tsx"></script>
</head>
<body>
  <!-- Vox SSG: route="{path}" component="{component}" -->
  <div id="root"><!-- ssr --></div>
</body>
</html>
"#,
        component = component,
        path = path,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssg_generates_index_html() {
        use vox_ast::span::Span;
        let module = Module { declarations: vec![], span: Span { start: 0, end: 0 } };
        let pages = generate_static_site(&module);
        assert!(!pages.is_empty(), "Should produce at least one page");
        let (name, html) = &pages[0];
        assert_eq!(name, "index.html");
        assert!(html.contains("<!DOCTYPE html>"), "Expected DOCTYPE in: {html}");
        assert!(html.contains("<div id=\"root\">"), "Expected root div in: {html}");
    }

    #[test]
    fn path_to_filename_root() {
        assert_eq!(path_to_filename("/"), "index.html");
        assert_eq!(path_to_filename(""), "index.html");
    }

    #[test]
    fn path_to_filename_nested() {
        assert_eq!(path_to_filename("/about"), "about/index.html");
        assert_eq!(path_to_filename("/blog/post"), "blog/post/index.html");
    }

    #[test]
    fn path_to_filename_dynamic_segment() {
        assert_eq!(path_to_filename("/items/:id"), "items/[id]/index.html");
    }

    #[test]
    fn ssg_with_routes_emits_one_page_per_route() {
        use vox_ast::decl::{Decl, RoutesDecl, RouteEntry};
        use vox_ast::span::Span;

        let dummy_span = Span { start: 0, end: 0 };
        let module = Module {
            declarations: vec![Decl::Routes(RoutesDecl {
                entries: vec![
                    RouteEntry {
                        path: "/".to_string(),
                        component_name: "Home".to_string(),
                        children: vec![],
                        redirect: None,
                        is_wildcard: false,
                        span: dummy_span,
                    },
                    RouteEntry {
                        path: "/about".to_string(),
                        component_name: "About".to_string(),
                        children: vec![],
                        redirect: None,
                        is_wildcard: false,
                        span: dummy_span,
                    },
                ],
                span: dummy_span,
            })],
            span: dummy_span,
        };

        let pages = generate_static_site(&module);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].0, "index.html");
        assert_eq!(pages[1].0, "about/index.html");
        assert!(pages[0].1.contains("<!DOCTYPE html>"));
        assert!(pages[1].1.contains("About"));
    }
}
