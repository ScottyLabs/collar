use super::*;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;

fn fixture() -> (TempDir, PathBuf, PathBuf) {
    let temp = TempDir::new().unwrap();
    let (project, bin) = doctor_fixture(temp.path());
    std::fs::write(project.join("secretspec.toml"), "[project]\nname = 'collie'\nrevision = '1.0'\n[providers]\nlocal = 'dotenv://.env'\n[profiles.default]\nSHARED = {description = 'Shared secret'}\n[profiles.dev]\nDEV_ONLY = {description = 'Development secret'}\n[profiles.dev.defaults]\nproviders = ['local']\n[scopes.build]\nsecrets = ['SHARED']\n").unwrap();
    for tool in ["nix", "bao", "secretspec", "devenv"] {
        let file = bin.join(tool);
        std::fs::write(&file, "#!/bin/sh\nprintf 'cwd:%s\\nreason:%s\\nbao:%s\\n' \"$PWD\" \"${SECRETSPEC_REASON-}\" \"${BAO_ADDR-}\" >> \"$COLLAR_TEST_LOG\"\nfor value in \"$@\"; do printf 'arg:%s\\n' \"$value\" >> \"$COLLAR_TEST_LOG\"; done\nif [ \"${1-}\" = set ]; then IFS= read -r secret; printf 'stdin:received\\n' >> \"$COLLAR_TEST_LOG\"; fi\nprintf '%s\\n' 'tool-output'\nexit \"${COLLAR_TEST_EXIT:-0}\"\n").unwrap();
        std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    (temp, project, bin)
}

fn cli(temp: &Path, project: &Path, bin: &Path) -> Command {
    let mut cmd = command();
    cmd.args(["-R", "ScottyLabs/collie-api"])
        .env("PATH", bin)
        .env("COLLAR_GOVERNANCE", temp.join("governance"))
        .env("COLLAR_TEST_LOG", temp.join("commands.log"))
        .env_remove("SECRETSPEC_PROFILE")
        .env_remove("SECRETSPEC_REASON")
        .env_remove("SECRETSPEC_PROVIDER")
        .env_remove("SECRETSPEC_SCOPE")
        .env_remove("SECRETSPEC_FILE")
        .current_dir(project);
    cmd
}

fn log(temp: &Path) -> String {
    std::fs::read_to_string(temp.join("commands.log")).unwrap()
}

#[test]
fn secret_checks_are_value_free_noninteractive_and_profile_bound() {
    let (temp, project, bin) = fixture();
    let output = cli(temp.path(), &project, &bin)
        .env("SECRETSPEC_REASON", "inspect development setup")
        .args([
            "secrets",
            "check",
            "--profile",
            "dev",
            "--provider",
            "local",
            "--scope",
            "build",
            "--json",
        ])
        .output()
        .unwrap();
    success(&output);
    let args = log(temp.path());
    for expected in [
        "arg:check\n",
        "arg:--file\n",
        "arg:--profile\narg:dev\n",
        "arg:--provider\narg:local\n",
        "arg:--no-prompt\n",
        "arg:--json\n",
        "arg:--scope\narg:build\n",
        "arg:--reason=inspect development setup\n",
        "arg:--caller\narg:collar\n",
        "arg:--caller-resource\narg:collie\n",
    ] {
        assert!(args.contains(expected), "missing {expected}: {args}");
    }
    assert!(!args.contains("arg:--explain\n"));
    assert!(args.contains(&format!("cwd:{}\n", project.display())));
    assert!(args.contains(&format!(
        "arg:{}\n",
        project.join("secretspec.toml").display()
    )));
    let output = cli(temp.path(), &project, &bin)
        .args(["secrets", "check"])
        .output()
        .unwrap();
    success(&output);
    assert!(log(temp.path()).contains("arg:--explain\n"));
}

#[test]
fn secret_updates_accept_explicit_stdin_without_value_arguments_or_logging() {
    let (temp, project, bin) = fixture();
    let mut child = cli(temp.path(), &project, &bin)
        .args([
            "secrets",
            "set",
            "SHARED",
            "--from-file",
            "-",
            "--reason",
            "update local fixture",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"never-echo-secret-input\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    success(&output);
    let args = log(temp.path());
    assert!(args.contains("arg:--from-file\narg:-\narg:--\narg:SHARED\n"));
    assert!(args.contains("stdin:received"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    for text in [args.as_str(), stdout.as_ref(), stderr.as_ref()] {
        assert!(!text.contains("never-echo-secret-input"));
    }
}

#[test]
fn invalid_secret_context_and_headless_prompts_fail_before_provider_execution() {
    let (temp, project, bin) = fixture();
    for (args, expected) in [
        (
            vec!["secrets", "check", "--profile", "prod"],
            "profile prod",
        ),
        (
            vec!["secrets", "check", "--scope", "unknown"],
            "scope is not declared",
        ),
        (
            vec!["secrets", "check", "--provider", "unknown"],
            "provider must be an alias",
        ),
        (
            vec!["secrets", "set", "UNDECLARED", "--from-file", "-"],
            "secret is not declared",
        ),
        (
            vec!["secrets", "set", "SHARED"],
            "terminal prompt or explicit --from-file",
        ),
        (
            vec!["dev", "shell"],
            "interactive shell requires a terminal",
        ),
    ] {
        let output = cli(temp.path(), &project, &bin)
            .args(args)
            .output()
            .unwrap();
        failure(&output, expected);
        assert!(!temp.path().join("commands.log").exists());
    }
    let spec = project.join("secretspec.toml");
    std::fs::write(
        &spec,
        std::fs::read_to_string(&spec)
            .unwrap()
            .replace("name = 'collie'", "name = 'wrong-project'"),
    )
    .unwrap();
    let output = cli(temp.path(), &project, &bin)
        .args(["secrets", "check"])
        .output()
        .unwrap();
    failure(&output, "differs from the governance secret namespace");
    assert!(!temp.path().join("commands.log").exists());
}

#[test]
fn devenv_preserves_literal_arguments_secret_profiles_and_access_reasons() {
    let (temp, project, bin) = fixture();
    let output = cli(temp.path(), &project, &bin)
        .args([
            "dev",
            "run",
            "--profile",
            "dev",
            "--reason",
            "test project command",
            "--",
            "printf",
            "%s",
            "two words",
            "$(touch should-not-exist)",
        ])
        .output()
        .unwrap();
    success(&output);
    let args = log(temp.path());
    assert!(args.contains("arg:--secretspec-profile\narg:dev\narg:shell\narg:--\narg:printf\narg:%s\narg:two words\narg:$(touch should-not-exist)\n"));
    assert!(args.contains("reason:test project command\n"));
    for action in [
        vec!["dev", "up", "--detach"],
        vec!["dev", "task", "security:"],
    ] {
        success(
            &cli(temp.path(), &project, &bin)
                .args(action)
                .output()
                .unwrap(),
        );
    }
    let args = log(temp.path());
    assert!(args.contains("arg:up\narg:-d\n"));
    assert!(args.contains("arg:tasks\narg:run\narg:--\narg:security:\n"));
}

#[test]
fn stopping_processes_does_not_require_governance_or_secret_preflight() {
    let (temp, project, bin) = fixture();
    std::fs::remove_file(project.join("secretspec.toml")).unwrap();
    let output = cli(temp.path(), &project, &bin)
        .env("COLLAR_GOVERNANCE", "/missing")
        .args(["dev", "down"])
        .output()
        .unwrap();
    success(&output);
    assert!(log(temp.path()).contains("arg:processes\narg:down\n"));
}

#[test]
fn repository_checks_run_prek_in_devenv_and_resolve_caller_relative_files() {
    let (temp, project, bin) = fixture();
    let nested = project.join("nested");
    std::fs::create_dir(&nested).unwrap();
    std::fs::write(nested.join("input.rs"), "fn main() {}\n").unwrap();
    let output = cli(temp.path(), &project, &bin)
        .current_dir(&nested)
        .args(["check", "treefmt", "--files", "input.rs"])
        .output()
        .unwrap();
    success(&output);
    let args = log(temp.path());
    assert!(args.contains(
        "arg:shell\narg:--\narg:prek\narg:run\narg:treefmt\narg:--files\narg:./nested/input.rs\n"
    ));
    assert!(args.contains(&format!("cwd:{}\n", project.display())));
    success(
        &cli(temp.path(), &project, &bin)
            .args(["check", "--all"])
            .output()
            .unwrap(),
    );
    assert!(log(temp.path()).contains("arg:--all-files\n"));
    std::fs::remove_file(temp.path().join("commands.log")).unwrap();
    let output = cli(temp.path(), &project, &bin)
        .args(["check", "--files"])
        .arg(temp.path().join("config.toml"))
        .output()
        .unwrap();
    failure(&output, "outside the project checkout");
    assert!(!temp.path().join("commands.log").exists());
}

#[test]
fn tool_failure_exit_codes_are_preserved() {
    let (temp, project, bin) = fixture();
    for args in [
        vec!["secrets", "check"],
        vec!["dev", "run", "--", "false"],
        vec!["check", "--all"],
    ] {
        let output = cli(temp.path(), &project, &bin)
            .env("COLLAR_TEST_EXIT", "7")
            .args(args)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(7),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn bao_bootstrap_works_outside_checkouts_and_does_not_print_token_metadata() {
    let (temp, project, bin) = fixture();
    let output = cli(temp.path(), &project, &bin)
        .current_dir(temp.path())
        .env("BAO_ADDR", "https://wrong.example")
        .args(["auth", "bao", "login"])
        .output()
        .unwrap();
    success(&output);
    let args = log(temp.path());
    assert!(args.contains("arg:shell\narg:nixpkgs#openbao\narg:--command\narg:bao\narg:login\narg:-method=oidc\narg:-no-print\n"));
    assert!(args.contains("bao:https://secrets.scottylabs.org\n"));
    let output = cli(temp.path(), &project, &bin)
        .args(["auth", "bao", "status"])
        .output()
        .unwrap();
    success(&output);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("tool-output"));
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("project secret access has not been checked")
    );
}
