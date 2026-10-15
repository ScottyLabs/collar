use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

pub(crate) fn clear_git_repository_env(command: &mut Command) {
    for variable in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_PREFIX",
        "GIT_SHALLOW_FILE",
    ] {
        command.env_remove(variable);
    }
}

pub(crate) fn git_command() -> Command {
    let mut command = Command::new("git");
    clear_git_repository_env(&mut command);
    command.args([
        "-c",
        "commit.gpgsign=false",
        "-c",
        "core.hooksPath=/dev/null",
    ]);
    command
}

pub(crate) fn isolated_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_collar"));
    clear_git_repository_env(&mut command);
    command
        .env_remove("COLLAR_CACHE_DIR")
        .env_remove("COLLAR_CONFIG")
        .env_remove("COLLAR_HOST")
        .env_remove("COLLAR_TOKEN")
        .env_remove("COLLAR_GOVERNANCE")
        .env_remove("COLLAR_INFRASTRUCTURE");
    command
}

pub(crate) fn command() -> Command {
    let mut command = isolated_command();
    command.arg("--no-config");
    command
}

pub(crate) struct Response {
    pub(crate) status: u16,
    pub(crate) body: String,
    pub(crate) headers: String,
}

impl Response {
    pub(crate) fn json(body: Value) -> Self {
        Self {
            status: 200,
            body: body.to_string(),
            headers: String::new(),
        }
    }
}

pub(crate) struct Server {
    pub(crate) origin: String,
    task: thread::JoinHandle<Vec<String>>,
}

impl Server {
    pub(crate) fn new(responses: Vec<Response>) -> Self {
        Self::with_responses(|_| responses)
    }

    pub(crate) fn with_responses(responses: impl FnOnce(&str) -> Vec<Response>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let responses = responses(&origin);
        let task = thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
                let deadline = Instant::now() + Duration::from_secs(10);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                Instant::now() < deadline,
                                "expected request was not received"
                            );
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("accept: {error}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0; 4096];
                loop {
                    let count = stream.read(&mut buffer).unwrap();
                    assert!(count > 0, "request ended before its body");
                    bytes.extend_from_slice(&buffer[..count]);
                    if let Some(index) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                        let header = std::str::from_utf8(&bytes[..index]).unwrap();
                        let length: usize = header
                            .lines()
                            .find_map(|line| {
                                line.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|value| value.trim().parse().unwrap())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= index + 4 + length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8(bytes).unwrap());
                write!(
                    stream,
                    "HTTP/1.1 {} Test\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n{}",
                    response.status,
                    response.body.len(),
                    response.headers,
                    response.body
                )
                .unwrap();
            }
            requests
        });
        Self { origin, task }
    }

    pub(crate) fn command(&self, token: bool) -> Command {
        let mut command = command();
        command
            .env("COLLAR_HOST", &self.origin)
            .args(["-R", "ScottyLabs/terrier"]);
        if token {
            command.env("COLLAR_TOKEN", "test-secret");
        }
        command
    }

    pub(crate) fn finish(self) -> Vec<String> {
        self.task.join().unwrap()
    }
}

pub(crate) fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(crate) fn failure(output: &Output, message: &str) {
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(message),
        "expected {message:?}, got {stderr}"
    );
    assert!(!stderr.contains("test-secret"));
}

pub(crate) fn pull(sha: &str) -> Value {
    json!({ "number": 42, "title": "Review me", "body": null, "state": "open", "merged": false, "draft": false, "html_url": "https://git.cmu.dev/ScottyLabs/terrier/pulls/42", "user": { "login": "contributor" }, "head": { "ref": "feat/collar", "sha": sha }, "base": { "ref": "main", "sha": "base" } })
}

pub(crate) fn project_governance(checkout: &std::path::Path) {
    std::fs::create_dir_all(checkout.join("data/teams")).unwrap();
    std::fs::write(checkout.join("data/org.toml"), "[org.forgejo]\norg = 'ScottyLabs'\nurl = 'https://git.cmu.dev'\n[org.keycloak]\nurl = 'https://idp.scottylabs.org'\nrealm = 'scottylabs'\n").unwrap();
    std::fs::write(checkout.join("data/teams/team.toml"), "[team]\nslug = 'tech'\nname = 'Tech'\nleads = ['Lead']\n[[team.projects]]\nslug = 'collie'\nname = 'Collie'\nmembers = ['member']\n[[team.projects.repos]]\nname = 'collie-api'\n[team.projects.repos.features]\nkennel = {}\n[[team.projects.repos]]\nname = 'collie-web'\n[team.projects.repos.features]\nsentry = {}\n").unwrap();
}

#[cfg(unix)]
pub(crate) fn doctor_fixture(temp: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let bin = temp.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let git = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|directory| directory.join("git"))
        .find(|path| path.is_file())
        .unwrap();
    symlink(git, bin.join("git")).unwrap();
    for tool in ["nix", "devenv", "bao", "secretspec", "ssh-add"] {
        let file = bin.join(tool);
        std::fs::write(&file, format!("#!/bin/sh\nprintf '%s %s\\n' '{tool}' \"$*\" >> \"$COLLAR_TEST_LOG\"\nprintf '%s\\n' 'never-print-tool-secret'\nexit 0\n")).unwrap();
        std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let project = temp.join("project");
    let governance = temp.join("governance");
    let infrastructure = temp.join("infrastructure");
    std::fs::create_dir(&project).unwrap();
    for file in ["devenv.nix", "devenv.yaml", "flake.nix"] {
        std::fs::write(project.join(file), "{}").unwrap();
    }
    std::fs::write(
        project.join("secretspec.toml"),
        "[project]\nname = 'collie'\nrevision = '1.0'\n[profiles.dev]\n",
    )
    .unwrap();
    project_governance(&governance);
    std::fs::create_dir_all(infrastructure.join("modules/hosts/test")).unwrap();
    std::fs::create_dir_all(infrastructure.join("modules/roles")).unwrap();
    std::fs::write(
        infrastructure.join("modules/hosts/test/configuration.nix"),
        "{}",
    )
    .unwrap();
    std::fs::write(infrastructure.join("modules/roles/test.nix"), "{}").unwrap();
    for checkout in [&project, &governance, &infrastructure] {
        for args in [
            vec!["init", "--quiet", "--initial-branch=main"],
            vec!["config", "core.hooksPath", "/dev/null"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["config", "user.name", "Test"],
            vec!["config", "user.email", "test@example.test"],
            vec!["add", "."],
            vec!["commit", "--quiet", "-m", "test"],
            vec!["config", "branch.main.remote", "."],
            vec!["config", "branch.main.merge", "refs/heads/main"],
        ] {
            assert!(
                git_command()
                    .args(args)
                    .current_dir(checkout)
                    .status()
                    .unwrap()
                    .success()
            );
        }
    }
    std::fs::write(temp.join("config.toml"), "").unwrap();
    (project, bin)
}

#[cfg(unix)]
pub(crate) fn doctor_command(
    temp: &std::path::Path,
    project: &std::path::Path,
    bin: &std::path::Path,
) -> Command {
    let mut command = isolated_command();
    command
        .arg("--config")
        .arg(temp.join("config.toml"))
        .args(["-R", "ScottyLabs/collie-api", "doctor", "--json"])
        .env("PATH", bin)
        .env("COLLAR_TEST_LOG", temp.join("commands.log"))
        .arg("--governance")
        .arg(temp.join("governance"))
        .arg("--infrastructure")
        .arg(temp.join("infrastructure"))
        .current_dir(project);
    command
}

pub(crate) const SOURCE_REVISION: &str = "1111111111111111111111111111111111111111";

pub(crate) fn source_head() -> Vec<Response> {
    vec![
        Response::json(json!({"default_branch": "main"})),
        Response::json(json!({"commit": {"id": SOURCE_REVISION}})),
    ]
}

pub(crate) fn tree_entry(path: &str, content: &str) -> Value {
    use sha1::Digest;
    let sha = format!(
        "{:x}",
        sha1::Sha1::digest(format!("blob {}\0{content}", content.len()).as_bytes())
    );
    json!({"path": path, "type": "blob", "mode": "100644", "size": content.len(), "sha": sha})
}
