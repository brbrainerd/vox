//! Behavior tests for `#[derive(VoxConfig)]`. The test struct uses a *manual*
//! Default (like OrchestratorConfig) so the divergence guard is meaningful.

use vox_config::VoxConfigDomain;

#[derive(Clone, vox_config_derive::VoxConfig)]
#[vox_config(prefix = "VOX_TESTDOMAIN", group = "Tuning")]
struct TestDomain {
    #[config(default = 3, label = "Max things")]
    max_things: u32,
    #[config(default = false)]
    verbose: bool,
    #[config(env = "VOX_LEGACY_NAME", default = "info")]
    log_level: String,
    #[config(skip)]
    backends: Vec<String>,
}

impl Default for TestDomain {
    fn default() -> Self {
        Self {
            max_things: 3,
            verbose: false,
            log_level: "info".to_string(),
            backends: Vec::new(),
        }
    }
}

#[test]
fn default_matches_config_default_attrs() {
    // C-divergence guard: #[config(default=X)] MUST equal the field's Default value.
    let d = TestDomain::default();
    assert_eq!(d.max_things, 3);
    assert_eq!(d.verbose, false);
    assert_eq!(d.log_level, "info");
}

#[test]
fn merge_env_reads_env() {
    // ponytail: single-threaded set_var; Rust 2024 requires unsafe.
    unsafe {
        std::env::set_var("VOX_TESTDOMAIN_MAX_THINGS", "9");
    }
    let mut c = TestDomain::default();
    c.merge_env();
    assert_eq!(c.max_things, 9);
    unsafe {
        std::env::remove_var("VOX_TESTDOMAIN_MAX_THINGS");
    }
}

#[test]
fn config_keys_cover_nonskip_fields_with_env_names() {
    let keys = TestDomain::config_keys();
    assert_eq!(keys.len(), 3, "skip field must be excluded");
    let names: Vec<_> = keys.iter().map(|k| k.key).collect();
    assert!(names.contains(&"VOX_TESTDOMAIN_MAX_THINGS"));
    assert!(names.contains(&"VOX_LEGACY_NAME")); // explicit env override honored
    assert!(!names.contains(&"VOX_TESTDOMAIN_BACKENDS")); // skipped
    assert!(keys.iter().all(|k| !k.secret));
    // group string mapped to the Tuning enum variant (not General).
    assert!(keys.iter().all(|k| k.group == vox_config::config_key::Group::Tuning));
}

#[test]
fn catalog_reports_current_and_default() {
    let c = TestDomain {
        max_things: 7,
        verbose: true,
        log_level: "debug".into(),
        backends: vec![],
    };
    let cat = c.catalog();
    let mt = cat.iter().find(|f| f.key == "VOX_TESTDOMAIN_MAX_THINGS").unwrap();
    assert_eq!(mt.current, "7");
    assert_eq!(mt.default, "3");
}
