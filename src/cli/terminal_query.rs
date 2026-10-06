use std::io::Read;
use std::process::{Child, Command, Stdio};
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
    finish_helper(&mut child, deadline, max_record_bytes)
}

fn finish_helper(
    child: &mut Child,
    deadline: Instant,
    max_record_bytes: usize,
) -> Result<Option<Vec<u8>>> {
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        match child.try_wait() {
            Ok(Some(status)) => {
                return Ok(if status.success() {
                    read_record(child, max_record_bytes)
                } else {
                    None
                });
            }
            Ok(None) => thread::sleep(remaining.min(HELPER_POLL_INTERVAL)),
            Err(_) => break,
        }
    }
    drop(child.kill());
    let _status = child
        .wait()
        .context("failed to reap the terminal query helper")?;
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

#[cfg(all(test, unix))]
mod tests;
