use super::*;

#[test]
fn bundled_skill_is_available_without_configuration_or_network() {
    let directory = TempDir::new().unwrap();
    let config = directory.path().join("bad.toml");
    std::fs::write(&config, "not valid TOML").unwrap();
    let output = isolated_command()
        .args(["--config", config.to_str().unwrap(), "skill"])
        .output()
        .unwrap();
    success(&output);
    assert!(
        output.stdout.starts_with(b"---\n"),
        "bundled skill must retain YAML frontmatter"
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        include_str!("../../docs/skills/collar/SKILL.md")
    );
}

#[test]
fn skill_installation_is_idempotent_and_preserves_local_edits_without_force() {
    let directory = TempDir::new().unwrap();
    let root = directory.path().join("skills");
    let file = root.join("collar/SKILL.md");
    let install = || {
        command()
            .args(["skill", "install"])
            .arg(&root)
            .output()
            .unwrap()
    };
    success(&install());
    let content = std::fs::read(&file).unwrap();
    success(&install());
    assert_eq!(std::fs::read(&file).unwrap(), content);
    std::fs::write(&file, "local customization").unwrap();
    failure(&install(), "use --force");
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "local customization"
    );
    success(
        &command()
            .args(["skill", "install", "--force"])
            .arg(&root)
            .output()
            .unwrap(),
    );
    assert_eq!(std::fs::read(&file).unwrap(), content);
}

#[cfg(unix)]
#[test]
fn skill_installation_does_not_overwrite_a_symlink_target() {
    let directory = TempDir::new().unwrap();
    let target = directory.path().join("custom");
    std::fs::write(&target, "preserve me").unwrap();
    let root = directory.path().join("skills");
    std::fs::create_dir_all(root.join("collar")).unwrap();
    std::os::unix::fs::symlink(&target, root.join("collar/SKILL.md")).unwrap();
    let output = command()
        .args(["skill", "install", "--force"])
        .arg(&root)
        .output()
        .unwrap();
    failure(&output, "must be a regular file");
    assert_eq!(std::fs::read_to_string(target).unwrap(), "preserve me");
}
