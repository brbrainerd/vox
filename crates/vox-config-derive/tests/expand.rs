//! Behavior tests for `#[derive(VoxConfig)]`. The test struct uses a *manual*
//! Default (like OrchestratorConfig) so the divergence guard is meaningful.

use vox_config::VoxConfigDomain;

// --- Coverage for the Option<T> and Parse (enum/FromStr) resolver branches, which
// no scalar test or orchestrator field exercises. Forces those quote! paths to compile.
#[derive(Clone, Debug, PartialEq, Default)]
enum Mode {
    #[default]
    Economy,
    Fast,
}
impl std::str::FromStr for Mode {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        match s {
            "Economy" => Ok(Mode::Economy),
            "Fast" => Ok(Mode::Fast),
            _ => Err(()),
        }
    }
}

#[derive(Clone, vox_config_derive::VoxConfig)]
#[vox_config(prefix = "VOX_TD2", group = "Tuning")]
struct TestDomain2 {
    #[config(env = "VOX_TD2_NAME", default = "")]
    name: Option<String>,
    #[config(default = "Economy")]
    mode: Mode,
}
impl Default for TestDomain2 {
    fn default() -> Self {
        Self {
            name: None,
            mode: Mode::Economy,
        }
    }
}

#[test]
fn option_and_enum_resolvers_compile_and_work() {
    let d = TestDomain2::default();
    assert_eq!(d.name, None);
    assert_eq!(d.mode, Mode::Economy);
    assert_eq!(TestDomain2::config_keys().len(), 2);

    unsafe {
        std::env::set_var("VOX_TD2_NAME", "hello");
        std::env::set_var("VOX_TD2_MODE", "Fast");
    }
    let c = TestDomain2::from_env_uncached();
    assert_eq!(c.name, Some("hello".to_string()));
    assert_eq!(c.mode, Mode::Fast);
    unsafe {
        std::env::remove_var("VOX_TD2_NAME");
        std::env::remove_var("VOX_TD2_MODE");
    }
}

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
    assert!(
        keys.iter()
            .all(|k| k.group == vox_config::config_key::Group::Tuning)
    );
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
    let mt = cat
        .iter()
        .find(|f| f.key == "VOX_TESTDOMAIN_MAX_THINGS")
        .unwrap();
    assert_eq!(mt.current, "7");
    assert_eq!(mt.default, "3");
}
