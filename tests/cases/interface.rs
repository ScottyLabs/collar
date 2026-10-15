use super::*;

#[test]
fn interactive_commands_reject_headless_use_before_network_access() {
    for arguments in [vec!["project", "select"], vec!["pr", "browse"]] {
        let output = command()
            .args(["--host", "http://127.0.0.1:1", "-R", "ScottyLabs/terrier"])
            .args(arguments)
            .output()
            .unwrap();
        failure(&output, "requires terminal stdin and stderr");
        assert!(output.stdout.is_empty());
    }
}

#[cfg(unix)]
#[test]
fn closed_output_pipe_terminates_without_a_panic() {
    use std::os::unix::process::ExitStatusExt;
    use std::process::Stdio;

    let mut child = command()
        .args(["completion", "bash"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.signal(), Some(13));
    assert!(output.stderr.is_empty());
}

#[test]
fn nushell_completion_defines_typed_commands() {
    let output = command().args(["completion", "nushell"]).output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let script = String::from_utf8(output.stdout).unwrap();
    assert!(script.contains("export extern collar"));
    assert!(script.contains("export extern \"collar pr view\""));
    assert!(script.contains("--repo(-R): string"));
    assert!(script.contains("export use completions *"));
}
