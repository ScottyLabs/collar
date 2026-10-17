#[allow(dead_code)]
#[path = "../../src/oauth.rs"]
mod helper;

#[cfg(unix)]
mod unix {
    use tempfile::TempDir;
    use url::Url;

    fn helper(temp: &TempDir, script: &str) -> std::path::PathBuf {
        let path = temp.path().join("mock-oauth-helper");
        std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        path
    }

    fn origin() -> Url {
        Url::parse("https://git.cmu.dev/").unwrap()
    }

    #[test]
    fn passes_forgejo_realm_for_unknown_host_and_returns_only_password() {
        let temp = TempDir::new().unwrap();
        let input_path = temp.path().join("input");
        let script = format!(
            "cat > '{}'\nprintf 'capability[]=authtype\\nusername=oauth2\\npassword=fixture-token\\npassword_expiry_utc=1234\\noauth_refresh_token=fixture-refresh\\n\\n'",
            input_path.display()
        );
        let helper = helper(&temp, &script);
        let token =
            super::helper::browser_login_with_helper(&origin(), None, helper.as_os_str()).unwrap();

        assert_eq!(token.access_token, "fixture-token");
        assert_eq!(token.refresh_token.as_deref(), Some("fixture-refresh"));
        assert_eq!(
            std::fs::read_to_string(input_path).unwrap(),
            "protocol=https\nhost=git.cmu.dev\nwwwauth[]=realm=\"Forgejo\"\n\n"
        );
    }

    #[test]
    fn sends_saved_refresh_token_to_helper_without_putting_it_in_urls() {
        let temp = TempDir::new().unwrap();
        let input_path = temp.path().join("input");
        let script = format!(
            "cat > '{}'\nprintf 'password=refreshed-access\\noauth_refresh_token=rotated-refresh\\n\\n'",
            input_path.display()
        );
        let helper = helper(&temp, &script);
        let credential = super::helper::browser_login_with_helper(
            &origin(),
            Some("saved-refresh"),
            helper.as_os_str(),
        )
        .unwrap();

        assert_eq!(credential.access_token, "refreshed-access");
        assert_eq!(credential.refresh_token.as_deref(), Some("rotated-refresh"));
        assert_eq!(
            std::fs::read_to_string(input_path).unwrap(),
            "protocol=https\nhost=git.cmu.dev\nwwwauth[]=realm=\"Forgejo\"\noauth_refresh_token=saved-refresh\n\n"
        );
    }

    #[test]
    fn rejects_helper_failure_without_exposing_its_stderr() {
        let temp = TempDir::new().unwrap();
        let helper = helper(&temp, "echo fixture-secret >&2\nexit 23");
        let error = super::helper::browser_login_with_helper(&origin(), None, helper.as_os_str())
            .err()
            .expect("helper failure is rejected")
            .to_string();

        assert!(error.contains("browser login failed"));
        assert!(!error.contains("fixture-secret"));
    }

    #[test]
    fn rejects_untrusted_http_origins_before_starting_the_helper() {
        let temp = TempDir::new().unwrap();
        let marker = temp.path().join("invoked");
        let helper = helper(&temp, &format!("touch '{}'", marker.display()));
        let origin = Url::parse("http://git.cmu.dev/").unwrap();

        let error = super::helper::browser_login_with_helper(&origin, None, helper.as_os_str())
            .err()
            .expect("insecure origin is rejected")
            .to_string();
        assert!(error.contains("require HTTPS"));
        assert!(!marker.exists());
    }
}

#[test]
fn refresh_rotates_credentials_without_browser_or_api_write_replay() {
    use super::*;
    for rotated in [None, Some("rotated-refresh")] {
        let server = Server::new(vec![Response::json(json!({
            "access_token": "new-access",
            "refresh_token": rotated,
            "expires_in": 3600,
        }))]);
        let credential =
            helper::refresh(&url::Url::parse(&server.origin).unwrap(), "old-refresh").unwrap();
        assert_eq!(credential.access_token, "new-access");
        assert_eq!(
            credential.refresh_token.as_deref(),
            Some(rotated.unwrap_or("old-refresh"))
        );
        assert!(credential.expires_at.is_some());
        let requests = server.finish();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("POST /login/oauth/access_token "));
        assert!(requests[0].contains("grant_type=refresh_token"));
        assert!(requests[0].contains("refresh_token=old-refresh"));
    }
}

#[test]
fn refresh_failures_are_redacted_and_not_retried() {
    use super::*;
    for status in [400, 500] {
        let server = Server::new(vec![Response {
            status,
            body: "secret-error-body".into(),
            headers: String::new(),
        }]);
        let error = helper::refresh(&url::Url::parse(&server.origin).unwrap(), "refresh-secret")
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("auth login --web"));
        assert!(!error.contains("secret-error-body"));
        assert!(!error.contains("refresh-secret"));
        assert_eq!(server.finish().len(), 1);
    }
}
