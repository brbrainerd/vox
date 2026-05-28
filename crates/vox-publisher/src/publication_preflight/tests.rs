use super::*;
#[allow(clippy::module_inception)] // File is `tests.rs`; inner `mod tests` keeps test items grouped.
mod tests {
    use super::*;
    use crate::scientific_metadata::{ScientificAuthor, ScientificPublicationMetadata};
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn sample_manifest(f: impl FnOnce(&mut PublicationManifest)) -> PublicationManifest {
        let mut m = PublicationManifest {
            publication_id: "p".to_string(),
            content_type: "scientia".to_string(),
            source_ref: None,
            title: "Title".to_string(),
            author: "Ada Lovelace".to_string(),
            abstract_text: Some("Abstract.".to_string()),
            body_markdown: "Hello.".to_string(),
            citations_json: None,
            metadata_json: None,
        };
        f(&mut m);
        m
    }

    #[test]
    fn legacy_distribution_key_surfaces_manual_migration_hint() {
        let m = sample_manifest(|x| {
            x.metadata_json = Some(format!(
                r#"{{"{}": {{"rss": false}}, "topic_pack": null}}"#,
                crate::switching::LEGACY_METADATA_SYNDICATION_KEY
            ));
        });
        let r = run_preflight(&m, PreflightProfile::Default);
        assert!(
            r.manual_required
                .iter()
                .any(|e| e.code == "legacy_syndication_metadata_key"),
            "{:?}",
            r.manual_required
        );
    }

    #[test]
    fn ok_when_aligned_scientific_block() {
        let sci = ScientificPublicationMetadata {
            authors: vec![ScientificAuthor {
                name: "Ada Lovelace".to_string(),
                orcid: None,
                ror: None,
                affiliation: None,
            }],
            license_spdx: Some("Apache-2.0".to_string()),
            ..Default::default()
        };
        let meta =
            crate::scientific_metadata::build_scientia_metadata_json("t", None, Some(&sci), None)
                .unwrap();
        let m = sample_manifest(|x| x.metadata_json = Some(meta));
        let r = run_preflight(&m, PreflightProfile::Default);
        assert!(r.ok, "{:?}", r.findings);
        assert!(r.readiness_score >= 80);
    }

    #[test]
    fn error_on_author_mismatch() {
        let sci = ScientificPublicationMetadata {
            authors: vec![ScientificAuthor {
                name: "Someone Else".to_string(),
                orcid: None,
                ror: None,
                affiliation: None,
            }],
            license_spdx: Some("Apache-2.0".to_string()),
            ..Default::default()
        };
        let meta =
            crate::scientific_metadata::build_scientia_metadata_json("t", None, Some(&sci), None)
                .unwrap();
        let m = sample_manifest(|x| x.metadata_json = Some(meta));
        let r = run_preflight(&m, PreflightProfile::Default);
        assert!(!r.ok);
        assert!(
            r.findings
                .iter()
                .any(|f| f.code == "author_primary_mismatch")
        );
    }

    #[test]
    fn double_blind_flags_email() {
        let m = sample_manifest(|x| {
            x.body_markdown = "Contact me at lee@example.com.".to_string();
        });
        let r = run_preflight(&m, PreflightProfile::DoubleBlind);
        assert!(!r.ok);
        assert!(
            r.findings
                .iter()
                .any(|f| f.code == "double_blind_email_in_body")
        );
    }

    #[test]
    fn double_blind_flags_orcid_in_body() {
        let m = sample_manifest(|x| {
            x.body_markdown = "See also https://orcid.org/0000-0002-1825-0097".to_string();
        });
        let r = run_preflight(&m, PreflightProfile::DoubleBlind);
        assert!(!r.ok);
        assert!(
            r.findings
                .iter()
                .any(|f| f.code == "double_blind_orcid_url_in_body")
        );
    }

    #[test]
    fn metadata_complete_errors_without_scientific_block() {
        let m = sample_manifest(|_| {});
        let r = run_preflight(&m, PreflightProfile::MetadataComplete);
        assert!(!r.ok);
        assert!(
            r.findings
                .iter()
                .any(|f| f.code == "scientific_metadata_required")
        );
    }

    #[test]
    fn arxiv_assist_errors_without_abstract_but_not_missing_scientific_block() {
        let m = sample_manifest(|x| {
            x.abstract_text = None;
        });
        let r = run_preflight(&m, PreflightProfile::ArxivAssist);
        assert!(!r.ok);
        assert!(r.findings.iter().any(|f| f.code == "abstract_required"));
        assert!(
            !r.findings
                .iter()
                .any(|f| f.code == "scientific_metadata_required")
        );
    }

    #[test]
    fn metadata_complete_ok_when_fully_populated() {
        let sci = ScientificPublicationMetadata {
            authors: vec![ScientificAuthor {
                name: "Ada Lovelace".to_string(),
                orcid: None,
                ror: None,
                affiliation: None,
            }],
            license_spdx: Some("Apache-2.0".to_string()),
            ..Default::default()
        };
        let meta =
            crate::scientific_metadata::build_scientia_metadata_json("t", None, Some(&sci), None)
                .unwrap();
        let m = sample_manifest(|x| x.metadata_json = Some(meta));
        let r = run_preflight(&m, PreflightProfile::MetadataComplete);
        assert!(r.ok, "{:?}", r.findings);
    }

    #[test]
    fn worthiness_attached_when_contract_provided() {
        let sci = ScientificPublicationMetadata {
            authors: vec![ScientificAuthor {
                name: "Ada Lovelace".to_string(),
                orcid: None,
                ror: None,
                affiliation: None,
            }],
            license_spdx: Some("Apache-2.0".to_string()),
            ethics_and_impact: Some(crate::scientific_metadata::EthicsAndImpactAttestation {
                broader_impact_statement: Some("Low risk.".to_string()),
                irb_or_human_subjects_note: None,
            }),
            ..Default::default()
        };
        let meta =
            crate::scientific_metadata::build_scientia_metadata_json("t", None, Some(&sci), None)
                .unwrap();
        let mut m = sample_manifest(|x| {
            x.metadata_json = Some(meta);
            x.citations_json = Some("[{}]".to_string());
        });
        let yaml = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../contracts/scientia/publication-worthiness.default.yaml"
        ));
        let contract =
            crate::publication_worthiness::load_contract_from_str(yaml).expect("contract");
        let r = run_preflight_with_worthiness(&m, PreflightProfile::Default, &contract);
        assert!(r.worthiness.is_some());
        let w = r.worthiness.as_ref().expect("worthiness");
        assert_ne!(
            w.decision,
            crate::publication_worthiness::WorthinessDecision::Publish,
            "heuristic never claims Publish without meaningful_advance: {w:?}"
        );
        m.body_markdown = "Contact me at x@y.zz.".to_string();
        let r2 = run_preflight_with_worthiness(&m, PreflightProfile::DoubleBlind, &contract);
        assert!(!r2.ok);
        assert!(r2.worthiness.is_some());
    }

    #[test]
    fn next_actions_include_default_pipeline_and_social_simulation() {
        let m = sample_manifest(|x| {
            x.metadata_json = Some(
                r#"{
                    "syndication": {
                        "scholarly": ["zenodo"],
                        "channels": ["twitter"],
                        "channel_payloads": {
                            "twitter": {
                                "short_text": "hello"
                            }
                        }
                    }
                }"#
                .to_string(),
            );
        });
        let r = run_preflight(&m, PreflightProfile::Default);
        assert!(
            r.next_actions
                .iter()
                .any(|a| a.code == "run_default_scholarly_pipeline"),
            "{:?}",
            r.next_actions
        );
        assert!(
            r.next_actions
                .iter()
                .any(|a| a.code == "simulate_social_routing"),
            "{:?}",
            r.next_actions
        );
        assert!(
            r.next_actions
                .iter()
                .any(|a| a.code == "dry_run_social_publish"),
            "{:?}",
            r.next_actions
        );
    }

    #[test]
    #[allow(unsafe_code)]
    fn openreview_readiness_respects_secrets_strict_mode() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let openreview_token_key = "VOX_OPENREVIEW_ACCESS_TOKEN";
        let prev_token = std::env::var(openreview_token_key).ok();
        let prev_backend = std::env::var("VOX_SECRETS_BACKEND").ok();
        let prev_profile = std::env::var("VOX_SECRETS_PROFILE").ok();
        const DB_REMOTE_ALIAS_URL_ENV: &str = concat!("VOX_", "TURSO", "_URL");
        let prev_url = std::env::var(DB_REMOTE_ALIAS_URL_ENV).ok();
        let prev_cloudless_path = std::env::var("VOX_SECRETS_CLOUDLESS_DB_PATH").ok();
        let prev_account_id = std::env::var("VOX_ACCOUNT_ID").ok();
        unsafe {
            std::env::set_var("VOX_OPENREVIEW_ACCESS_TOKEN", "publisher-env-token");
            std::env::set_var("VOX_SECRETS_BACKEND", "vox_cloud");
            std::env::set_var("VOX_SECRETS_PROFILE", "dev");
            std::env::remove_var(DB_REMOTE_ALIAS_URL_ENV);
            let tmp = std::env::temp_dir().join("vox-secrets-publisher-strict-lenient.db");
            std::env::set_var(
                "VOX_SECRETS_CLOUDLESS_DB_PATH",
                tmp.to_string_lossy().to_string(),
            );
            std::env::set_var("VOX_ACCOUNT_ID", "publisher-strict-lenient-test");
        }
        let lenient = run_preflight(&sample_manifest(|_| {}), PreflightProfile::Default);
        let openreview_lenient = lenient
            .destination_readiness
            .iter()
            .find(|d| d.destination == "openreview")
            .expect("openreview readiness");
        assert!(openreview_lenient.ready);

        unsafe {
            std::env::set_var("VOX_SECRETS_PROFILE", "hard_cut");
            std::env::remove_var(DB_REMOTE_ALIAS_URL_ENV);
        }
        let strict = run_preflight(&sample_manifest(|_| {}), PreflightProfile::Default);
        let openreview_strict = strict
            .destination_readiness
            .iter()
            .find(|d| d.destination == "openreview")
            .expect("openreview readiness");
        assert!(!openreview_strict.ready);

        unsafe {
            match prev_token {
                Some(v) => std::env::set_var("VOX_OPENREVIEW_ACCESS_TOKEN", v),
                None => std::env::remove_var("VOX_OPENREVIEW_ACCESS_TOKEN"),
            }
            match prev_backend {
                Some(v) => std::env::set_var("VOX_SECRETS_BACKEND", v),
                None => std::env::remove_var("VOX_SECRETS_BACKEND"),
            }
            match prev_profile {
                Some(v) => std::env::set_var("VOX_SECRETS_PROFILE", v),
                None => std::env::remove_var("VOX_SECRETS_PROFILE"),
            }
            match prev_url {
                Some(v) => std::env::set_var(DB_REMOTE_ALIAS_URL_ENV, v),
                None => std::env::remove_var(DB_REMOTE_ALIAS_URL_ENV),
            }
            match prev_cloudless_path {
                Some(v) => std::env::set_var("VOX_SECRETS_CLOUDLESS_DB_PATH", v),
                None => std::env::remove_var("VOX_SECRETS_CLOUDLESS_DB_PATH"),
            }
            match prev_account_id {
                Some(v) => std::env::set_var("VOX_ACCOUNT_ID", v),
                None => std::env::remove_var("VOX_ACCOUNT_ID"),
            }
        }
    }

    #[test]
    #[allow(unsafe_code)]
    fn operator_status_surface_never_serializes_secret_values() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let openreview_token_key = "VOX_OPENREVIEW_ACCESS_TOKEN";
        let prev_token = std::env::var(openreview_token_key).ok();
        unsafe {
            std::env::set_var("VOX_OPENREVIEW_ACCESS_TOKEN", "do-not-leak-me");
            std::env::set_var("VOX_SECRETS_BACKEND", "env_only");
            std::env::remove_var("VOX_SECRETS_PROFILE");
        }
        let manifest = sample_manifest(|_| {});
        let report = run_preflight(&manifest, PreflightProfile::Default);
        let status = operator_status_surface_v1(
            &manifest.publication_id,
            PreflightProfile::Default,
            &report,
        );
        let json = serde_json::to_string(&status).expect("serialize operator status");
        assert!(!json.contains("do-not-leak-me"));
        assert!(!json.contains("VOX_OPENREVIEW_ACCESS_TOKEN"));
        unsafe {
            match prev_token {
                Some(v) => std::env::set_var("VOX_OPENREVIEW_ACCESS_TOKEN", v),
                None => std::env::remove_var("VOX_OPENREVIEW_ACCESS_TOKEN"),
            }
            std::env::remove_var("VOX_SECRETS_BACKEND");
            std::env::remove_var("VOX_SECRETS_PROFILE");
        }
    }
}
