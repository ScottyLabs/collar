use std::{fs, path::PathBuf, process::Command};

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_collar"));
    for variable in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
    ] {
        command.env_remove(variable);
    }
    command
        .env_remove("COLLAR_CONFIG")
        .args(["--no-config", "rfc"]);
    command
}

fn temp_rfcs_dir(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("collar-rfc-{label}-{}", std::process::id()));
    let directory = root.join("rfcs");
    fs::create_dir_all(&directory).unwrap();
    directory
}

#[test]
fn searches_sections_with_status_defaults_and_plain_piped_output() {
    let root = std::env::temp_dir().join(format!("collar-rfc-{}", std::process::id()));
    let directory = root.join("rfcs");
    fs::create_dir_all(&directory).unwrap();
    let source = "# RFC 0001: Storage\n- **Status:** Accepted\n\n## Design\nUse Redis for sessions.\n\n## Alternatives\nPostgres adds load.\n";
    fs::write(directory.join("0001-storage.md"), source).unwrap();
    fs::write(
        directory.join("0002-draft.md"),
        source.replace("Accepted", "Draft"),
    )
    .unwrap();
    fs::write(directory.join("README.md"), source).unwrap();

    let run = |args: &[&str]| {
        let output = command()
            .args(["--no-config", "rfc"])
            .args(args)
            .env("DEVENV_ROOT", &root)
            .env("CLICOLOR", "0")
            .env_remove("CLICOLOR_FORCE")
            .current_dir(std::env::temp_dir())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let output = run(&["STORAGE", "redis"]);
    assert!(output.contains("Use Redis for sessions."));
    assert!(output.contains("0001-storage.md:4"));
    assert!(output.contains("#design"));
    assert!(!output.contains("0002-draft"));
    assert!(!output.contains("README"));
    assert!(!output.contains('\x1b'));
    assert_eq!(run(&["redis", "postgres"]), "No matching RFCs.\n");
    assert!(run(&[]).contains("Accepted"));
    assert!(run(&["--status", "DRAFT"]).contains("0002-draft.md"));
    assert!(run(&["--status=all"]).contains("0002-draft.md"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn provides_help_and_rejects_invalid_options() {
    for args in [vec!["--unknown"], vec!["--status"]] {
        let output = command()
            .args(["--no-config", "rfc"])
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
    }
    let output = command().args(["--help"]).output().unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("accepted"));
    assert!(help.contains("--new"));
}

#[test]
fn searches_repository_code_blocks_in_their_markdown_section() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("rfcs");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("0001-deployment.md"),
        "# RFC 0001: Deployment\n- **Status:** Accepted\n\n## Deployment\n```nix\n# NixOS configuration\n```\n",
    )
    .unwrap();
    let output = command()
        .args(["NixOS", "configuration"])
        .env("DEVENV_ROOT", root.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("#deployment")
    );
}

#[test]
fn new_picks_the_next_number_and_slugifies_the_title() {
    let root = std::env::temp_dir().join(format!("collar-rfc-new-{}", std::process::id()));
    let directory = root.join("rfcs");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("0001-storage.md"), "# RFC 0001: Storage\n").unwrap();
    fs::write(
        directory.join("0007-mobile-architecture.md"),
        "# RFC 0007: Mobile\n",
    )
    .unwrap();

    let output = command()
        .args(["--new", "Rate Limiting!"])
        .env("DEVENV_ROOT", &root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let printed_path = String::from_utf8(output.stdout).unwrap().trim().to_string();
    assert!(printed_path.ends_with("0008-rate-limiting.md"));

    let contents = fs::read_to_string(&printed_path).unwrap();
    assert!(contents.starts_with("# RFC 0008: Rate Limiting!"));
    assert!(contents.contains("- **Status:** Draft"));
    assert!(contents.contains("## Overview"));
    assert!(contents.contains("## Implementation Phases"));

    // Running it again picks the next number rather than clobbering 0008.
    let second = command()
        .args(["--new", "Rate Limiting!"])
        .env("DEVENV_ROOT", &root)
        .output()
        .unwrap();
    assert!(second.status.success());
    let second_path = String::from_utf8(second.stdout).unwrap().trim().to_string();
    assert!(second_path.ends_with("0009-rate-limiting.md"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn surfaces_relations_and_warns_on_missing_status() {
    let directory = temp_rfcs_dir("relations");
    fs::write(
        directory.join("0001-sessions.md"),
        "# RFC 0001: Sessions\n- **Status:** Accepted\n## Design\nUse Redis.\n",
    )
    .unwrap();
    fs::write(
        directory.join("0002-sessions-v2.md"),
        "# RFC 0002: Sessions v2\n- **Status:** Accepted\n- **Supersedes:** 0001\n## Design\nUse Redis with clustering.\n",
    )
    .unwrap();
    fs::write(
        directory.join("0003-no-status.md"),
        "# RFC 0003: Undecided\n## Design\nTBD.\n",
    )
    .unwrap();

    let output = command()
        .args(["--status", "all", "redis"])
        .env("DEVENV_ROOT", directory.parent().unwrap())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Supersedes: 0001"));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("0003-no-status.md"));
    fs::remove_dir_all(directory.parent().unwrap()).unwrap();
}

#[test]
fn ranks_title_matches_above_body_only_matches() {
    let directory = temp_rfcs_dir("ranking");
    // Filename sorts this one first, but the query only hits its body.
    fs::write(
        directory.join("0001-storage.md"),
        "# RFC 0001: Storage\n- **Status:** Accepted\n## Design\nUses caching internally.\n",
    )
    .unwrap();
    // Filename sorts this one second, but the query matches its title.
    fs::write(
        directory.join("0002-caching.md"),
        "# RFC 0002: Caching\n- **Status:** Accepted\n## Design\nSomething else entirely.\n",
    )
    .unwrap();

    let output = command()
        .args(["caching"])
        .env("DEVENV_ROOT", directory.parent().unwrap())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let storage_pos = stdout.find("RFC 0001: Storage").unwrap();
    let caching_pos = stdout.find("RFC 0002: Caching").unwrap();
    assert!(
        caching_pos < storage_pos,
        "title match should rank above body-only match:\n{stdout}"
    );
    fs::remove_dir_all(directory.parent().unwrap()).unwrap();
}

#[test]
fn explicit_directory_searches_standalone_design_and_overrides_devenv() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("RFC.md"),
        "# Collar design\n- **Status:** Draft\n## Identity\nNamespace selection.\n",
    )
    .unwrap();
    let output = command()
        .arg("--directory")
        .arg(temp.path())
        .args(["--status", "all", "namespace"])
        .env("DEVENV_ROOT", "/missing")
        .current_dir("/")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("RFC.md:3"));
    assert!(text.contains("#identity"));
}

#[test]
fn finds_git_root_without_devenv_and_rejects_unsafe_creation() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("rfcs");
    fs::create_dir(&directory).unwrap();
    fs::write(
        directory.join("0001-test.md"),
        "# Test\n- **Status:** Accepted\n",
    )
    .unwrap();
    assert!(
        Command::new("git")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_COMMON_DIR")
            .args(["init", "--quiet"])
            .current_dir(temp.path())
            .status()
            .unwrap()
            .success()
    );
    let output = command()
        .env_remove("DEVENV_ROOT")
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Test"));
    for args in [
        vec!["--new", "Injected\n- **Status:** Accepted"],
        vec!["--new", "New", "query"],
    ] {
        let output = command()
            .arg("--directory")
            .arg(&directory)
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
    }
    fs::write(directory.join("9999-last.md"), "# Last").unwrap();
    let output = command()
        .arg("--directory")
        .arg(&directory)
        .args(["--new", "Overflow"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("maximum is 9999"));
    assert!(!directory.join("10000-overflow.md").exists());
}
