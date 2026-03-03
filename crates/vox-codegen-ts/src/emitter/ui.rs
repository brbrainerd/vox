use vox_ast::decl::{KeyframeDecl, ThemeDecl, ComponentDecl};

pub fn generate_keyframes_css(keyframes: &[&KeyframeDecl]) -> String {
    let mut css = String::new();
    for kf in keyframes {
        css.push_str(&format!("@keyframes {} {{\n", kf.name));
        for step in &kf.steps {
            css.push_str(&format!("  {} {{\n", step.selector));
            for (prop, val) in &step.properties {
                let css_prop = if prop.starts_with("--") {
                    prop.clone()
                } else {
                    prop.chars().fold(String::new(), |mut acc, c| {
                        if c.is_uppercase() {
                            acc.push('-');
                            acc.push(c.to_ascii_lowercase());
                        } else {
                            acc.push(c);
                        }
                        acc
                    })
                };
                css.push_str(&format!("    {}: {};\n", css_prop, val));
            }
            css.push_str("  }\n");
        }
        css.push_str("}\n\n");
    }
    css
}

pub fn generate_theme_css(theme: &ThemeDecl) -> String {
    let mut css = String::new();
    css.push_str(":root {\n");
    for (prop, val) in &theme.light {
        css.push_str(&format!("  --{}: {};\n", prop, val));
    }
    css.push_str("}\n\n");
    css.push_str("[data-theme=\"dark\"] {\n");
    for (prop, val) in &theme.dark {
        css.push_str(&format!("  --{}: {};\n", prop, val));
    }
    css.push_str("}\n");
    css
}

pub fn generate_theme_toggle() -> String {
    "import React, { useState, useEffect } from \"react\";\n\nexport function ThemeToggle(): React.ReactElement {\n  const [theme, setTheme] = useState<\"light\" | \"dark\">(\n    () => (document.documentElement.dataset.theme as \"light\" | \"dark\") || \"light\"\n  );\n  useEffect(() => {\n    document.documentElement.dataset.theme = theme;\n  }, [theme]);\n  return (\n    <button onClick={() => setTheme(t => t === \"light\" ? \"dark\" : \"light\")}>\n      {theme === \"light\" ? \"🌙\" : \"☀️\"}\n    </button>\n  );\n}\n".to_string()
}

pub fn generate_component_css(comp: &ComponentDecl) -> String {
    let mut css = String::new();
    let comp_name_lower = comp.func.name.to_lowercase();
    for block in &comp.styles {
        let parts: Vec<&str> = block.selector.split(',').collect();
        let scoped_selectors: Vec<String> = parts.into_iter().map(|s| {
            let s = s.trim();
            if s.contains(':') {
                let mut split = s.splitn(2, ':');
                let base = split.next().unwrap();
                let pseudo = split.next().unwrap();
                format!("{}[data-vox-cp=\"{}\"]:{}", base.trim(), comp_name_lower, pseudo)
            } else {
                format!("{}[data-vox-cp=\"{}\"]", s, comp_name_lower)
            }
        }).collect();
        let scoped_selector = scoped_selectors.join(", ");
        css.push_str(&format!("{} {{\n", scoped_selector));
        for (prop, val) in &block.properties {
            let css_prop = if prop.starts_with("--") {
                prop.clone()
            } else {
                prop.chars().fold(String::new(), |mut acc, c| {
                    if c.is_uppercase() {
                        acc.push('-');
                        acc.push(c.to_ascii_lowercase());
                    } else {
                        acc.push(c);
                    }
                    acc
                })
            };
            css.push_str(&format!("  {}: {};\n", css_prop, val));
        }
        css.push_str("}\n\n");
    }
    css
}
