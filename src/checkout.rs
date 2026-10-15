//! Check out inspected PR heads while preserving local branches and worktree state.

use std::{path::Path, process::Command};

use anyhow::{Context, Result, ensure};

use crate::{
    client::Api,
    pr, process,
    repo::{self, Repo},
};

fn git(root: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(root);
    command
}

fn read(root: &Path, args: &[&str]) -> Result<String> {
    let output = git(root)
        .args(args)
        .output()
        .context("run Git inspection")?;
    ensure!(
        output.status.success(),
        "Git {} failed ({}); inspect the checkout",
        args[0],
        output.status
    );
    String::from_utf8(output.stdout).context("Git inspection returned non-UTF-8 data")
}

fn clean(root: &Path) -> Result<()> {
    ensure!(
        read(
            root,
            &[
                "status",
                "--porcelain=v1",
                "--untracked-files=all",
                "--ignore-submodules=none"
            ]
        )?
        .is_empty(),
        "checkout has uncommitted or untracked changes; commit or stash them before PR checkout"
    );
    Ok(())
}

fn checkout_remote(root: &Path, repo: &Repo, remote: Option<&str>) -> Result<String> {
    let names = read(root, &["remote"])?;
    let remote = remote
        .or_else(|| {
            ["upstream", "origin"]
                .into_iter()
                .find(|candidate| names.lines().any(|name| name == *candidate))
        })
        .context("no upstream or origin remote; provide --remote NAME")?;
    ensure!(
        names.lines().any(|name| name == remote),
        "--remote must name a configured Git remote"
    );
    let key = format!("remote.{remote}.url");
    let url = read(root, &["config", "--get-all", &key])?;
    ensure!(
        url.lines().count() == 1,
        "remote must have exactly one fetch URL"
    );
    let target = Repo::remote(url.trim(), None).context("parse checkout remote")?;
    ensure!(
        target.origin == repo.origin
            && target.owner.eq_ignore_ascii_case(&repo.owner)
            && target.name.eq_ignore_ascii_case(&repo.name),
        "checkout remote does not match the selected repository"
    );
    Ok(remote.to_owned())
}

fn existing_branch(root: &Path, branch: &str, sha: &str) -> Result<bool> {
    ensure!(
        !branch.starts_with('-'),
        "local branch must not start with a hyphen"
    );
    let valid = git(root)
        .args(["check-ref-format", "--branch", branch])
        .output()
        .context("validate local branch")?;
    ensure!(valid.status.success(), "invalid local branch name");
    ensure!(
        std::str::from_utf8(&valid.stdout)?.trim() == branch,
        "local branch must be a literal branch name"
    );
    let local_ref = format!("refs/heads/{branch}");
    let existing = git(root)
        .args(["show-ref", "--verify", "--quiet", &local_ref])
        .status()
        .context("inspect local branch")?;
    let exists = match existing.code() {
        Some(0) => true,
        Some(1) => false,
        _ => anyhow::bail!("Git could not inspect the local branch ({existing})"),
    };
    if exists {
        ensure!(
            read(root, &["rev-parse", "--verify", &local_ref])?.trim() == sha,
            "local branch already exists at a different commit; choose --branch NAME"
        );
    }
    Ok(exists)
}

fn fetch_head(root: &Path, remote: &str, number: u64, sha: &str) -> Result<()> {
    let fetched_ref = format!("refs/collar/pulls/{number}");
    let refspec = format!("+refs/pull/{number}/head:{fetched_ref}");
    process::run(git(root).args([
        "fetch",
        "--no-tags",
        "--no-write-fetch-head",
        "--recurse-submodules=no",
        "--",
        remote,
        &refspec,
    ]))?;
    let commit_ref = format!("{fetched_ref}^{{commit}}");
    ensure!(
        read(root, &["rev-parse", "--verify", &commit_ref])?.trim() == sha,
        "fetched PR head differs from the inspected commit; inspect the PR before retrying"
    );
    Ok(())
}

fn switch_branch(root: &Path, branch: &str, sha: &str, exists: bool) -> Result<()> {
    let mut switch = git(root);
    if exists {
        switch.args([
            "switch",
            "--no-recurse-submodules",
            "--no-overwrite-ignore",
            "--",
            branch,
        ]);
    } else {
        switch.args([
            "switch",
            "--no-recurse-submodules",
            "--no-overwrite-ignore",
            "-c",
            branch,
            sha,
        ]);
    }
    process::run(&mut switch)?;
    ensure!(
        read(root, &["rev-parse", "--verify", "HEAD"])?.trim() == sha,
        "checkout HEAD changed concurrently; inspect the Git state"
    );
    Ok(())
}

pub fn run(
    api: &Api,
    repo: &Repo,
    selector: Option<&str>,
    branch: Option<&str>,
    remote: Option<&str>,
) -> Result<()> {
    let root = repo::root()?;
    clean(&root)?;
    let remote = checkout_remote(&root, repo, remote)?;
    let number = pr::select(api, repo, selector)?;
    let pull = pr::get(api, repo, number)?;
    ensure!(
        pull.number == number,
        "forge returned a different PR number"
    );
    let sha = &pull.head.sha;
    ensure!(
        matches!(sha.len(), 40 | 64) && sha.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "PR head is not a full Git commit ID"
    );
    let default_branch = format!("pr/{number}");
    let branch = branch.unwrap_or(&default_branch);
    let exists = existing_branch(&root, branch, sha)?;
    fetch_head(&root, &remote, number, sha)?;
    clean(&root)?;
    switch_branch(&root, branch, sha, exists)?;
    println!("Checked out PR #{number} at {sha} on {branch}");
    Ok(())
}
