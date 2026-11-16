//! Supervise tool processes independently of the MCP protocol connection.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use serde_json::Value;

const OUTPUT_LIMIT: usize = 256 * 1024;
// A worker result can contain two bounded streams, escaped as JSON strings.
const WORKER_LIMIT: usize = OUTPUT_LIMIT * 12 + 4096;
const POLL_INTERVAL: Duration = Duration::from_millis(20);
const CANCELLED: &str =
    "Tool cancelled. A write may already have completed; inspect the resource before retrying.";

pub(super) fn execute(
    executable: &Path,
    argv: &[OsString],
    cancelled: &AtomicBool,
) -> Result<Value> {
    if cancelled.load(Ordering::Acquire) {
        return Ok(super::text_result(
            "Tool cancelled before execution.".into(),
            true,
        ));
    }
    let mut command = Command::new(executable);
    command.args(["mcp", "--worker", "--"]).args(argv);
    let mut child = Guard {
        child: command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("start MCP tool supervisor")?,
        supervisor: true,
    };
    let mut lifetime = child.child.stdin.take();
    let stdout = capture(
        child
            .child
            .stdout
            .take()
            .context("read supervisor output")?,
        WORKER_LIMIT,
    );
    let stderr = capture(
        child
            .child
            .stderr
            .take()
            .context("read supervisor errors")?,
        OUTPUT_LIMIT,
    );
    let status = loop {
        if cancelled.load(Ordering::Acquire) {
            // EOF also reaches the worker when the MCP process is killed.
            drop(lifetime.take());
        }
        if let Some(status) = child
            .child
            .try_wait()
            .context("wait for MCP tool supervisor")?
        {
            break status;
        }
        thread::sleep(POLL_INTERVAL);
    };
    drop(lifetime);
    let stdout = join(stdout)?;
    let stderr = join(stderr)?;
    if cancelled.load(Ordering::Acquire) {
        return Ok(super::text_result(CANCELLED.into(), true));
    }
    ensure!(
        status.success(),
        "MCP tool supervisor failed: {}",
        stderr.text()
    );
    ensure!(
        !stdout.truncated,
        "MCP tool supervisor returned excessive output"
    );
    serde_json::from_slice(&stdout.bytes).context("decode MCP tool result")
}

pub(super) fn worker(argv: &[OsString]) -> Result<()> {
    let disconnected = Arc::new(AtomicBool::new(false));
    let input_closed = Arc::clone(&disconnected);
    // This thread need not be joined: it owns only stdin, and process exit ends it after the tool has been reaped.
    thread::spawn(move || {
        let mut byte = [0];
        let stdin = std::io::stdin();
        let mut input = stdin.lock();
        loop {
            match input.read(&mut byte) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
        input_closed.store(true, Ordering::Release);
    });
    let mut child = Guard {
        child: Command::new(std::env::current_exe().context("locate collar executable")?)
            .args(argv)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("start collar tool")?,
        supervisor: false,
    };
    let stdout = capture(
        child.child.stdout.take().context("read tool output")?,
        OUTPUT_LIMIT,
    );
    let stderr = capture(
        child.child.stderr.take().context("read tool errors")?,
        OUTPUT_LIMIT,
    );
    let status = loop {
        if disconnected.load(Ordering::Acquire) {
            child.stop();
            break child.child.wait().context("reap cancelled tool")?;
        }
        if let Some(status) = child.child.try_wait().context("wait for collar tool")? {
            break status;
        }
        thread::sleep(POLL_INTERVAL);
    };
    let stdout = join(stdout)?;
    let stderr = join(stderr)?;
    let result = if disconnected.load(Ordering::Acquire) {
        super::text_result(CANCELLED.into(), true)
    } else if status.success() {
        super::text_result(stdout.text(), false)
    } else {
        let mut text = stderr.text().trim_end().to_owned();
        if !stdout.bytes.is_empty() {
            text.push_str("\n\n");
            text.push_str(&stdout.text());
        }
        super::text_result(text, true)
    };
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer(&mut output, &result).context("encode tool result")?;
    output.flush().context("write tool result")
}

struct Guard {
    child: Child,
    supervisor: bool,
}

impl Guard {
    fn stop(&mut self) {
        if self.supervisor {
            // Give the supervisor time to observe its lifetime pipe closing and reap the tool before killing an unresponsive supervisor.
            drop(self.child.stdin.take());
            for _ in 0..50 {
                if matches!(self.child.try_wait(), Ok(Some(_))) {
                    return;
                }
                thread::sleep(POLL_INTERVAL);
            }
        }
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Captured {
    bytes: Vec<u8>,
    truncated: bool,
}

impl Captured {
    fn text(&self) -> String {
        let bytes = match std::str::from_utf8(&self.bytes) {
            Err(error) if self.truncated && error.error_len().is_none() => {
                &self.bytes[..error.valid_up_to()]
            }
            _ => &self.bytes,
        };
        let mut text = String::from_utf8_lossy(bytes).into_owned();
        if self.truncated {
            text.push_str("\n[output truncated; request fewer fields or a narrower selection]");
        }
        text
    }
}

fn capture(
    reader: impl Read + Send + 'static,
    limit: usize,
) -> thread::JoinHandle<Result<Captured>> {
    thread::spawn(move || read_bounded(reader, limit))
}

fn read_bounded(mut reader: impl Read, limit: usize) -> Result<Captured> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    let mut truncated = false;
    loop {
        let count = reader.read(&mut buffer).context("read child stream")?;
        if count == 0 {
            break;
        }
        let keep = count.min(limit.saturating_sub(bytes.len()));
        bytes.extend_from_slice(&buffer[..keep]);
        truncated |= keep < count;
    }
    // Keep draining above even after the limit, so a full pipe cannot block a tool that still needs to finish its request.
    Ok(Captured { bytes, truncated })
}

fn join(handle: thread::JoinHandle<Result<Captured>>) -> Result<Captured> {
    handle
        .join()
        .map_err(|_| anyhow::anyhow!("child stream reader panicked"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_drains_but_retains_only_the_limit() {
        let output = read_bounded(
            std::io::Cursor::new(vec![b'x'; OUTPUT_LIMIT * 3]),
            OUTPUT_LIMIT,
        )
        .unwrap();
        assert_eq!(output.bytes.len(), OUTPUT_LIMIT);
        assert!(output.truncated);
        assert!(output.text().ends_with("narrower selection]"));
    }

    #[test]
    fn capture_does_not_mark_exact_limit_as_truncated() {
        let output = read_bounded(std::io::Cursor::new(vec![b'x'; 16]), 16).unwrap();
        assert_eq!(output.text(), "x".repeat(16));
        assert!(!output.truncated);
    }

    #[test]
    fn cancelled_request_does_not_spawn_a_supervisor() {
        let output = execute(Path::new("/does/not/exist"), &[], &AtomicBool::new(true)).unwrap();
        assert_eq!(output["isError"], true);
        assert!(
            output["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("before execution")
        );
    }

    #[test]
    fn truncated_utf8_is_still_valid_text() {
        let output = read_bounded(std::io::Cursor::new("aéé".as_bytes()), 4).unwrap();
        assert!(output.truncated);
        assert!(output.text().starts_with("aé"));
        assert!(!output.text().contains('\u{fffd}'));
    }

    #[cfg(unix)]
    #[test]
    fn both_pipes_are_drained_without_blocking_the_child() {
        let mut child = Guard {
            child: Command::new("sh")
                .args(["-c", "i=0; while [ $i -lt 10000 ]; do printf 'stdout line\\n'; printf 'stderr line\\n' >&2; i=$((i+1)); done"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
            supervisor: false,
        };
        let stdout = capture(child.child.stdout.take().unwrap(), 16);
        let stderr = capture(child.child.stderr.take().unwrap(), 16);
        assert!(child.child.wait().unwrap().success());
        for stream in [join(stdout).unwrap(), join(stderr).unwrap()] {
            assert_eq!(stream.bytes.len(), 16);
            assert!(stream.truncated);
        }
    }

    #[cfg(unix)]
    #[test]
    fn stop_kills_and_reaps_the_tool() {
        let mut child = Guard {
            child: Command::new("sleep").arg("30").spawn().unwrap(),
            supervisor: false,
        };
        child.stop();
        assert!(!child.child.try_wait().unwrap().unwrap().success());
    }
}
