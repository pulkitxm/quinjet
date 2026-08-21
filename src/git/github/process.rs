#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(crate) struct BoundedOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
}

#[expect(
    clippy::large_stack_arrays,
    reason = "the read buffer is deliberately one page of stack"
)]
pub(crate) fn run_bounded_command(
    command: &mut Command,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<BoundedOutput> {
    let _ = command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .context("child process did not expose stdout")?;
    let stderr = child
        .stderr
        .take()
        .context("child process did not expose stderr")?;
    let stderr_reader = thread::spawn(move || read_and_drain(stderr, stderr_limit));

    let mut collected = Vec::with_capacity(stdout_limit.min(64 * 1024));
    let mut buffer = [0_u8; 64 * 1024];
    let mut truncated = false;
    loop {
        let read = match stdout.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                drop(child.kill());
                drop(child.wait());
                drop(stderr_reader.join());
                return Err(error.into());
            }
        };
        let remaining = stdout_limit.saturating_sub(collected.len());
        if read > remaining {
            collected.extend_from_slice(buffer.get(..remaining).unwrap_or(&buffer));
            truncated = true;
            drop(child.kill());
            break;
        }
        collected.extend_from_slice(buffer.get(..read).unwrap_or(&buffer));
    }
    drop(stdout);
    let status = child.wait()?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| anyhow::anyhow!("stderr reader thread panicked"))??;
    Ok(BoundedOutput {
        status,
        stdout: collected,
        stderr,
        stdout_truncated: truncated,
    })
}

#[expect(
    clippy::large_stack_arrays,
    reason = "the read buffer is deliberately one page of stack"
)]
pub(super) fn read_and_drain(mut reader: impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let mut collected = Vec::with_capacity(limit.min(32 * 1024));
    let mut buffer = [0_u8; 32 * 1024];
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        let remaining = limit.saturating_sub(collected.len());
        collected.extend_from_slice(buffer.get(..read.min(remaining)).unwrap_or(&buffer));
    }
    Ok(collected)
}

pub(crate) fn bounded_command_error(context: &str, output: &BoundedOutput) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let details = if stderr.is_empty() { stdout } else { stderr };
    if details.is_empty() {
        format!("{context} (exit status {})", output.status)
    } else {
        format!("{context}: {details}")
    }
}
