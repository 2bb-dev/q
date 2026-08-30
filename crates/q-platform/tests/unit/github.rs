use super::*;
use crate::TEST_ENV_GUARD as ENV_GUARD;
use tempfile::TempDir;

fn with_app_dir<T>(action: impl FnOnce() -> T) -> T {
    let _guard = ENV_GUARD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let tmp = TempDir::new().unwrap();
    std::env::set_var("QCLI_APP_DIR", tmp.path());
    let result = action();
    std::env::remove_var("QCLI_APP_DIR");
    result
}

#[test]
fn stored_token_roundtrips_and_deletes() {
    with_app_dir(|| {
        assert_eq!(stored_token().unwrap(), None);
        store_token("gho_secret").unwrap();
        assert_eq!(stored_token().unwrap(), Some("gho_secret".to_string()));
        assert!(delete_token().unwrap());
        assert_eq!(stored_token().unwrap(), None);
        assert!(!delete_token().unwrap());
    });
}

#[cfg(unix)]
#[test]
fn stored_token_file_is_owner_only() {
    with_app_dir(|| {
        use std::os::unix::fs::PermissionsExt;
        store_token("gho_secret").unwrap();
        let path = app_dir().unwrap().join("github_token");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    });
}

#[test]
fn blank_stored_token_counts_as_missing() {
    with_app_dir(|| {
        store_token("  ").unwrap();
        assert_eq!(stored_token().unwrap(), None);
    });
}

#[test]
fn device_authorization_defaults_the_poll_interval() {
    let parsed: DeviceAuthorization = serde_json::from_str(
        r#"{"device_code":"d","user_code":"ABCD-1234","verification_uri":"https://github.com/login/device"}"#,
    )
    .unwrap();
    assert_eq!(parsed.poll_interval(), std::time::Duration::from_secs(5));
}
