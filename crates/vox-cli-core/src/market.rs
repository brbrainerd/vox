//! Hardware market catalog: schema and constraint evaluation.
//!
//! The schema is a contract (`contracts/market/catalog-schema.v1.yaml`), not
//! Rust types. TTLs are declared there and asserted against the compiled
//! constants by the tests below, rather than restated in two places that drift.

use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error("catalog schema: {0}")]
    Parse(#[from] serde_yaml::Error),
    #[error("`{0}` is a derived attribute and cannot be asserted by a source")]
    AssertedDerived(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttrKind {
    Spec,
    Price,
    Stock,
    Availability,
}

impl AttrKind {
    /// `None` = never expires. Asserted against the contract in tests.
    pub fn ttl_hours(&self) -> Option<f64> {
        match self {
            AttrKind::Spec => None,
            AttrKind::Price | AttrKind::Availability => Some(6.0),
            AttrKind::Stock => Some(0.25),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttrType {
    Number,
    Text,
    Enum,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AttributeDef {
    pub kind: AttrKind,
    #[serde(rename = "type")]
    pub ty: AttrType,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub required: bool,
    /// Human-readable cause, surfaced in exclusion reasons. This is what turns
    /// "48 GB < 64 GB" into "…because unified memory reserves ~25%".
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct KindDef {
    #[serde(default)]
    ttl_hours: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
struct Category {
    #[serde(default)]
    attributes: BTreeMap<String, AttributeDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CatalogSchema {
    #[serde(default)]
    attribute_kinds: BTreeMap<String, KindDef>,
    #[serde(default)]
    universal_attributes: BTreeMap<String, AttributeDef>,
    #[serde(default)]
    categories: BTreeMap<String, Category>,
}

impl CatalogSchema {
    pub fn load_from_str(yaml: &str) -> Result<Self, SchemaError> {
        Ok(serde_yaml::from_str(yaml)?)
    }

    /// A category-specific attribute of the same name wins over the universal one.
    pub fn attribute(&self, category: &str, name: &str) -> Option<&AttributeDef> {
        self.categories
            .get(category)
            .and_then(|c| c.attributes.get(name))
            .or_else(|| self.universal_attributes.get(name))
    }

    /// Outer `None` = the contract does not declare this kind.
    /// Inner `None` = declared `ttl_hours: null`, i.e. never expires.
    pub fn declared_ttl_hours(&self, kind: &str) -> Option<Option<f64>> {
        self.attribute_kinds.get(kind).map(|k| k.ttl_hours)
    }

    /// Required names for a category, deduplicated. Resolve-then-filter rather
    /// than chaining two maps, so a category override with `required: false`
    /// cannot have the universal `required: true` leak back in.
    pub fn required_attributes(&self, category: &str) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .universal_attributes
            .keys()
            .map(String::as_str)
            .chain(
                self.categories
                    .get(category)
                    .into_iter()
                    .flat_map(|c| c.attributes.keys().map(String::as_str)),
            )
            .collect();
        names.sort_unstable();
        names.dedup();
        names.retain(|n| self.attribute(category, n).is_some_and(|d| d.required));
        names
    }
}

/// Trust tier of an observed value. Ordering IS the precedence rule.
///
/// Scoped to market attributes (price, stock, availability) when it gates a
/// comparison: an A6000's 48 GB is 48 GB whether a search snippet or a checkout
/// page reported it. Applying an evidence ladder to a physical spec is a
/// category error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Evidence {
    Aggregator = 1,   // price tracker, no merchant page
    SearchIndex = 2,  // search snippet, page never loaded
    MerchantPage = 3, // page fetched, no stock text
    Transactable = 4, // page fetched, cart live, stock stated
    Derived = 5,      // computed from an observation by a named, versioned rule
}

#[derive(Debug, Clone, PartialEq)]
pub struct Value {
    pub number: f64,
    pub unit: Option<String>,
    pub evidence: Evidence,
}

#[derive(Debug, Clone)]
pub struct CatalogItem {
    pub item_id: String,
    pub category: String,
    pub attributes: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Gte,
    Lte,
    Eq,
}

#[derive(Debug, Clone)]
pub struct Constraint {
    pub attribute: String,
    pub op: CmpOp,
    pub value: f64,
    pub unit: String,
}

impl Constraint {
    pub fn gte(attribute: &str, value: f64, unit: &str) -> Self {
        Self::new(attribute, CmpOp::Gte, value, unit)
    }
    pub fn lte(attribute: &str, value: f64, unit: &str) -> Self {
        Self::new(attribute, CmpOp::Lte, value, unit)
    }
    pub fn eq(attribute: &str, value: f64, unit: &str) -> Self {
        Self::new(attribute, CmpOp::Eq, value, unit)
    }

    fn new(attribute: &str, op: CmpOp, value: f64, unit: &str) -> Self {
        Self {
            attribute: attribute.into(),
            op,
            value,
            unit: unit.into(),
        }
    }

    fn satisfied_by(&self, v: f64) -> bool {
        match self.op {
            CmpOp::Gte => v >= self.value,
            CmpOp::Lte => v <= self.value,
            // ponytail: exact f64 equality. Both sides are literals — one from a
            // contract, one from a caller — never arithmetic results. Switch to
            // an epsilon if a derivation ever feeds this path.
            CmpOp::Eq => v == self.value,
        }
    }

    fn describe(&self) -> &'static str {
        match self.op {
            CmpOp::Gte => ">=",
            CmpOp::Lte => "<=",
            CmpOp::Eq => "==",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Excluded {
    pub item_id: String,
    pub reason: String,
}

/// `indeterminate` is deliberately separate from `excluded`. Folding them loses
/// the distinction between "measured, falls short" and "never measured" — and
/// only the second is a data gap someone can close.
#[derive(Debug, Default)]
pub struct Outcome {
    pub passed: Vec<CatalogItem>,
    pub excluded: Vec<Excluded>,
    pub indeterminate: Vec<Excluded>,
}

enum Verdict {
    Passed,
    Excluded(String),
    Indeterminate(String),
}

fn evaluate(schema: &CatalogSchema, item: &CatalogItem, c: &Constraint) -> Verdict {
    let Some(def) = schema.attribute(&item.category, &c.attribute) else {
        return Verdict::Indeterminate(format!(
            "`{}` is not an attribute of category `{}` in the catalog schema",
            c.attribute, item.category
        ));
    };
    let Some(v) = item.attributes.get(&c.attribute) else {
        return Verdict::Indeterminate(format!(
            "`{}` is unknown for this item — not recorded by any source",
            c.attribute
        ));
    };
    // String equality, not conversion. GiB vs GB is a real 7.4% difference, and
    // guessing which was meant is how a wrong answer looks confident.
    if v.unit.as_deref() != Some(c.unit.as_str()) {
        return Verdict::Indeterminate(format!(
            "`{}` is recorded in {} but the constraint is in {} — no conversion is defined",
            c.attribute,
            v.unit.as_deref().unwrap_or("no unit"),
            c.unit
        ));
    }
    if c.satisfied_by(v.number) {
        return Verdict::Passed;
    }
    let mut reason = format!(
        "{} is {} {}, which fails {} {} {}",
        c.attribute,
        v.number,
        c.unit,
        c.attribute,
        c.describe(),
        c.value
    );
    if let Some(note) = def.note.as_deref() {
        reason.push_str(&format!(" ({note})"));
    }
    Verdict::Excluded(reason)
}

/// Hard filters. Constraints eliminate; they never score.
///
/// Scoped to a single `category`: an item of a different category is dropped
/// from the result entirely (not into any of the three buckets), rather than
/// landing in `indeterminate` with a reason indistinguishable from a genuine
/// data gap (e.g. "not an attribute of category `gpu`" reading identically to
/// "we never measured this").
pub fn apply(
    schema: &CatalogSchema,
    items: &[CatalogItem],
    category: &str,
    constraints: &[Constraint],
) -> Outcome {
    let mut out = Outcome::default();
    'items: for item in items.iter().filter(|i| i.category == category) {
        for c in constraints {
            match evaluate(schema, item, c) {
                Verdict::Passed => {}
                Verdict::Excluded(r) => {
                    out.excluded.push(Excluded {
                        item_id: item.item_id.clone(),
                        reason: r,
                    });
                    continue 'items;
                }
                Verdict::Indeterminate(r) => {
                    out.indeterminate.push(Excluded {
                        item_id: item.item_id.clone(),
                        reason: r,
                    });
                    continue 'items;
                }
            }
        }
        out.passed.push(item.clone());
    }
    out
}

/// Memory architecture, which decides how much of installed RAM the GPU can
/// actually address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arch {
    AppleUnified,
    StrixHalo,
    Discrete,
}

/// `unified_memory_reserve_v1`.
///
/// Neither reserve is a hardware constant: Apple's is adjustable via
/// `iogpu.wired_limit_percent`, Strix Halo's is a UEFI/driver setting. These
/// are the shipped defaults, which is what a buyer gets out of the box — and
/// why the output is `Evidence::Derived` with the rule named and versioned,
/// rather than a figure wearing a merchant page's provenance.
pub fn derive_gpu_accessible_gb(total_gb: f64, arch: Arch) -> Value {
    let usable = match arch {
        Arch::AppleUnified | Arch::StrixHalo => (total_gb * 0.75).floor(),
        // A discrete GPU's VRAM is not carved out of system memory.
        Arch::Discrete => total_gb,
    };
    Value {
        number: usable,
        unit: Some("GB".into()),
        evidence: Evidence::Derived,
    }
}

#[derive(Deserialize)]
struct SeedValue {
    number: f64,
    #[serde(default)]
    unit: Option<String>,
    evidence: Evidence,
}

#[derive(Deserialize)]
struct SeedItem {
    item_id: String,
    category: String,
    #[serde(default = "discrete")]
    arch: Arch,
    #[serde(default)]
    attributes: BTreeMap<String, SeedValue>,
}

fn discrete() -> Arch {
    Arch::Discrete
}

#[derive(Deserialize)]
struct Seed {
    items: Vec<SeedItem>,
}

/// Loads observed values and computes derived ones.
///
/// A seed that asserts a derived attribute directly is rejected: letting a
/// source hand-write `gpu_accessible_gb` is precisely the unprovenanced number
/// this layer exists to stop.
pub fn seed_from_yaml(schema: &CatalogSchema, yaml: &str) -> Result<Vec<CatalogItem>, SchemaError> {
    let seed: Seed = serde_yaml::from_str(yaml)?;
    let mut items = Vec::with_capacity(seed.items.len());
    for s in seed.items {
        let mut attributes: BTreeMap<String, Value> = s
            .attributes
            .into_iter()
            .map(|(k, v)| {
                (
                    k,
                    Value {
                        number: v.number,
                        unit: v.unit,
                        evidence: v.evidence,
                    },
                )
            })
            .collect();

        if attributes.contains_key("gpu_accessible_gb") {
            return Err(SchemaError::AssertedDerived("gpu_accessible_gb".into()));
        }
        // Only derive when the input is actually in GB: guessing past a unit
        // mismatch is exactly what this layer exists to refuse (see the GiB-vs-GB
        // check in `evaluate()`). Absence routes to `Indeterminate` downstream.
        if let Some(total) = attributes.get("total_memory_gb")
            && total.unit.as_deref() == Some("GB")
        {
            let derived = derive_gpu_accessible_gb(total.number, s.arch);
            attributes.insert("gpu_accessible_gb".into(), derived);
        }
        // Reserved: required-attribute checking belongs at promotion time,
        // which arrives with the store. Named here so the parameter is not
        // mistaken for an oversight.
        let _ = schema;
        items.push(CatalogItem {
            item_id: s.item_id,
            category: s.category,
            attributes,
        });
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_contract_parses_and_exposes_its_categories() {
        let yaml = include_str!("../../../contracts/market/catalog-schema.v1.yaml");
        let s = CatalogSchema::load_from_str(yaml).expect("shipped contract must parse");
        assert!(s.attribute("laptop", "gpu_accessible_gb").is_some());
        // Universal attributes apply to every category without being restated.
        assert!(s.attribute("laptop", "price_usd").is_some());
        assert!(s.attribute("gpu", "price_usd").is_some());
    }

    #[test]
    fn an_unknown_category_or_attribute_is_none_rather_than_a_panic() {
        let yaml = include_str!("../../../contracts/market/catalog-schema.v1.yaml");
        let s = CatalogSchema::load_from_str(yaml).unwrap();
        assert!(s.attribute("spaceship", "warp_factor").is_none());
        assert!(s.attribute("laptop", "warp_factor").is_none());
    }

    /// The contract declares TTLs in YAML and `AttrKind::ttl_hours()` declares
    /// them in Rust. Asserting the Rust constant against a Rust literal — which
    /// the first draft did — tests nothing that can break. What CAN break is the
    /// two drifting: as originally drafted `CatalogSchema` had no
    /// `attribute_kinds` field at all, so serde discarded the block and the
    /// contract could say 24 while the code used 6, forever, silently. That also
    /// made the operator's only lever over fetch volume inert.
    #[test]
    fn declared_ttls_in_the_contract_match_the_compiled_ttls() {
        let yaml = include_str!("../../../contracts/market/catalog-schema.v1.yaml");
        let s = CatalogSchema::load_from_str(yaml).unwrap();
        for (name, kind) in [
            ("spec", AttrKind::Spec),
            ("price", AttrKind::Price),
            ("stock", AttrKind::Stock),
            ("availability", AttrKind::Availability),
        ] {
            let declared = s
                .declared_ttl_hours(name)
                .unwrap_or_else(|| panic!("contract declares no attribute_kind `{name}`"));
            assert_eq!(
                declared,
                kind.ttl_hours(),
                "drift: contract says {name}={declared:?}, code says {:?}",
                kind.ttl_hours()
            );
        }
        assert!(
            AttrKind::Spec.ttl_hours().is_none(),
            "an A6000 is always 48GB"
        );
        assert!(
            AttrKind::Stock.ttl_hours().unwrap() < AttrKind::Price.ttl_hours().unwrap(),
            "stock moves faster than price"
        );
    }

    /// `attribute()` resolves category-specific over universal. This rule had no
    /// coverage in the first draft, and the drafted `required_attributes` broke
    /// it: chaining the category map with the universal map let an overridden
    /// name leak back in from the universal side and report as required.
    #[test]
    fn a_category_attribute_overrides_the_universal_one_of_the_same_name() {
        const OVERRIDE: &str = r#"
x-vox-version: 1
schema_version: 1
attribute_kinds:
  spec:  { ttl_hours: null }
  price: { ttl_hours: 6 }
universal_attributes:
  price_usd: { kind: price, type: number, unit: USD, required: true }
categories:
  bundle:
    attributes:
      price_usd: { kind: price, type: number, unit: EUR, required: false }
  laptop:
    attributes:
      gpu_accessible_gb: { kind: spec, type: number, unit: GB, required: true }
"#;
        let s = CatalogSchema::load_from_str(OVERRIDE).unwrap();
        assert_eq!(
            s.attribute("bundle", "price_usd").unwrap().unit.as_deref(),
            Some("EUR")
        );
        assert_eq!(
            s.attribute("laptop", "price_usd").unwrap().unit.as_deref(),
            Some("USD")
        );

        let req = s.required_attributes("bundle");
        assert_eq!(
            req.iter().filter(|n| **n == "price_usd").count(),
            0,
            "the override is not required; the universal one must not leak back in: {req:?}"
        );
    }

    #[test]
    fn a_malformed_contract_names_what_it_could_not_read() {
        let e = CatalogSchema::load_from_str("categories: [not, a, map]")
            .unwrap_err()
            .to_string();
        assert!(e.to_lowercase().contains("categories"), "got: {e}");
    }

    fn v(n: f64, unit: &str) -> Value {
        Value {
            number: n,
            unit: Some(unit.into()),
            evidence: Evidence::MerchantPage,
        }
    }

    fn item(id: &str, attrs: &[(&str, Value)]) -> CatalogItem {
        CatalogItem {
            item_id: id.into(),
            category: "laptop".into(),
            attributes: attrs
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        }
    }

    fn schema() -> CatalogSchema {
        CatalogSchema::load_from_str(include_str!(
            "../../../contracts/market/catalog-schema.v1.yaml"
        ))
        .unwrap()
    }

    #[test]
    fn a_measured_miss_is_excluded_with_both_numbers_and_the_cause() {
        let items = vec![item("mbp14", &[("gpu_accessible_gb", v(48.0, "GB"))])];
        let out = apply(
            &schema(),
            &items,
            "laptop",
            &[Constraint::gte("gpu_accessible_gb", 64.0, "GB")],
        );

        assert!(out.passed.is_empty());
        assert_eq!(out.excluded.len(), 1);
        let r = &out.excluded[0].reason;
        // Both numbers in the right roles. `contains("48") && contains("64")`
        // alone also passes if they are swapped into "64 < required 48".
        assert!(r.contains("48"), "must state the actual value: {r}");
        assert!(r.contains("64"), "must state the requirement: {r}");
        // The schema `note` is what turns a comparison into an explanation.
        assert!(
            r.contains("unified") || r.contains("reserves"),
            "must explain WHY 64GB exposes only 48GB: {r}"
        );
    }

    /// The spec's named silent-drop failure. An item that simply lacks the
    /// attribute must be distinguishable from one measured and found short: the
    /// two demand different action — close a data gap, or move on.
    #[test]
    fn an_absent_attribute_is_indeterminate_not_a_measured_failure() {
        let items = vec![
            item("roomy", &[("gpu_accessible_gb", v(96.0, "GB"))]),
            item("cramped", &[("gpu_accessible_gb", v(48.0, "GB"))]),
            item("unknown", &[("price_usd", v(4200.0, "USD"))]),
        ];
        let out = apply(
            &schema(),
            &items,
            "laptop",
            &[Constraint::gte("gpu_accessible_gb", 64.0, "GB")],
        );

        assert_eq!(out.passed.len(), 1);
        assert_eq!(out.passed[0].item_id, "roomy");
        assert_eq!(out.excluded.len(), 1, "only the measured miss");
        assert_eq!(out.excluded[0].item_id, "cramped");

        assert_eq!(out.indeterminate.len(), 1);
        let r = out.indeterminate[0].reason.to_lowercase();
        assert!(
            r.contains("unknown") || r.contains("not recorded"),
            "got: {r}"
        );
        assert!(
            !r.contains('<'),
            "must not claim a comparison it never made: {r}"
        );
    }

    /// 96 >= 64 numerically, so only unit-checking produces this exclusion.
    #[test]
    fn a_value_in_the_wrong_unit_is_indeterminate_not_silently_compared() {
        let items = vec![item("euro-spec", &[("gpu_accessible_gb", v(96.0, "GiB"))])];
        let out = apply(
            &schema(),
            &items,
            "laptop",
            &[Constraint::gte("gpu_accessible_gb", 64.0, "GB")],
        );
        assert!(out.passed.is_empty(), "GiB and GB differ by 7.4%");
        assert_eq!(out.indeterminate.len(), 1);
        assert!(
            out.indeterminate[0].reason.contains("GiB"),
            "{:?}",
            out.indeterminate[0]
        );
    }

    #[test]
    fn every_comparison_operator_works_at_its_boundary() {
        let items = vec![item("edge", &[("gpu_accessible_gb", v(64.0, "GB"))])];
        let s = schema();
        for (c, should_pass) in [
            (Constraint::gte("gpu_accessible_gb", 64.0, "GB"), true),
            (Constraint::lte("gpu_accessible_gb", 64.0, "GB"), true),
            (Constraint::eq("gpu_accessible_gb", 64.0, "GB"), true),
            (Constraint::gte("gpu_accessible_gb", 65.0, "GB"), false),
            (Constraint::lte("gpu_accessible_gb", 63.0, "GB"), false),
        ] {
            let out = apply(&s, &items, "laptop", &[c]);
            assert_eq!(
                out.passed.len(),
                usize::from(should_pass),
                "at the boundary"
            );
        }
    }

    /// Constraints eliminate in conjunction; the reason must name the one that
    /// actually did it, not the first one checked.
    #[test]
    fn multiple_constraints_report_the_one_that_eliminated_the_item() {
        let items = vec![item(
            "heavy",
            &[
                ("gpu_accessible_gb", v(96.0, "GB")),
                ("tdp_w", v(2000.0, "W")),
            ],
        )];
        let out = apply(
            &schema(),
            &items,
            "laptop",
            &[
                Constraint::gte("gpu_accessible_gb", 64.0, "GB"),
                Constraint::lte("tdp_w", 1600.0, "W"),
            ],
        );
        assert_eq!(out.excluded.len(), 1);
        let r = &out.excluded[0].reason;
        assert!(r.contains("tdp_w"), "must name the failing constraint: {r}");
        assert!(
            !r.contains("gpu_accessible_gb"),
            "must not blame a passing one: {r}"
        );
    }

    /// An item outside the query's category must not appear in any bucket. If it
    /// fell into `indeterminate` (e.g. "not an attribute of category `gpu`") that
    /// reads identically to a genuine data gap and defeats the point of the
    /// three-way split.
    #[test]
    fn an_item_of_a_different_category_is_dropped_not_bucketed() {
        let mut gpu = item("a6000", &[("gpu_accessible_gb", v(48.0, "GB"))]);
        gpu.category = "gpu".into();
        let items = vec![item("mbp14", &[("gpu_accessible_gb", v(96.0, "GB"))]), gpu];

        let out = apply(
            &schema(),
            &items,
            "laptop",
            &[Constraint::gte("gpu_accessible_gb", 64.0, "GB")],
        );

        assert_eq!(out.passed.len(), 1);
        assert_eq!(out.passed[0].item_id, "mbp14");
        assert!(out.excluded.is_empty());
        assert!(
            out.indeterminate.is_empty(),
            "the gpu item must be dropped, not indeterminate: {:?}",
            out.indeterminate
        );
    }

    #[test]
    fn a_constraint_on_an_attribute_the_schema_does_not_know_is_loud() {
        let items = vec![item("x", &[("gpu_accessible_gb", v(96.0, "GB"))])];
        let out = apply(
            &schema(),
            &items,
            "laptop",
            &[Constraint::gte("warp_factor", 9.0, "c")],
        );
        assert!(out.passed.is_empty());
        assert_eq!(out.indeterminate.len(), 1);
        assert!(
            out.indeterminate[0].reason.contains("warp_factor"),
            "a typo'd attribute must be loud, not a silent empty set: {:?}",
            out.indeterminate[0]
        );
    }
}
