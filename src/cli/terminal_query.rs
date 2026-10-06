use std::io::Read;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

const HELPER_POLL_INTERVAL: Duration = Duration::from_millis(5);

pub(crate) fn query_helper(
    argument: &str,
    budget: Duration,
    max_record_bytes: usize,
) -> Result<Option<Vec<u8>>> {
    // nosemgrep: rust.lang.security.current-exe.current-exe
    let Ok(executable) = std::env::current_exe() else {
        return Ok(None);
    };
    let deadline = Instant::now() + budget;
    let Ok(mut child) = Command::new(executable)
        .arg(argument)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped())
        .spawn()
    else {
        return Ok(None);
    };
    finish_query(&mut child, deadline, max_record_bytes)
}

pub(crate) fn enable_tmux_passthrough(budget: Duration) -> Result<()> {
    let deadline = Instant::now() + budget;
    let Ok(mut child) = Command::new("tmux")
        .args(["set", "-p", "allow-passthrough", "on"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return Ok(());
    };
    let _status = wait_for_child(&mut child, deadline)?;
    Ok(())
}

fn finish_query(
    child: &mut Child,
    deadline: Instant,
    max_record_bytes: usize,
) -> Result<Option<Vec<u8>>> {
    Ok(wait_for_child(child, deadline)?
        .filter(ExitStatus::success)
        .and_then(|_status| read_record(child, max_record_bytes)))
}

fn wait_for_child(child: &mut Child, deadline: Instant) -> Result<Option<ExitStatus>> {
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(Some(status)),
            Ok(None) => thread::sleep(remaining.min(HELPER_POLL_INTERVAL)),
            Err(_) => break,
        }
    }
    drop(child.kill());
    let _status = child.wait().context("failed to reap the terminal helper")?;
    Ok(None)
}

fn read_record(child: &mut Child, max_record_bytes: usize) -> Option<Vec<u8>> {
    let stderr = child.stderr.take()?;
    let mut bytes = Vec::with_capacity(max_record_bytes);
    let _read = stderr
        .take(max_record_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= max_record_bytes).then_some(bytes)
}

#[cfg(test)]
pub(crate) fn test_without_tmux(test_name: &str) -> Result<()> {
    // nosemgrep: rust.lang.security.current-exe.current-exe
    let executable = std::env::current_exe()?;
    let mut child = Command::new(executable)
        .args(["--exact", test_name, "--nocapture"])
        .env("TERM", "dumb")
        .env_remove("TERM_PROGRAM")
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .stdin(Stdio::null())
        .spawn()?;
    let status = wait_for_child(&mut child, Instant::now() + Duration::from_secs(5))?
        .context("isolated encoding test timed out")?;
    anyhow::ensure!(status.success(), "isolated encoding test failed");
    Ok(())
}

#[cfg(all(test, unix))]
mod tests;
