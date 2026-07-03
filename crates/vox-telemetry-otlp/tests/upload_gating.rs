#![allow(unsafe_code)] // test-only std::env::set_var (unsafe on edition 2024)
/// Guards the invariant: remote upload must be gated by is_remote_allowed().
/// When the master switch is off or consent is Denied/Unset, upload returns 0.
use tempfile::TempDir;
use vox_telemetry::config::{ConsentState, set_remote_consent};

fn with_temp_config<F: FnOnce()>(f: F) -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    #[cfg(windows)]
    unsafe {
        std::env::set_var("APPDATA", dir.path());
    }
    #[cfg(not(windows))]
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", dir.path());
    }
    f();
    dir
}

#[test]
fn upload_gated_when_consent_unset() {
    let _dir = with_temp_config(|| {
        unsafe {
            std::env::remove_var("VOX_TELEMETRY");
        }
        // No consent file written → Unset.
        assert!(
            !vox_telemetry::config::is_remote_allowed(),
            "upload must be gated when consent is Unset"
        );
    });
}

#[test]
fn upload_allowed_when_consent_granted_and_master_on() {
    let _dir = with_temp_config(|| {
        unsafe {
            std::env::remove_var("VOX_TELEMETRY");
        }
        set_remote_consent(ConsentState::Granted);
        assert!(
            vox_telemetry::config::is_remote_allowed(),
            "upload must be allowed when consent is Granted and master is on"
        );
    });
}

#[test]
fn upload_gated_when_master_off_even_if_granted() {
    let _dir = with_temp_config(|| {
        unsafe {
            std::env::set_var("VOX_TELEMETRY", "off");
        }
        set_remote_consent(ConsentState::Granted);
        assert!(
            !vox_telemetry::config::is_remote_allowed(),
            "master off must override consent=Granted"
        );
        unsafe {
            std::env::remove_var("VOX_TELEMETRY");
        }
    });
}

#[test]
fn upload_gated_when_consent_denied() {
    let _dir = with_temp_config(|| {
        unsafe {
            std::env::remove_var("VOX_TELEMETRY");
        }
        set_remote_consent(ConsentState::Denied);
        assert!(
            !vox_telemetry::config::is_remote_allowed(),
            "upload must be gated when consent is Denied"
        );
    });
}
