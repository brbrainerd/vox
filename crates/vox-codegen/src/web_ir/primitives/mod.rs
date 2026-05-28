//! TASK-6.1 — Vox GUI semantic primitive set.
//!
//! Primitives replace JSX tags as the authoring surface.  The parser already
//! accepts custom tag names inside `view:` blocks; when a tag name matches a
//! known primitive, `web_ir/lower.rs` delegates here to obtain the canonical
//! HTML tag and Tailwind class list instead of emitting the tag verbatim.
//!
//! ## Initial primitive set (10 highest-usage)
//!
//! Layout: `stack`, `row`, `column`, `wrap`
//! Content: `text`, `heading`, `link`, `image`
//! Interactive: `button`
//! Structural: `panel`, `card`, `list`, `list_item`, `route_outlet`
//!
//! Each primitive:
//! - Maps to a fixed HTML tag.
//! - Carries a base Tailwind class list.
//! - Declares its accessibility role where non-trivial.
//! - Accepts typed prop overrides (`gap`, `size`, `weight`, `surface`).
//!
//! **Prop conventions** (shared across applicable primitives):
//! - `gap` — spacing token name; resolves to `gap-<token>` Tailwind class.
//! - `size` — typography scale: `xs` | `sm` | `base` | `lg` | `xl` | `2xl`.
//! - `weight` — `normal` | `medium` | `semibold` | `bold`.
//! - `align` — `start` | `center` | `end`.
//! - `wrap` — `true` enables `flex-wrap`.

/// A resolved primitive emission: the HTML tag + ordered Tailwind class list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimitiveEmission {
    /// HTML tag to emit (e.g. `"div"`, `"p"`, `"button"`).
    pub html_tag: &'static str,
    /// Base Tailwind classes applied unconditionally.
    pub base_classes: Vec<String>,
    /// WAI-ARIA role override when different from the HTML implicit role.
    pub aria_role: Option<&'static str>,
    /// Surface pair name from `surface` attr (e.g. `"primary"`).
    /// Lowering will inject CSS vars `--fg` / `--bg` and add a `data-vox-surface` attr for validation.
    pub surface_ref: Option<String>,
}

impl PrimitiveEmission {
    /// Returns the `class` attribute value as a space-joined string.
    pub fn class_string(&self) -> String {
        self.base_classes.join(" ")
    }
}

/// Resolve a JSX tag name to a `PrimitiveEmission` if it is a known primitive.
/// Returns `None` for ordinary HTML tags or unrecognised names.
///
/// `attrs` are the raw `(name, value)` pairs from the JSX element, allowing
/// prop-driven class augmentation (e.g. `gap="4"` → `gap-4`).
/// VUV-4 universal style kwargs supported on every primitive. The set is intentionally aimed at
/// what the dashboard uses today; new kwargs are added as the migration surfaces them. Any kwarg
/// listed here MUST also appear in `PRIMITIVE_CONSUMED_PROPS` in `web_ir/lower.rs` so it doesn't
/// leak through as a raw HTML attribute.
pub const UNIVERSAL_STYLE_KWARGS: &[&str] = &[
    "gap",
    "gap_x",
    "gap_y",
    "pad",
    "pad_x",
    "pad_y",
    "pad_t",
    "pad_b",
    "pad_l",
    "pad_r",
    "mb",
    "mt",
    "ml",
    "mr",
    "mx",
    "my",
    "w",
    "h",
    "min_w",
    "min_h",
    "max_w",
    "max_h",
    "bg",
    "color",
    "border",
    "border_x",
    "border_y",
    "border_t",
    "border_b",
    "border_l",
    "border_r",
    "border_color",
    "radius",
    "radius_t",
    "radius_b",
    "radius_l",
    "radius_r",
    "radius_tl",
    "radius_tr",
    "radius_bl",
    "radius_br",
    "overflow",
    "overflow_x",
    "overflow_y",
    "flex",
    "shrink",
    "grow",
    "justify",
    "items",
    "tracking",
    "leading",
    "case",
    "italic",
    "font_family",
    "position",
    "inset",
    "top",
    "bottom",
    "left",
    "right",
    "shadow",
    "opacity",
    "raw_class",
    "safe_area",
];

#[must_use]
pub fn resolve(tag: &str, attrs: &[(String, String)]) -> Option<PrimitiveEmission> {
    let get_attr = |name: &str| -> Option<&str> {
        attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    let surface_name: Option<String> = get_attr("surface").map(|s| s.to_string());

    match tag {
        // ── Layout ────────────────────────────────────────────────────────
        "stack" | "column" => {
            let mut classes = vec!["flex".to_string(), "flex-col".to_string()];

            apply_align(get_attr("align"), &mut classes, false);
            Some(PrimitiveEmission {
                html_tag: "div",
                base_classes: classes,
                aria_role: None,
                surface_ref: surface_name.clone(),
            })
        }
        "row" => {
            let mut classes = vec!["flex".to_string(), "flex-row".to_string()];

            apply_align(get_attr("align"), &mut classes, true);
            if get_attr("wrap") == Some("true") {
                classes.push("flex-wrap".to_string());
            }
            Some(PrimitiveEmission {
                html_tag: "div",
                base_classes: classes,
                aria_role: None,
                surface_ref: surface_name.clone(),
            })
        }
        "wrap" => {
            let classes = vec![
                "flex".to_string(),
                "flex-row".to_string(),
                "flex-wrap".to_string(),
            ];

            Some(PrimitiveEmission {
                html_tag: "div",
                base_classes: classes,
                aria_role: None,
                surface_ref: surface_name.clone(),
            })
        }
        // ── Content ───────────────────────────────────────────────────────
        "text" => {
            let mut classes = Vec::new();
            apply_text_size(get_attr("size"), &mut classes);
            apply_weight(get_attr("weight"), &mut classes);
            Some(PrimitiveEmission {
                html_tag: "p",
                base_classes: classes,
                aria_role: None,
                surface_ref: surface_name.clone(),
            })
        }
        "heading" => {
            let level = get_attr("level")
                .and_then(|v| v.parse::<u8>().ok())
                .unwrap_or(2);
            let tag = match level {
                1 => "h1",
                2 => "h2",
                3 => "h3",
                4 => "h4",
                5 => "h5",
                _ => "h6",
            };
            let mut classes = Vec::new();
            let default_size = match level {
                1 => "3xl",
                2 => "2xl",
                3 => "xl",
                4 => "lg",
                _ => "base",
            };
            apply_text_size(Some(get_attr("size").unwrap_or(default_size)), &mut classes);
            apply_weight(Some(get_attr("weight").unwrap_or("semibold")), &mut classes);
            Some(PrimitiveEmission {
                html_tag: tag,
                base_classes: classes,
                aria_role: None,
                surface_ref: surface_name.clone(),
            })
        }
        "link" => {
            let mut classes = vec![
                "text-primary".to_string(),
                "underline-offset-4".to_string(),
                "hover:underline".to_string(),
            ];
            apply_text_size(get_attr("size"), &mut classes);
            Some(PrimitiveEmission {
                html_tag: "a",
                base_classes: classes,
                aria_role: None,
                surface_ref: surface_name.clone(),
            })
        }
        "image" => Some(PrimitiveEmission {
            html_tag: "img",
            base_classes: vec!["max-w-full".to_string(), "h-auto".to_string()],
            aria_role: None,
            surface_ref: surface_name.clone(),
        }),
        // ── Interactive ───────────────────────────────────────────────────
        "button" => {
            let variant = get_attr("variant").unwrap_or("default");
            let mut classes = vec![
                "inline-flex".to_string(),
                "items-center".to_string(),
                "justify-center".to_string(),
                "rounded-md".to_string(),
                "text-sm".to_string(),
                "font-medium".to_string(),
                "ring-offset-background".to_string(),
                "transition-colors".to_string(),
                "focus-visible:outline-none".to_string(),
                "focus-visible:ring-2".to_string(),
                "focus-visible:ring-ring".to_string(),
                "focus-visible:ring-offset-2".to_string(),
                "disabled:pointer-events-none".to_string(),
                "disabled:opacity-50".to_string(),
            ];
            match variant {
                "outline" => {
                    classes.push("border".to_string());
                    classes.push("border-input".to_string());
                    classes.push("bg-background".to_string());
                    classes.push("hover:bg-accent".to_string());
                    classes.push("hover:text-accent-foreground".to_string());
                }
                "ghost" => {
                    classes.push("hover:bg-accent".to_string());
                    classes.push("hover:text-accent-foreground".to_string());
                }
                "destructive" => {
                    classes.push("bg-destructive".to_string());
                    classes.push("text-destructive-foreground".to_string());
                    classes.push("hover:bg-destructive/90".to_string());
                }
                _ => {
                    // "default" variant
                    classes.push("bg-primary".to_string());
                    classes.push("text-primary-foreground".to_string());
                    classes.push("hover:bg-primary/90".to_string());
                }
            }
            apply_size_padding(get_attr("size"), &mut classes);
            Some(PrimitiveEmission {
                html_tag: "button",
                base_classes: classes,
                aria_role: None,
                surface_ref: surface_name.clone(),
            })
        }
        // ── Structural ────────────────────────────────────────────────────
        "panel" => {
            let classes = vec![
                "bg-background".to_string(),
                "rounded-lg".to_string(),
                "border".to_string(),
                "border-border".to_string(),
                "p-4".to_string(),
            ];

            Some(PrimitiveEmission {
                html_tag: "div",
                base_classes: classes,
                aria_role: Some("region"),
                surface_ref: surface_name.clone(),
            })
        }
        "card" => {
            let classes = vec![
                "bg-card".to_string(),
                "text-card-foreground".to_string(),
                "rounded-xl".to_string(),
                "border".to_string(),
                "shadow-sm".to_string(),
                "p-6".to_string(),
            ];

            Some(PrimitiveEmission {
                html_tag: "div",
                base_classes: classes,
                aria_role: Some("region"),
                surface_ref: surface_name.clone(),
            })
        }
        "list" => Some(PrimitiveEmission {
            html_tag: "ul",
            base_classes: vec!["list-none".to_string(), "space-y-1".to_string()],
            aria_role: None,
            surface_ref: surface_name.clone(),
        }),
        "list_item" | "list-item" => Some(PrimitiveEmission {
            html_tag: "li",
            base_classes: vec![],
            aria_role: None,
            surface_ref: surface_name.clone(),
        }),
        "route_outlet" | "route-outlet" => Some(PrimitiveEmission {
            html_tag: "div",
            base_classes: vec!["contents".to_string()],
            aria_role: Some("main"),
            surface_ref: surface_name.clone(),
        }),
        // ── Overlay ───────────────────────────────────────────────────────
        "overlay" => Some(PrimitiveEmission {
            html_tag: "div",
            base_classes: vec!["relative".to_string()],
            aria_role: Some("presentation"),
            surface_ref: surface_name.clone(),
        }),
        "toast" => {
            let mut classes = vec!["fixed".to_string()];
            apply_overlay_position(get_attr("position"), &mut classes);
            apply_z_index(get_attr("z"), &mut classes);
            Some(PrimitiveEmission {
                html_tag: "div",
                base_classes: classes,
                aria_role: Some("alert"),
                surface_ref: surface_name.clone(),
            })
        }
        "drawer" => {
            let mut classes = vec!["fixed".to_string()];
            apply_overlay_position(get_attr("position"), &mut classes);
            apply_z_index(get_attr("z"), &mut classes);
            Some(PrimitiveEmission {
                html_tag: "div",
                base_classes: classes,
                aria_role: Some("dialog"),
                surface_ref: surface_name.clone(),
            })
        }
        "modal" => {
            let mut classes = vec![
                "fixed".to_string(),
                "inset-0".to_string(),
                "flex".to_string(),
                "items-center".to_string(),
                "justify-center".to_string(),
            ];
            apply_z_index(get_attr("z"), &mut classes);
            Some(PrimitiveEmission {
                html_tag: "div",
                base_classes: classes,
                aria_role: Some("dialog"),
                surface_ref: surface_name.clone(),
            })
        }
        _ => None,
    }
    .map(|mut e| {
        apply_universal_kwargs(attrs, &mut e.base_classes);
        e
    })
}

/// Returns `true` if the tag is in the known primitive set.
///
/// Back-compat shim — the canonical implementation lives in
/// `vox_compiler::lowering_shared::primitive_tags::is_primitive` so analysis-side
/// callers (parser) don't need to depend on emit IR.
pub use vox_compiler::lowering_shared::primitive_tags::is_primitive;

// ---------------------------------------------------------------------------
// Prop-to-class helpers
// ---------------------------------------------------------------------------

fn apply_align(align: Option<&str>, classes: &mut Vec<String>, is_row: bool) {
    match align {
        Some("center") => {
            classes.push("items-center".to_string());
            if is_row {
                classes.push("justify-center".to_string());
            }
        }
        Some("end") => {
            classes.push("items-end".to_string());
        }
        Some("start") | None => {}
        _ => {}
    }
}

fn apply_text_size(size: Option<&str>, classes: &mut Vec<String>) {
    let cls = match size {
        Some("xs") => "text-xs",
        Some("sm") => "text-sm",
        Some("base") | None => "text-base",
        Some("lg") => "text-lg",
        Some("xl") => "text-xl",
        Some("2xl") => "text-2xl",
        Some("3xl") => "text-3xl",
        _ => "text-base",
    };
    classes.push(cls.to_string());
}

fn apply_weight(weight: Option<&str>, classes: &mut Vec<String>) {
    let cls = match weight {
        Some("normal") => "font-normal",
        Some("medium") => "font-medium",
        Some("semibold") => "font-semibold",
        Some("bold") => "font-bold",
        None => return,
        _ => return,
    };
    classes.push(cls.to_string());
}

fn apply_size_padding(size: Option<&str>, classes: &mut Vec<String>) {
    match size {
        Some("sm") => {
            classes.push("h-8".to_string());
            classes.push("px-3".to_string());
            classes.push("text-xs".to_string());
        }
        Some("lg") => {
            classes.push("h-11".to_string());
            classes.push("px-8".to_string());
        }
        Some("icon") => {
            classes.push("h-10".to_string());
            classes.push("w-10".to_string());
        }
        _ => {
            classes.push("h-10".to_string());
            classes.push("px-4".to_string());
            classes.push("py-2".to_string());
        }
    }
}

fn apply_overlay_position(position: Option<&str>, classes: &mut Vec<String>) {
    match position {
        Some("top_right") | Some("top-right") => {
            classes.push("top-0".to_string());
            classes.push("right-0".to_string());
        }
        Some("top_left") | Some("top-left") => {
            classes.push("top-0".to_string());
            classes.push("left-0".to_string());
        }
        Some("bottom_right") | Some("bottom-right") => {
            classes.push("bottom-0".to_string());
            classes.push("right-0".to_string());
        }
        Some("bottom_left") | Some("bottom-left") => {
            classes.push("bottom-0".to_string());
            classes.push("left-0".to_string());
        }
        Some("top") => {
            classes.push("top-0".to_string());
            classes.push("left-0".to_string());
            classes.push("right-0".to_string());
        }
        Some("bottom") => {
            classes.push("bottom-0".to_string());
            classes.push("left-0".to_string());
            classes.push("right-0".to_string());
        }
        Some("left") => {
            classes.push("top-0".to_string());
            classes.push("bottom-0".to_string());
            classes.push("left-0".to_string());
        }
        Some("right") => {
            classes.push("top-0".to_string());
            classes.push("bottom-0".to_string());
            classes.push("right-0".to_string());
        }
        Some("center") => {
            classes.push("inset-0".to_string());
            classes.push("m-auto".to_string());
        }
        _ => {}
    }
}

fn apply_z_index(z: Option<&str>, classes: &mut Vec<String>) {
    if let Some(val) = z {
        if let Some(tier) = crate::web_ir::ZTier::from_str(val) {
            // ADR 034: Map tiers to fixed Tailwind z-index numbers.
            // Within-tier ordering is deliberately unspecified.
            let z_idx = match tier {
                crate::web_ir::ZTier::Background => 0,
                crate::web_ir::ZTier::Content => 10,
                crate::web_ir::ZTier::Chrome => 20,
                crate::web_ir::ZTier::Popover => 30,
                crate::web_ir::ZTier::Modal => 40,
                crate::web_ir::ZTier::Toast => 50,
                crate::web_ir::ZTier::SystemOverlay => 100,
            };
            classes.push(format!("z-[{z_idx}]"));
        } else {
            // Tailwind JIT arbitrary z-index: z-[100]
            classes.push(format!("z-[{val}]"));
        }
    }
}

/// VUV-4 conflict resolution: should a primitive's base class (e.g. `bg-background`, `rounded-lg`)
/// be suppressed because the author supplied a kwarg that addresses the same Tailwind axis?
///
/// Heuristic by axis. Examples:
///   - author `bg=…`  → drop primitive base classes matching `bg-*`.
///   - author `radius=…` → drop primitive base classes matching `rounded*`.
///   - author `color=…`  → drop primitive base classes matching `text-{color}` but NOT
///     `text-{size}` (xs/sm/lg/etc.) or alignment/transform tokens.
///
/// Conservative on ambiguous cases — if unsure, keep the primitive base class.
#[must_use]
pub fn primitive_base_class_overridden(base_class: &str, author_kwargs: &[&str]) -> bool {
    let has = |k: &str| author_kwargs.contains(&k);
    // Strip Tailwind state-variant modifier prefixes (`hover:`, `focus:`, `md:`, …) so an
    // author kwarg like `bg=` correctly overrides primitives' `hover:bg-…` defaults too.
    // Modifiers are colon-separated; the actual utility is whatever follows the last `:`.
    let utility = base_class.rsplit(':').next().unwrap_or(base_class);
    if has("bg") && utility.starts_with("bg-") {
        return true;
    }
    if has("color") {
        const SIZE_OR_NON_COLOR: &[&str] = &[
            "xs", "sm", "base", "lg", "xl", "2xl", "3xl", "4xl", "5xl", "6xl", "7xl", "8xl", "9xl",
            "left", "right", "center", "justify", "start", "end", "balance", "pretty", "wrap",
            "nowrap", "ellipsis", "clip",
        ];
        if let Some(rest) = utility.strip_prefix("text-")
            && !SIZE_OR_NON_COLOR.contains(&rest)
        {
            return true;
        }
    }
    let radius_like = [
        "radius",
        "radius_t",
        "radius_b",
        "radius_l",
        "radius_r",
        "radius_tl",
        "radius_tr",
        "radius_bl",
        "radius_br",
    ];
    if radius_like.iter().any(|k| has(k)) && utility.starts_with("rounded") {
        return true;
    }
    // Border kwargs split by intent: width vs color. Width kwargs own `border` and the
    // directional variants; color kwarg owns everything that ISN'T the bare width.
    let any_border_width_kwarg = has("border")
        || has("border_x")
        || has("border_y")
        || has("border_t")
        || has("border_b")
        || has("border_l")
        || has("border_r");
    if any_border_width_kwarg && (utility == "border" || utility.starts_with("border-")) {
        return true;
    }
    if has("border_color") && utility.starts_with("border-") && utility != "border" {
        return true;
    }
    let any_pad_kwarg = has("pad")
        || has("pad_x")
        || has("pad_y")
        || has("pad_t")
        || has("pad_b")
        || has("pad_l")
        || has("pad_r");
    if any_pad_kwarg
        && (utility.starts_with("p-")
            || utility.starts_with("px-")
            || utility.starts_with("py-")
            || utility.starts_with("pt-")
            || utility.starts_with("pb-")
            || utility.starts_with("pl-")
            || utility.starts_with("pr-"))
    {
        return true;
    }
    if has("w") && utility.starts_with("w-") {
        return true;
    }
    if has("h") && utility.starts_with("h-") {
        return true;
    }
    if has("max_w") && utility.starts_with("max-w-") {
        return true;
    }
    if has("max_h") && utility.starts_with("max-h-") {
        return true;
    }
    if has("font_family") && utility.starts_with("font-") {
        // Don't filter font-bold/font-medium/etc. (those are weight, owned by `weight` kwarg)
        const WEIGHTS: &[&str] = &[
            "font-thin",
            "font-extralight",
            "font-light",
            "font-normal",
            "font-medium",
            "font-semibold",
            "font-bold",
            "font-extrabold",
            "font-black",
        ];
        if !WEIGHTS.contains(&utility) {
            return true;
        }
    }
    false
}

/// VUV-4: resolve a single (kwarg, value) pair to its Tailwind class fragment(s), or `None` if
/// the kwarg is not a recognized universal style kwarg. Used by both the web_ir lowering pass
/// (via `apply_universal_kwargs`) and the codegen_ts AST-emit path so both produce identical
/// className output.
#[must_use]
pub fn resolve_universal_kwarg(kwarg: &str, value: &str) -> Option<Vec<String>> {
    let v = value.trim_matches('"').trim_matches('\'');
    let v_dashed = v.replace('.', "-");
    let out: Vec<String> = match kwarg {
        "gap" => vec![format!("gap-{v}")],
        "gap_x" => vec![format!("gap-x-{v}")],
        "gap_y" => vec![format!("gap-y-{v}")],
        "pad" => vec![format!("p-{v}")],
        "pad_x" => vec![format!("px-{v}")],
        "pad_y" => vec![format!("py-{v}")],
        "pad_t" => vec![format!("pt-{v}")],
        "pad_b" => vec![format!("pb-{v}")],
        "pad_l" => vec![format!("pl-{v}")],
        "pad_r" => vec![format!("pr-{v}")],
        "mb" => vec![format!("mb-{v}")],
        "mt" => vec![format!("mt-{v}")],
        "ml" => vec![format!("ml-{v}")],
        "mr" => vec![format!("mr-{v}")],
        "mx" => vec![format!("mx-{v}")],
        "my" => vec![format!("my-{v}")],
        "w" => vec![format!("w-{v}")],
        "h" => vec![format!("h-{v}")],
        "min_w" => vec![format!("min-w-{v}")],
        "min_h" => vec![format!("min-h-{v}")],
        "max_w" => vec![format!("max-w-{v}")],
        "max_h" => vec![format!("max-h-{v}")],
        "bg" => vec![format!("bg-{}", v_dashed)],
        "color" => vec![format!("text-{}", v_dashed)],
        "border" => match v {
            "" | "true" | "1" => vec!["border".to_string()],
            "false" | "0" => return Some(vec![]),
            _ => vec![format!("border-{v}")],
        },
        "border_x" => match v {
            "" | "true" | "1" => vec!["border-x".to_string()],
            "false" | "0" => return Some(vec![]),
            _ => vec![format!("border-x-{v}")],
        },
        "border_y" => match v {
            "" | "true" | "1" => vec!["border-y".to_string()],
            "false" | "0" => return Some(vec![]),
            _ => vec![format!("border-y-{v}")],
        },
        "border_t" => match v {
            "" | "true" | "1" => vec!["border-t".to_string()],
            "false" | "0" => return Some(vec![]),
            _ => vec![format!("border-t-{v}")],
        },
        "border_b" => match v {
            "" | "true" | "1" => vec!["border-b".to_string()],
            "false" | "0" => return Some(vec![]),
            _ => vec![format!("border-b-{v}")],
        },
        "border_l" => match v {
            "" | "true" | "1" => vec!["border-l".to_string()],
            "false" | "0" => return Some(vec![]),
            _ => vec![format!("border-l-{v}")],
        },
        "border_r" => match v {
            "" | "true" | "1" => vec!["border-r".to_string()],
            "false" | "0" => return Some(vec![]),
            _ => vec![format!("border-r-{v}")],
        },
        "border_color" => vec![format!("border-{}", v_dashed)],
        "radius" => vec![format!("rounded-{v}")],
        "radius_t" => vec![format!("rounded-t-{v}")],
        "radius_b" => vec![format!("rounded-b-{v}")],
        "radius_l" => vec![format!("rounded-l-{v}")],
        "radius_r" => vec![format!("rounded-r-{v}")],
        "radius_tl" => vec![format!("rounded-tl-{v}")],
        "radius_tr" => vec![format!("rounded-tr-{v}")],
        "radius_bl" => vec![format!("rounded-bl-{v}")],
        "radius_br" => vec![format!("rounded-br-{v}")],
        "overflow" => vec![format!("overflow-{v}")],
        "overflow_x" => vec![format!("overflow-x-{v}")],
        "overflow_y" => vec![format!("overflow-y-{v}")],
        "flex" => match v {
            "1" | "true" => vec!["flex-1".to_string()],
            _ => vec![format!("flex-{v}")],
        },
        "shrink" => match v {
            "0" => vec!["shrink-0".to_string()],
            _ => vec![format!("shrink-{v}")],
        },
        "grow" => match v {
            "0" => vec!["grow-0".to_string()],
            _ => vec![format!("grow-{v}")],
        },
        // `justify`/`items` only have an effect on flex containers. The primitive base
        // classes (`row`/`column`/`stack` already include `flex`); on non-flex primitives
        // (`panel`, `card`) callers should add `flex=true` explicitly. We emit the bare
        // utility class and let the primitive own the `flex` decision — duplicating `flex`
        // here produces ugly className arrays without semantic gain.
        "justify" => vec![format!("justify-{v}")],
        "items" => vec![format!("items-{v}")],
        "tracking" => vec![format!("tracking-{v}")],
        "leading" => vec![format!("leading-{v}")],
        "case" => match v {
            "upper" | "uppercase" => vec!["uppercase".to_string()],
            "lower" | "lowercase" => vec!["lowercase".to_string()],
            "normal" => vec!["normal-case".to_string()],
            _ => return None,
        },
        "italic" => match v {
            "true" | "" => vec!["italic".to_string()],
            "false" => vec!["not-italic".to_string()],
            _ => return None,
        },
        "font_family" => match v {
            "mono" => vec!["font-mono".to_string()],
            "sans" => vec!["font-sans".to_string()],
            "serif" => vec!["font-serif".to_string()],
            _ => return None,
        },
        "position" => vec![v.to_string()],
        "inset" => vec![format!("inset-{v}")],
        "top" => vec![format!("top-{v}")],
        "bottom" => vec![format!("bottom-{v}")],
        "left" => vec![format!("left-{v}")],
        "right" => vec![format!("right-{v}")],
        "shadow" => match v {
            "" | "true" => vec!["shadow".to_string()],
            _ => vec![format!("shadow-{v}")],
        },
        "opacity" => vec![format!("opacity-{v}")],
        "z" => {
            if let Some(tier) = crate::web_ir::ZTier::from_str(v) {
                let z_idx = match tier {
                    crate::web_ir::ZTier::Background => 0,
                    crate::web_ir::ZTier::Content => 10,
                    crate::web_ir::ZTier::Chrome => 20,
                    crate::web_ir::ZTier::Popover => 30,
                    crate::web_ir::ZTier::Modal => 40,
                    crate::web_ir::ZTier::Toast => 50,
                    crate::web_ir::ZTier::SystemOverlay => 100,
                };
                vec![format!("z-[{z_idx}]")]
            } else {
                vec![format!("z-[{v}]")]
            }
        }
        "raw_class" => v
            .split_whitespace()
            .map(std::string::ToString::to_string)
            .collect(),
        // Note: `safe_area` is listed in UNIVERSAL_STYLE_KWARGS (so it's consumed and
        // doesn't leak as a raw HTML attr) but is NOT handled here — it emits inline
        // CSS env() via `safe_area_to_style_props()` in the hir_emit path instead.
        _ => return None,
    };
    Some(out)
}

/// D1: Map a `safe_area` kwarg value to an inline React `style` object fragment string.
///
/// Returns a comma-separated list of `key: 'value'` pairs suitable for embedding inside
/// `style={{ … }}`. Returns an empty string for unrecognised values.
///
/// CSS `env()` variables are not expressible as Tailwind utilities, so this bypasses the
/// className path and writes a `style` attribute directly.
#[must_use]
pub fn safe_area_to_style_props(value: &str) -> String {
    match value.trim_matches('"').trim_matches('\'') {
        "top" => "paddingTop: 'env(safe-area-inset-top)'".into(),
        "bottom" => "paddingBottom: 'env(safe-area-inset-bottom)'".into(),
        "left" => "paddingLeft: 'env(safe-area-inset-left)'".into(),
        "right" => "paddingRight: 'env(safe-area-inset-right)'".into(),
        "all" => "paddingTop: 'env(safe-area-inset-top)', paddingRight: 'env(safe-area-inset-right)', paddingBottom: 'env(safe-area-inset-bottom)', paddingLeft: 'env(safe-area-inset-left)'".into(),
        "horizontal" => "paddingLeft: 'env(safe-area-inset-left)', paddingRight: 'env(safe-area-inset-right)'".into(),
        "vertical" => "paddingTop: 'env(safe-area-inset-top)', paddingBottom: 'env(safe-area-inset-bottom)'".into(),
        _ => String::new(),
    }
}

/// VUV-4: apply the universal style kwargs to the class list. Each kwarg maps to a Tailwind utility
/// using a small lookup table. Token-shaped values (e.g. `zinc.400`) are converted to dash-form
/// (`zinc-400`) so authors can write `bg: zinc.400` and the lowering produces `bg-zinc-400`.
///
/// `raw_class` is special: its value is added verbatim, intended as a transitional escape hatch
/// during the JSX-to-VUV cutover. It will be retired in favour of full typed-kwarg coverage.
fn apply_universal_kwargs(attrs: &[(String, String)], classes: &mut Vec<String>) {
    for (k, v) in attrs {
        if let Some(more) = resolve_universal_kwarg(k, v) {
            classes.extend(more);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn stack_emits_flex_col() {
        let e = resolve("stack", &[]).unwrap();
        assert_eq!(e.html_tag, "div");
        assert!(e.base_classes.contains(&"flex".to_string()));
        assert!(e.base_classes.contains(&"flex-col".to_string()));
    }

    #[test]
    fn stack_with_gap_appends_gap_class() {
        let e = resolve("stack", &attrs(&[("gap", "4")])).unwrap();
        assert!(
            e.base_classes.contains(&"gap-4".to_string()),
            "{:?}",
            e.base_classes
        );
    }

    #[test]
    fn row_emits_flex_row() {
        let e = resolve("row", &[]).unwrap();
        assert_eq!(e.html_tag, "div");
        assert!(e.base_classes.contains(&"flex-row".to_string()));
        assert!(!e.base_classes.contains(&"flex-wrap".to_string()));
    }

    #[test]
    fn row_with_wrap_adds_flex_wrap() {
        let e = resolve("row", &attrs(&[("wrap", "true")])).unwrap();
        assert!(e.base_classes.contains(&"flex-wrap".to_string()));
    }

    #[test]
    fn text_emits_p() {
        let e = resolve("text", &[]).unwrap();
        assert_eq!(e.html_tag, "p");
    }

    #[test]
    fn text_with_size_sm() {
        let e = resolve("text", &attrs(&[("size", "sm")])).unwrap();
        assert!(e.base_classes.contains(&"text-sm".to_string()));
    }

    #[test]
    fn heading_level_1_emits_h1() {
        let e = resolve("heading", &attrs(&[("level", "1")])).unwrap();
        assert_eq!(e.html_tag, "h1");
        assert!(e.base_classes.contains(&"text-3xl".to_string()));
    }

    #[test]
    fn heading_default_level_emits_h2() {
        let e = resolve("heading", &[]).unwrap();
        assert_eq!(e.html_tag, "h2");
    }

    #[test]
    fn button_default_variant_has_primary_classes() {
        let e = resolve("button", &[]).unwrap();
        assert_eq!(e.html_tag, "button");
        assert!(e.base_classes.contains(&"bg-primary".to_string()));
        assert!(
            e.base_classes
                .contains(&"text-primary-foreground".to_string())
        );
    }

    #[test]
    fn button_outline_variant() {
        let e = resolve("button", &attrs(&[("variant", "outline")])).unwrap();
        assert!(e.base_classes.contains(&"border".to_string()));
        assert!(e.base_classes.contains(&"bg-background".to_string()));
    }

    #[test]
    fn link_emits_a_with_primary_color() {
        let e = resolve("link", &[]).unwrap();
        assert_eq!(e.html_tag, "a");
        assert!(e.base_classes.contains(&"text-primary".to_string()));
    }

    #[test]
    fn panel_emits_with_aria_region() {
        let e = resolve("panel", &[]).unwrap();
        assert_eq!(e.html_tag, "div");
        assert_eq!(e.aria_role, Some("region"));
    }

    #[test]
    fn card_emits_card_bg() {
        let e = resolve("card", &[]).unwrap();
        assert!(e.base_classes.contains(&"bg-card".to_string()));
    }

    #[test]
    fn list_emits_ul() {
        let e = resolve("list", &[]).unwrap();
        assert_eq!(e.html_tag, "ul");
    }

    #[test]
    fn route_outlet_emits_main_role() {
        let e = resolve("route_outlet", &[]).unwrap();
        assert_eq!(e.aria_role, Some("main"));
    }

    #[test]
    fn unknown_tag_returns_none() {
        assert!(resolve("div", &[]).is_none());
        assert!(resolve("span", &[]).is_none());
        assert!(resolve("custom-unknown", &[]).is_none());
    }

    #[test]
    fn is_primitive_recognizes_all_10() {
        for tag in &[
            "stack",
            "row",
            "column",
            "text",
            "button",
            "link",
            "panel",
            "card",
            "list",
            "route_outlet",
        ] {
            assert!(is_primitive(tag), "{tag} should be a primitive");
        }
        assert!(!is_primitive("div"));
        assert!(!is_primitive("span"));
    }

    #[test]
    fn class_string_joins_with_space() {
        let e = resolve("stack", &[]).unwrap();
        let s = e.class_string();
        assert!(s.contains("flex"), "{s}");
        assert!(s.contains("flex-col"), "{s}");
    }

    // VUV-4: typed universal style kwargs.

    #[test]
    fn vuv_padding_kwargs_emit_tailwind() {
        let e = resolve("row", &attrs(&[("pad_x", "4"), ("pad_y", "2")])).unwrap();
        assert!(
            e.base_classes.contains(&"px-4".to_string()),
            "{:?}",
            e.base_classes
        );
        assert!(
            e.base_classes.contains(&"py-2".to_string()),
            "{:?}",
            e.base_classes
        );
    }

    #[test]
    fn vuv_margin_kwargs_emit_tailwind() {
        let e = resolve("text", &attrs(&[("mb", "2"), ("mt", "4")])).unwrap();
        assert!(e.base_classes.contains(&"mb-2".to_string()));
        assert!(e.base_classes.contains(&"mt-4".to_string()));
    }

    #[test]
    fn vuv_color_token_value_dashes_to_tailwind() {
        // Token-shaped value `zinc.400` is converted to `zinc-400` and prefixed with `text-`.
        let e = resolve("text", &attrs(&[("color", "zinc.400")])).unwrap();
        assert!(
            e.base_classes.contains(&"text-zinc-400".to_string()),
            "{:?}",
            e.base_classes
        );
    }

    #[test]
    fn vuv_bg_token_value_emits_bg_class() {
        let e = resolve("panel", &attrs(&[("bg", "blue.600")])).unwrap();
        assert!(
            e.base_classes.contains(&"bg-blue-600".to_string()),
            "{:?}",
            e.base_classes
        );
    }

    #[test]
    fn vuv_radius_kwargs_emit_rounded_classes() {
        let e = resolve("panel", &attrs(&[("radius", "2xl"), ("radius_br", "sm")])).unwrap();
        assert!(
            e.base_classes.contains(&"rounded-2xl".to_string()),
            "{:?}",
            e.base_classes
        );
        assert!(e.base_classes.contains(&"rounded-br-sm".to_string()));
    }

    #[test]
    fn vuv_max_w_min_h_emit_size_classes() {
        let e = resolve("panel", &attrs(&[("max_w", "xl"), ("min_h", "16")])).unwrap();
        assert!(e.base_classes.contains(&"max-w-xl".to_string()));
        assert!(e.base_classes.contains(&"min-h-16".to_string()));
    }

    #[test]
    fn vuv_flex_1_and_shrink_0() {
        let e = resolve("row", &attrs(&[("flex", "1"), ("shrink", "0")])).unwrap();
        assert!(e.base_classes.contains(&"flex-1".to_string()));
        assert!(e.base_classes.contains(&"shrink-0".to_string()));
    }

    #[test]
    fn vuv_case_upper_emits_uppercase() {
        let e = resolve("text", &attrs(&[("case", "upper")])).unwrap();
        assert!(e.base_classes.contains(&"uppercase".to_string()));
    }

    #[test]
    fn vuv_tracking_and_leading() {
        let e = resolve(
            "text",
            &attrs(&[("tracking", "widest"), ("leading", "snug")]),
        )
        .unwrap();
        assert!(e.base_classes.contains(&"tracking-widest".to_string()));
        assert!(e.base_classes.contains(&"leading-snug".to_string()));
    }

    #[test]
    fn vuv_justify_and_items_emit_flex_classes() {
        let e = resolve(
            "row",
            &attrs(&[("justify", "between"), ("items", "center")]),
        )
        .unwrap();
        assert!(e.base_classes.contains(&"justify-between".to_string()));
        assert!(e.base_classes.contains(&"items-center".to_string()));
    }

    #[test]
    fn vuv_raw_class_passes_value_verbatim() {
        let e = resolve(
            "panel",
            &attrs(&[("raw_class", "border-white/10 backdrop-blur-md")]),
        )
        .unwrap();
        assert!(e.base_classes.contains(&"border-white/10".to_string()));
        assert!(e.base_classes.contains(&"backdrop-blur-md".to_string()));
    }

    #[test]
    fn vuv_unknown_kwarg_passes_through_to_attrs() {
        // An unknown kwarg should NOT be consumed here; it stays in attrs and (if not in
        // PRIMITIVE_CONSUMED_PROPS) leaks through as an HTML attribute. This is the desired
        // failure mode — typos surface as visible HTML rather than silently disappear.
        let e = resolve("row", &attrs(&[("padd", "4")])).unwrap();
        assert!(
            !e.base_classes.iter().any(|c| c.contains("padd")),
            "{:?}",
            e.base_classes
        );
    }
}
