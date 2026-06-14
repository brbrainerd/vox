use crate::PublisherConfig;
#[cfg(feature = "live-api-canary")]
use crate::adapters::canary;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AdapterHealthReport {
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub adapters: Vec<AdapterHealthEntry>,
}

#[derive(Debug, Serialize)]
pub struct AdapterHealthEntry {
    pub name: &'static str,
    pub feature_enabled: bool,
    pub credentials_present: bool,
    pub heartbeat_status: Option<HeartbeatStatus>,
    pub diagnostic_message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum HeartbeatStatus {
    Skipped { reason: String },
    Ok { latency_ms: u64 },
    AuthFailure { hint: String },
    ContractMismatch { detail: String },
    ServiceDown { http_status: Option<u16> },
    NetworkError { message: String },
}

pub async fn report_health(
    cfg: &PublisherConfig,
    live: bool,
) -> anyhow::Result<AdapterHealthReport> {
    let mut adapters = Vec::new();

    // Mastodon
    adapters.push(AdapterHealthEntry {
        name: "mastodon",
        feature_enabled: cfg!(feature = "scientia-mastodon"),
        credentials_present: cfg.mastodon_access_token.is_some(),
        heartbeat_status: if live && cfg!(feature = "scientia-mastodon") {
            #[cfg(feature = "live-api-canary")]
            {
                Some(canary::probe_mastodon(cfg).await)
            }
            #[cfg(not(feature = "live-api-canary"))]
            {
                None
            }
        } else {
            None
        },
        diagnostic_message: None,
    });

    // Bluesky
    adapters.push(AdapterHealthEntry {
        name: "bluesky",
        feature_enabled: cfg!(feature = "scientia-bluesky"),
        credentials_present: cfg.bluesky_handle.is_some() && cfg.bluesky_password.is_some(),
        heartbeat_status: if live && cfg!(feature = "scientia-bluesky") {
            #[cfg(feature = "live-api-canary")]
            {
                Some(canary::probe_bluesky(cfg).await)
            }
            #[cfg(not(feature = "live-api-canary"))]
            {
                None
            }
        } else {
            None
        },
        diagnostic_message: None,
    });

    // Discord
    adapters.push(AdapterHealthEntry {
        name: "discord",
        feature_enabled: cfg!(feature = "scientia-discord"),
        credentials_present: vox_secrets::resolve_secret(
            vox_secrets::SecretId::VoxSocialDiscordWebhook,
        )
        .expose()
        .is_some(),
        heartbeat_status: if live && cfg!(feature = "scientia-discord") {
            #[cfg(feature = "live-api-canary")]
            {
                Some(canary::probe_discord(cfg).await)
            }
            #[cfg(not(feature = "live-api-canary"))]
            {
                None
            }
        } else {
            None
        },
        diagnostic_message: None,
    });

    // Twitter
    adapters.push(AdapterHealthEntry {
        name: "twitter",
        feature_enabled: cfg!(feature = "scientia-twitter"),
        credentials_present: cfg.twitter_bearer_token.is_some()
            || vox_secrets::resolve_secret(vox_secrets::SecretId::VoxNewsTwitterBearer)
                .expose()
                .is_some(),
        heartbeat_status: if live && cfg!(feature = "scientia-twitter") {
            #[cfg(feature = "live-api-canary")]
            {
                Some(canary::probe_twitter(cfg).await)
            }
            #[cfg(not(feature = "live-api-canary"))]
            {
                Some(HeartbeatStatus::Skipped {
                    reason: "live-api-canary feature inactive".to_string(),
                })
            }
        } else {
            None
        },
        diagnostic_message: None,
    });

    // LinkedIn
    adapters.push(AdapterHealthEntry {
        name: "linkedin",
        feature_enabled: cfg!(feature = "scientia-linkedin"),
        credentials_present: cfg.linkedin_access_token.is_some()
            || vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSocialLinkedinAccessToken)
                .expose()
                .is_some(),
        heartbeat_status: if live && cfg!(feature = "scientia-linkedin") {
            #[cfg(feature = "live-api-canary")]
            {
                Some(canary::probe_linkedin(cfg).await)
            }
            #[cfg(not(feature = "live-api-canary"))]
            {
                Some(HeartbeatStatus::Skipped {
                    reason: "live-api-canary feature inactive".to_string(),
                })
            }
        } else {
            None
        },
        diagnostic_message: None,
    });

    // OpenCollective
    adapters.push(AdapterHealthEntry {
        name: "opencollective",
        feature_enabled: cfg!(feature = "scientia-opencollective"),
        credentials_present: cfg.open_collective_token.is_some()
            || vox_secrets::resolve_secret(vox_secrets::SecretId::VoxNewsOpenCollectiveToken)
                .expose()
                .is_some(),
        heartbeat_status: if live && cfg!(feature = "scientia-opencollective") {
            #[cfg(feature = "live-api-canary")]
            {
                Some(canary::probe_opencollective(cfg).await)
            }
            #[cfg(not(feature = "live-api-canary"))]
            {
                Some(HeartbeatStatus::Skipped {
                    reason: "live-api-canary feature inactive".to_string(),
                })
            }
        } else {
            None
        },
        diagnostic_message: None,
    });

    // Reddit
    adapters.push(AdapterHealthEntry {
        name: "reddit",
        feature_enabled: cfg!(feature = "scientia-reddit"),
        credentials_present: cfg.reddit_refresh_token.is_some()
            || vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSocialRedditRefreshToken)
                .expose()
                .is_some(),
        heartbeat_status: Some(HeartbeatStatus::Skipped {
            reason: "unimplemented".to_string(),
        }),
        diagnostic_message: None,
    });

    // Zenodo
    adapters.push(AdapterHealthEntry {
        name: "zenodo",
        feature_enabled: true, // usually enabled if scholarly
        credentials_present: vox_secrets::resolve_secret(
            vox_secrets::SecretId::VoxZenodoAccessToken,
        )
        .expose()
        .is_some(),
        heartbeat_status: if live {
            #[cfg(feature = "live-api-canary")]
            {
                Some(canary::probe_zenodo(cfg).await)
            }
            #[cfg(not(feature = "live-api-canary"))]
            {
                Some(HeartbeatStatus::Skipped {
                    reason: "live-api-canary feature inactive".to_string(),
                })
            }
        } else {
            None
        },
        diagnostic_message: None,
    });

    // OpenReview
    adapters.push(AdapterHealthEntry {
        name: "openreview",
        feature_enabled: true,
        credentials_present: vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOpenReviewEmail)
            .expose()
            .is_some()
            && vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOpenReviewPassword)
                .expose()
                .is_some(),
        heartbeat_status: None, // No probe yet
        diagnostic_message: None,
    });

    Ok(AdapterHealthReport {
        generated_at: chrono::Utc::now(),
        adapters,
    })
}

#[cfg(test)]
mod semcov_wave1d_tests {
    #![allow(unused_imports)]
    use super::*;
    use crate::PublisherConfig;

    #[tokio::test]
    async fn report_health_offline_default_config_is_deterministic() {
        let cfg = PublisherConfig::default();
        let report = report_health(&cfg, false)
            .await
            .expect("report_health should succeed");

        // Exactly nine adapters in a fixed, documented order.
        let names: Vec<&str> = report.adapters.iter().map(|a| a.name).collect();
        assert_eq!(
            names,
            vec![
                "mastodon",
                "bluesky",
                "discord",
                "twitter",
                "linkedin",
                "opencollective",
                "reddit",
                "zenodo",
                "openreview",
            ]
        );

        let by_name = |n: &str| report.adapters.iter().find(|a| a.name == n).unwrap();

        // Reddit always reports the "unimplemented" Skipped heartbeat, regardless of live flag.
        match &by_name("reddit").heartbeat_status {
            Some(HeartbeatStatus::Skipped { reason }) => assert_eq!(reason, "unimplemented"),
            other => panic!("expected reddit Skipped(unimplemented), got {other:?}"),
        }

        // With live=false, mastodon/bluesky/twitter/linkedin perform no probe.
        for n in [
            "mastodon",
            "bluesky",
            "twitter",
            "linkedin",
            "opencollective",
            "zenodo",
        ] {
            assert!(
                by_name(n).heartbeat_status.is_none(),
                "{n} heartbeat should be None when live=false"
            );
        }

        // Default config carries no mastodon/bluesky credentials.
        assert!(!by_name("mastodon").credentials_present);
        assert!(!by_name("bluesky").credentials_present);

        // openreview never has a probe wired yet.
        assert!(by_name("openreview").heartbeat_status.is_none());

        // diagnostic_message is never populated.
        assert!(
            report
                .adapters
                .iter()
                .all(|a| a.diagnostic_message.is_none())
        );
    }
}
