use super::*;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn nix_fixture(root: &Path) -> PathBuf {
    let bin = root.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let nix = bin.join("nix");
    std::fs::write(
        &nix,
        "#!/bin/sh\nprintf 'cwd:%s\\n' \"$PWD\" >> \"$COLLAR_TEST_LOG\"\nfor value in \"$@\"; do printf 'arg:%s\\n' \"$value\" >> \"$COLLAR_TEST_LOG\"; done\nexit \"${COLLAR_TEST_EXIT:-0}\"\n",
    )
    .unwrap();
    std::fs::set_permissions(nix, std::fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

fn local_flake(root: &Path) -> PathBuf {
    let checkout = root.join("infrastructure");
    std::fs::create_dir_all(checkout.join("modules/hosts/infra-01")).unwrap();
    std::fs::create_dir_all(checkout.join("modules/roles")).unwrap();
    std::fs::write(checkout.join("flake.nix"), "{}").unwrap();
    std::fs::write(checkout.join("flake.lock"), "{}").unwrap();
    std::fs::write(checkout.join("devenv.nix"), "{}").unwrap();
    std::fs::write(
        checkout.join("modules/hosts/infra-01/configuration.nix"),
        "{}",
    )
    .unwrap();
    std::fs::write(checkout.join("modules/roles/infra-01.nix"), "{}").unwrap();
    checkout
}

fn local_command(root: &Path, checkout: &Path, bin: &Path) -> Command {
    let mut command = command();
    command
        .args(["--host", "http://127.0.0.1:1", "-R", "ScottyLabs/terrier"])
        .env("COLLAR_INFRASTRUCTURE", checkout)
        .env("COLLAR_TEST_LOG", root.join("nix.log"))
        .env("PATH", bin);
    command
}

fn log(root: &Path) -> String {
    std::fs::read_to_string(root.join("nix.log")).unwrap()
}

#[test]
fn builds_local_host_with_exact_flake_selector_and_preserves_exit_status() {
    let temp = TempDir::new().unwrap();
    let checkout = local_flake(temp.path());
    let bin = nix_fixture(temp.path());
    let output = local_command(temp.path(), &checkout, &bin)
        .args(["infra", "build", "infra-01"])
        .output()
        .unwrap();
    success(&output);
    let args = log(temp.path());
    assert!(args.contains(&format!("cwd:{}\n", checkout.display())));
    assert!(args.contains(&format!(
        "arg:{}#nixosConfigurations.infra-01.config.system.build.toplevel\n",
        checkout.display()
    )));
    assert!(args.contains("arg:--no-link\n"));

    let output = local_command(temp.path(), &checkout, &bin)
        .env("COLLAR_TEST_EXIT", "9")
        .args(["infra", "build", "infra-01"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(9));
}

#[test]
fn rejects_unknown_host_before_invoking_nix() {
    let temp = TempDir::new().unwrap();
    let checkout = local_flake(temp.path());
    let bin = nix_fixture(temp.path());
    let output = local_command(temp.path(), &checkout, &bin)
        .args(["infra", "build", "missing"])
        .output()
        .unwrap();
    failure(&output, "unknown infrastructure host missing");
    assert!(!temp.path().join("nix.log").exists());
}

#[test]
fn pins_remote_flake_to_latest_snapshot_and_propagates_offline_to_nix() {
    let temp = TempDir::new().unwrap();
    let bin = nix_fixture(temp.path());
    let files = [
        ("flake.nix", "{ outputs = _: {}; }"),
        ("modules/hosts/infra-01/configuration.nix", "{}"),
        ("modules/roles/infra-01.nix", "{}"),
    ];
    let mut responses = source_head();
    responses.push(Response::json(json!({
        "page": 1,
        "total_count": files.len(),
        "truncated": false,
        "tree": files.iter().map(|(path, content)| tree_entry(path, content)).collect::<Vec<_>>()
    })));
    responses.extend(files.iter().map(|(_, content)| Response {
        status: 200,
        body: content.to_string(),
        headers: String::new(),
    }));
    let server = Server::new(responses);
    let origin = server.origin.clone();
    let output = command()
        .args(["--host", &server.origin, "--cache-dir"])
        .arg(temp.path())
        .env("PATH", &bin)
        .env("COLLAR_TEST_LOG", temp.path().join("nix.log"))
        .args(["infra", "atlantis-config"])
        .output()
        .unwrap();
    success(&output);
    let requests = server.finish();
    assert_eq!(requests.len(), 6);
    let output = command()
        .args(["--host", &origin, "--cache-dir"])
        .arg(temp.path())
        .env("PATH", &bin)
        .env("COLLAR_TEST_LOG", temp.path().join("nix.log"))
        .args(["infra", "atlantis-config", "--offline-source"])
        .output()
        .unwrap();
    success(&output);
    let args = log(temp.path());
    assert!(args.contains("arg:--offline\n"));
    assert!(args.contains(&format!(
        "arg:git+{origin}/ScottyLabs/infrastructure?rev={SOURCE_REVISION}#atlantis-yaml\n"
    )));
    assert!(args.contains("arg:--no-link\n"));
}
