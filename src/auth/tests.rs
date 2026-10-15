use super::*;
use std::{cell::RefCell, collections::BTreeMap, path::PathBuf};

#[derive(Default)]
struct FakeStore {
    credentials: RefCell<BTreeMap<String, String>>,
    operations: RefCell<Vec<String>>,
    fail_get: bool,
    fail_set_at: Option<usize>,
    fail_delete: bool,
    sets: RefCell<usize>,
}

impl CredentialStore for FakeStore {
    fn get(&self, origin: &Url) -> Result<Option<String>> {
        self.operations.borrow_mut().push(format!("get:{origin}"));
        anyhow::ensure!(!self.fail_get, "injected get failure");
        Ok(self.credentials.borrow().get(origin.as_str()).cloned())
    }

    fn set(&self, origin: &Url, token: &str) -> Result<()> {
        self.operations.borrow_mut().push(format!("set:{origin}"));
        let mut sets = self.sets.borrow_mut();
        *sets += 1;
        anyhow::ensure!(self.fail_set_at != Some(*sets), "injected set failure");
        self.credentials
            .borrow_mut()
            .insert(origin.to_string(), token.to_owned());
        Ok(())
    }

    fn delete(&self, origin: &Url) -> Result<()> {
        self.operations
            .borrow_mut()
            .push(format!("delete:{origin}"));
        anyhow::ensure!(!self.fail_delete, "injected delete failure");
        self.credentials.borrow_mut().remove(origin.as_str());
        Ok(())
    }
}

fn config(path: PathBuf) -> Config {
    Config::load(Some(path), false, true).unwrap()
}

fn origin(host: &str) -> Url {
    Url::parse(&format!("https://{host}/")).unwrap()
}

#[test]
fn persist_registers_token_and_logout_removes_both() {
    let temp = tempfile::tempdir().unwrap();
    let origin = origin("forge.example");
    let mut config = config(temp.path().join("config.toml"));
    let store = FakeStore::default();
    persist(&mut config, &store, &origin, Some("secret")).unwrap();
    assert!(config.has_login(&origin));
    assert_eq!(
        saved_token(&origin, &config, &store).unwrap().as_deref(),
        Some("secret")
    );
    persist(&mut config, &store, &origin, None).unwrap();
    assert!(!config.has_login(&origin));
    assert_eq!(store.credentials.borrow().get(origin.as_str()), None);
}

#[test]
fn saved_token_skips_unregistered_hosts_and_errors_for_missing_credentials() {
    let temp = tempfile::tempdir().unwrap();
    let registered = origin("forge.example");
    let other = origin("other.example");
    let mut config = config(temp.path().join("config.toml"));
    let store = FakeStore {
        fail_get: true,
        ..FakeStore::default()
    };
    assert_eq!(saved_token(&other, &config, &store).unwrap(), None);
    assert!(store.operations.borrow().is_empty());
    config.set_login(&registered, true).unwrap();
    let missing = FakeStore::default();
    assert!(
        saved_token(&registered, &config, &missing)
            .unwrap_err()
            .to_string()
            .contains("saved forge credential is missing")
    );
    let failing = FakeStore {
        fail_get: true,
        ..FakeStore::default()
    };
    assert!(
        saved_token(&registered, &config, &failing)
            .unwrap_err()
            .to_string()
            .contains("injected get failure")
    );
}

#[test]
fn oauth_bundle_round_trips_refresh_token_and_rejects_expired_access() {
    let temp = tempfile::tempdir().unwrap();
    let origin = origin("forge.example");
    let mut config = config(temp.path().join("config.toml"));
    let store = FakeStore::default();
    let bundle = OAuthBundle {
        access_token: "access-secret".to_owned(),
        refresh_token: Some("refresh-secret".to_owned()),
        expires_at: Some(1),
    };
    let encoded = encode_oauth(&bundle).unwrap();
    store
        .credentials
        .borrow_mut()
        .insert(origin.to_string(), encoded);
    config.set_login(&origin, true).unwrap();

    assert_eq!(
        saved_refresh_token(&origin, &config, &store)
            .unwrap()
            .as_deref(),
        Some("refresh-secret")
    );
    let error = saved_token(&origin, &config, &store)
        .expect_err("expired access token is rejected")
        .to_string();
    assert!(error.contains("run collar auth login"));
    assert!(!error.contains("access-secret"));
    assert!(!error.contains("refresh-secret"));
}

#[test]
fn credential_set_failure_does_not_register_login() {
    let temp = tempfile::tempdir().unwrap();
    let origin = origin("forge.example");
    let mut config = config(temp.path().join("config.toml"));
    let store = FakeStore {
        fail_set_at: Some(1),
        ..FakeStore::default()
    };
    assert!(persist(&mut config, &store, &origin, Some("secret")).is_err());
    assert!(!config.has_login(&origin));
}

#[test]
fn metadata_failure_restores_previous_token_and_login_marker() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.toml");
    let origin = origin("forge.example");
    let mut config = config(path.clone());
    let store = FakeStore::default();
    persist(&mut config, &store, &origin, Some("old")).unwrap();
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    let error = persist(&mut config, &store, &origin, Some("new")).unwrap_err();
    assert!(format!("{error:#}").contains("previous credential restored"));
    assert!(config.has_login(&origin));
    assert_eq!(
        store
            .credentials
            .borrow()
            .get(origin.as_str())
            .map(String::as_str),
        Some("old")
    );
}

#[test]
fn failed_metadata_for_new_credential_deletes_orphan() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.toml");
    let origin = origin("forge.example");
    let mut config = config(path.clone());
    std::fs::create_dir(&path).unwrap();
    let store = FakeStore::default();
    let error = persist(&mut config, &store, &origin, Some("new")).unwrap_err();
    assert!(format!("{error:#}").contains("previous credential restored"));
    assert!(!config.has_login(&origin));
    assert_eq!(store.credentials.borrow().get(origin.as_str()), None);
}

#[test]
fn rollback_failure_preserves_metadata_error_and_recovery_guidance() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config.toml");
    let origin = origin("forge.example");
    let mut config = config(path.clone());
    std::fs::create_dir(&path).unwrap();
    let store = FakeStore {
        fail_delete: true,
        ..FakeStore::default()
    };
    let error = persist(&mut config, &store, &origin, Some("new")).unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("could not save login metadata"));
    assert!(message.contains("credential rollback also failed"));
    assert!(message.contains("run auth logout for this host"));
}
