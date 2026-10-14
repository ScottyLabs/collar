use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};

pub fn check(command: &mut Command, timeout: Duration) -> Result<()> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("start {}", command.get_program().to_string_lossy()))?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().context("check diagnostic process")? {
            ensure!(status.success(), "command exited with {status}");
            return Ok(());
        }
        if started.elapsed() >= timeout {
            let termination = child.kill();
            let reaped = child.wait();
            termination.context("terminate timed-out diagnostic process")?;
            reaped.context("reap timed-out diagnostic process")?;
            bail!("command exceeded {} seconds", timeout.as_secs_f64());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub fn run(command: &mut Command) -> Result<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    let status = command
        .status()
        .with_context(|| format!("start {program}"))?;
    if !status.success() {
        return Err(ToolExit { program, status }.into());
    }
    Ok(())
}

pub struct ToolExit {
    program: String,
    status: std::process::ExitStatus,
}

impl ToolExit {
    pub fn code(&self) -> u8 {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            if let Some(signal) = self.status.signal() {
                return u8::try_from(128 + signal).unwrap_or(1);
            }
        }
        self.status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .unwrap_or(1)
    }
}
impl std::fmt::Display for ToolExit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} exited with {}", self.program, self.status)
    }
}
impl std::fmt::Debug for ToolExit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}
impl std::error::Error for ToolExit {}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn bounds_checks_and_propagates_failures() {
        check(
            Command::new("sh").args(["-c", "exit 0"]),
            Duration::from_secs(1),
        )
        .unwrap();
        assert!(
            check(
                Command::new("sh").args(["-c", "exit 7"]),
                Duration::from_secs(1)
            )
            .is_err()
        );
        let started = Instant::now();
        let error = check(
            Command::new("sh").args(["-c", "exec sleep 5"]),
            Duration::from_millis(50),
        )
        .unwrap_err();
        assert!(error.to_string().contains("exceeded"));
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
