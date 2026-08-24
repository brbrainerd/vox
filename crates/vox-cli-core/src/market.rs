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
}
