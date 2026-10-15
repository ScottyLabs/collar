use super::*;
use std::path::{Path, PathBuf};

struct GitFixture {
    _temp: TempDir,
    bare: PathBuf,
    checkout: PathBuf,
    base: String,
    head: String,
}

impl GitFixture {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let bare = temp.path().join("remote.git");
        let source = temp.path().join("source");
        let checkout = temp.path().join("checkout");
        git(temp.path(), ["init", "--bare", bare.to_str().unwrap()]);
        git(temp.path(), ["init", source.to_str().unwrap()]);
        git(&source, ["config", "user.name", "Test User"]);
        git(&source, ["config", "user.email", "test@example.invalid"]);
        std::fs::write(source.join("base.txt"), "base\n").unwrap();
        git(&source, ["add", "base.txt"]);
        git(&source, ["commit", "-m", "base"]);
        let base = git_out(&source, ["rev-parse", "HEAD"]);
        std::fs::write(source.join("feature.txt"), "feature\n").unwrap();
        git(&source, ["add", "feature.txt"]);
        git(&source, ["commit", "-m", "feature"]);
        let head = git_out(&source, ["rev-parse", "HEAD"]);
        git(
            &source,
            [
                "push",
                bare.to_str().unwrap(),
                &format!("{base}:refs/heads/main"),
            ],
        );
        git(
            &source,
            [
                "push",
                bare.to_str().unwrap(),
                &format!("{head}:refs/pull/42/head"),
            ],
        );
        git(
            temp.path(),
            [
                "--git-dir",
                bare.to_str().unwrap(),
                "symbolic-ref",
                "HEAD",
                "refs/heads/main",
            ],
        );
        git(
            temp.path(),
            ["clone", bare.to_str().unwrap(), checkout.to_str().unwrap()],
        );
        Self {
            _temp: temp,
            bare,
            checkout,
            base,
            head,
        }
    }

    fn configure_remote(&self, origin: &str) {
        let url = format!("{origin}/ScottyLabs/terrier.git");
        git(&self.checkout, ["remote", "set-url", "origin", &url]);
        git(
            &self.checkout,
            [
                "config",
                &format!("url.file://{}.insteadOf", self.bare.display()),
                &url,
            ],
        );
    }

    fn command(&self, server: &Server) -> Command {
        let mut command = server.command(false);
        command.current_dir(&self.checkout);
        command
    }

    fn head(&self) -> String {
        git_out(&self.checkout, ["rev-parse", "HEAD"])
    }

    fn branch(&self) -> String {
        git_out(&self.checkout, ["branch", "--show-current"])
    }
}

fn git<const N: usize>(directory: &Path, args: [&str; N]) {
    git_out(directory, args);
}

fn git_out<const N: usize>(directory: &Path, args: [&str; N]) -> String {
    let output = git_command()
        .current_dir(directory)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn checkout_fetches_pr_ref_and_reuses_branch_at_the_same_commit() {
    let fixture = GitFixture::new();
    let server = Server::with_responses(|origin| {
        fixture.configure_remote(origin);
        vec![
            Response::json(pull(&fixture.head)),
            Response::json(pull(&fixture.head)),
        ]
    });
    let first = fixture
        .command(&server)
        .args(["pr", "checkout", "42"])
        .output()
        .unwrap();
    success(&first);
    assert_eq!(fixture.head(), fixture.head);
    assert_eq!(fixture.branch(), "pr/42");
    assert_eq!(
        std::fs::read_to_string(fixture.checkout.join("feature.txt")).unwrap(),
        "feature\n"
    );
    git(&fixture.checkout, ["switch", "main"]);
    let second = fixture
        .command(&server)
        .args(["pr", "checkout", "42"])
        .output()
        .unwrap();
    success(&second);
    assert_eq!(fixture.head(), fixture.head);
    assert_eq!(fixture.branch(), "pr/42");
    assert_eq!(server.finish().len(), 2);
}

#[test]
fn dirty_checkout_and_wrong_remote_fail_before_api_request() {
    let fixture = GitFixture::new();
    let server = Server::with_responses(|origin| {
        fixture.configure_remote(origin);
        vec![]
    });
    std::fs::write(fixture.checkout.join("untracked.txt"), "dirty\n").unwrap();
    let dirty = fixture
        .command(&server)
        .args(["pr", "checkout", "42"])
        .output()
        .unwrap();
    failure(&dirty, "uncommitted or untracked changes");
    std::fs::remove_file(fixture.checkout.join("untracked.txt")).unwrap();
    git(
        &fixture.checkout,
        [
            "remote",
            "set-url",
            "origin",
            "https://example.invalid/other/repo.git",
        ],
    );
    let wrong = fixture
        .command(&server)
        .args(["pr", "checkout", "42"])
        .output()
        .unwrap();
    failure(&wrong, "checkout remote does not match");
    assert!(server.finish().is_empty());
}

#[test]
fn fetched_head_mismatch_leaves_current_branch_and_head_unchanged() {
    let fixture = GitFixture::new();
    let before_head = fixture.head();
    let before_branch = fixture.branch();
    let server = Server::with_responses(|origin| {
        fixture.configure_remote(origin);
        vec![Response::json(pull(&fixture.base))]
    });
    let output = fixture
        .command(&server)
        .args(["pr", "checkout", "42"])
        .output()
        .unwrap();
    failure(&output, "fetched PR head differs");
    assert_eq!(fixture.head(), before_head);
    assert_eq!(fixture.branch(), before_branch);
    assert_eq!(server.finish().len(), 1);
}

#[test]
fn existing_branch_at_other_commit_is_never_reset() {
    let fixture = GitFixture::new();
    git(&fixture.checkout, ["branch", "pr/42", &fixture.base]);
    let server = Server::with_responses(|origin| {
        fixture.configure_remote(origin);
        vec![Response::json(pull(&fixture.head))]
    });
    let output = fixture
        .command(&server)
        .args(["pr", "checkout", "42"])
        .output()
        .unwrap();
    failure(&output, "local branch already exists at a different commit");
    assert_eq!(
        git_out(&fixture.checkout, ["rev-parse", "refs/heads/pr/42"]),
        fixture.base
    );
    assert_eq!(server.finish().len(), 1);
}

#[test]
fn invalid_branch_name_fails_without_switching() {
    let fixture = GitFixture::new();
    let before_head = fixture.head();
    let server = Server::with_responses(|origin| {
        fixture.configure_remote(origin);
        vec![Response::json(pull(&fixture.head))]
    });
    let output = fixture
        .command(&server)
        .args(["pr", "checkout", "42", "--branch", "bad name"])
        .output()
        .unwrap();
    failure(&output, "invalid local branch name");
    assert_eq!(fixture.head(), before_head);
    assert_eq!(server.finish().len(), 1);
}

#[test]
fn checkout_does_not_overwrite_ignored_local_files() {
    let fixture = GitFixture::new();
    std::fs::write(fixture.checkout.join(".git/info/exclude"), "feature.txt\n").unwrap();
    std::fs::write(fixture.checkout.join("feature.txt"), "local contents\n").unwrap();
    let server = Server::with_responses(|origin| {
        fixture.configure_remote(origin);
        vec![Response::json(pull(&fixture.head))]
    });
    let output = fixture
        .command(&server)
        .args(["pr", "checkout", "42"])
        .output()
        .unwrap();
    failure(&output, "git exited");
    assert_eq!(fixture.head(), fixture.base);
    assert_eq!(fixture.branch(), "main");
    assert_eq!(
        std::fs::read_to_string(fixture.checkout.join("feature.txt")).unwrap(),
        "local contents\n"
    );
    assert_eq!(server.finish().len(), 1);
}
