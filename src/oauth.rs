//! Runs the credential helper with bounded output and parses its OAuth credential response.

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use url::Url;

const HELPER_TIMEOUT: Duration = Duration::from_secs(300);
const HELPER_OUTPUT_LIMIT: usize = 64 * 1024;

/// Access and refresh credentials returned by the installed OAuth helper.
///
/// Forgejo pre-registers this helper's OAuth client. The helper owns the PKCE,
/// loopback callback, state validation, and OAuth token exchange.
pub struct OAuthCredential {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
}

#[cfg(test)]
pub fn browser_login_with_helper(
    origin: &Url,
    refresh_token: Option<&str>,
    helper: &OsStr,
) -> Result<OAuthCredential> {
    browser_login_inner(origin, refresh_token, helper)
}

pub fn browser_login(origin: &Url, refresh_token: Option<&str>) -> Result<OAuthCredential> {
    browser_login_inner(origin, refresh_token, OsStr::new("git-credential-oauth"))
}

fn browser_login_inner(
    origin: &Url,
    refresh_token: Option<&str>,
    helper: &OsStr,
) -> Result<OAuthCredential> {
    ensure!(
        origin.username().is_empty()
            && origin.password().is_none()
            && origin.path() == "/"
            && origin.query().is_none()
            && origin.fragment().is_none()
            && origin.host_str().is_some(),
        "forge host must be an origin without credentials, a path, or a query"
    );
    ensure!(
        origin.scheme() == "https" || is_loopback(origin),
        "forge connections require HTTPS (HTTP is allowed only on loopback)"
    );

    let host = origin.host_str().context("forge host has no hostname")?;
    let host = match origin.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    };
    if let Some(token) = refresh_token {
        ensure!(
            !token.trim().is_empty() && !token.contains(['\r', '\n']),
            "saved OAuth refresh token is invalid; run collar auth login again"
        );
    }
    let mut request = format!(
        "protocol={}\nhost={host}\nwwwauth[]=realm=\"Forgejo\"\n\n",
        origin.scheme()
    );
    if let Some(token) = refresh_token {
        request = format!(
            "protocol={}\nhost={host}\nwwwauth[]=realm=\"Forgejo\"\noauth_refresh_token={token}\n\n",
            origin.scheme()
        );
    }
    let git_config = tempfile::tempdir().context("isolate OAuth helper Git configuration")?;
    let mut child = Command::new(helper)
        .arg("get")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .current_dir(git_config.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", git_config.path().join("gitconfig"))
        .env("GIT_CONFIG_COUNT", "0")
        .env_remove("GIT_CONFIG_PARAMETERS")
        .env_remove("GIT_CONFIG")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_COMMON_DIR")
        .spawn()
        .context("start git-credential-oauth; install the Forgejo OAuth credential helper")?;
    let stdout = child
        .stdout
        .take()
        .context("capture credential helper output")?;
    let output_reader = match thread::Builder::new()
        .name("collar-oauth-output".into())
        .spawn(move || read_limited(stdout))
    {
        Ok(reader) => reader,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error).context("read credential helper output");
        }
    };
    let input_result = child
        .stdin
        .take()
        .context("open credential helper input")
        .and_then(|mut input| {
            input
                .write_all(request.as_bytes())
                .context("send forge host to credential helper")
        });
    if let Err(error) = input_result {
        let _ = child.kill();
        let _ = child.wait();
        let _ = output_reader.join();
        return Err(error);
    }
    let deadline = Instant::now() + HELPER_TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait().context("wait for Forgejo browser login")? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = output_reader.join();
            anyhow::bail!("Forgejo browser login timed out after five minutes");
        }
        thread::sleep(Duration::from_millis(50));
    };
    let (output, exceeded_limit) = output_reader
        .join()
        .map_err(|_| anyhow::anyhow!("credential helper output reader failed"))??;
    ensure!(!exceeded_limit, "credential helper output exceeds 64 KiB");
    ensure!(
        status.success(),
        "Forgejo browser login failed; check the browser flow and credential helper setup"
    );
    parse_credential(&output)
}

fn read_limited(mut reader: impl Read) -> Result<(Vec<u8>, bool)> {
    let mut output = Vec::new();
    let mut exceeded_limit = false;
    let mut buffer = [0; 4096];
    loop {
        let count = reader.read(&mut buffer).context("read helper output")?;
        if count == 0 {
            break;
        }
        let remaining = HELPER_OUTPUT_LIMIT.saturating_sub(output.len());
        output.extend_from_slice(&buffer[..count.min(remaining)]);
        exceeded_limit |= count > remaining;
    }
    Ok((output, exceeded_limit))
}

fn is_loopback(origin: &Url) -> bool {
    origin.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    })
}

pub fn parse_credential(output: &[u8]) -> Result<OAuthCredential> {
    let output =
        std::str::from_utf8(output).context("credential helper returned invalid output")?;
    let mut password = None;
    let mut refresh_token = None;
    let mut expires_at = None;
    let mut keys = std::collections::HashSet::new();
    for line in output.lines() {
        if line.is_empty() {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .context("credential helper returned malformed output")?;
        ensure!(
            !key.is_empty()
                && key.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'[' | b']')
                }),
            "credential helper returned malformed output"
        );
        if !key.ends_with("[]") {
            ensure!(
                keys.insert(key),
                "credential helper returned duplicate fields"
            );
        }
        if key == "password" {
            password = Some(value);
        } else if key == "oauth_refresh_token" {
            refresh_token = Some(value);
        } else if key == "password_expiry_utc" {
            expires_at = Some(
                value
                    .parse::<u64>()
                    .context("credential helper returned an invalid token expiry")?,
            );
        }
    }
    let password = password.context("credential helper returned no access token")?;
    ensure!(
        !password.trim().is_empty() && !password.contains(['\r', '\n']),
        "credential helper returned an invalid access token"
    );
    if let Some(token) = refresh_token {
        ensure!(
            !token.trim().is_empty() && !token.contains(['\r', '\n']),
            "credential helper returned an invalid refresh token"
        );
    }
    Ok(OAuthCredential {
        access_token: password.to_owned(),
        refresh_token: refresh_token.map(str::to_owned),
        expires_at,
    })
}

#[cfg(test)]
mod tests {
    use super::{browser_login_with_helper, parse_credential};
    use std::ffi::OsStr;
    use url::Url;

    #[test]
    fn rejects_insecure_origin_before_starting_helper() {
        let origin = Url::parse("http://forge.example/").unwrap();
        let error = browser_login_with_helper(&origin, None, OsStr::new("missing-helper"))
            .err()
            .expect("non-loopback HTTP origin is rejected");
        assert!(error.to_string().contains("require HTTPS"));
    }

    #[test]
    fn accepts_a_single_password_field_as_the_access_token() {
        assert_eq!(
            parse_credential(b"username=forge-user\npassword=oauth-token\n\n")
                .unwrap()
                .access_token,
            "oauth-token"
        );
    }

    #[test]
    fn retains_refresh_token_without_exposing_it_in_errors() {
        let credential =
            parse_credential(b"password=access\noauth_refresh_token=refresh\n\n").unwrap();
        assert_eq!(credential.access_token, "access");
        assert_eq!(credential.refresh_token.as_deref(), Some("refresh"));
    }

    #[test]
    fn retains_access_token_expiry() {
        let credential =
            parse_credential(b"password=access\npassword_expiry_utc=1234\n\n").unwrap();
        assert_eq!(credential.expires_at, Some(1234));
    }

    #[test]
    fn accepts_git_credential_helper_capability_output() {
        let credential = parse_credential(
            b"capability[]=authtype\nusername=oauth2\npassword=fixture-access\npassword_expiry_utc=1234\noauth_refresh_token=fixture-refresh\n\n",
        )
        .unwrap();
        assert_eq!(credential.access_token, "fixture-access");
        assert_eq!(credential.refresh_token.as_deref(), Some("fixture-refresh"));
        assert_eq!(credential.expires_at, Some(1234));
    }

    #[test]
    fn allows_repeated_multivalue_keys_but_not_duplicate_secrets() {
        assert!(
            parse_credential(b"capability[]=authtype\ncapability[]=state\npassword=access\n")
                .is_ok()
        );
        for output in [
            b"password=first\npassword=second\n".as_slice(),
            b"password=access\noauth_refresh_token=one\noauth_refresh_token=two\n",
            b"password=access\npassword_expiry_utc=1\npassword_expiry_utc=2\n",
        ] {
            assert!(parse_credential(output).is_err());
        }
    }

    #[test]
    fn rejects_missing_duplicate_or_empty_access_tokens() {
        for output in [
            b"username=forge-user\n\n".as_slice(),
            b"password=first\npassword=second\n\n".as_slice(),
            b"password=\n\n".as_slice(),
            b"password= \n\n".as_slice(),
            b"username=one\nusername=two\npassword=valid\n".as_slice(),
            b"password\n".as_slice(),
        ] {
            assert!(parse_credential(output).is_err());
        }
    }

    #[test]
    fn rejects_non_utf8_helper_output() {
        assert!(parse_credential(b"password=\xff\n").is_err());
    }
}
