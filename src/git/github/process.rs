#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(crate) struct BoundedOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
}

pub(crate) fn run_bounded_command(
    command: &mut Command,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<BoundedOutput> {
    let _ = command.stdin(Stdio::null());
    run_bounded_command_inner(command, None, stdout_limit, stderr_limit)
}

pub(super) fn run_bounded_command_with_input(
    command: &mut Command,
    input: &[u8],
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<BoundedOutput> {
    let _ = command.stdin(Stdio::piped());
    run_bounded_command_inner(command, Some(input), stdout_limit, stderr_limit)
}

#[expect(
    clippy::large_stack_arrays,
    reason = "the read buffer is deliberately one page of stack"
)]
fn run_bounded_command_inner(
    command: &mut Command,
    input: Option<&[u8]>,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<BoundedOutput> {
    let _ = command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;
    if let Some(input) = input {
        let mut stdin = child
            .stdin
            .take()
            .context("child process did not expose stdin")?;
        if let Err(error) = stdin.write_all(input) {
            drop(child.kill());
            drop(child.wait());
            return Err(error.into());
        }
    }
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn stdout_limits_handle_zero_exact_and_oversized_output() {
        let cases: [(usize, &[u8], bool); 3] =
            [(0, b"", true), (4, b"data", false), (3, b"dat", true)];

        for (limit, expected, truncated) in cases {
            let mut command = shell("printf data");
            let output = run_bounded_command(&mut command, limit, 32).unwrap();
            assert_eq!(output.stdout, expected, "limit {limit}");
            assert_eq!(output.stdout_truncated, truncated, "limit {limit}");
        }
    }

    #[test]
    fn stdin_is_forwarded_without_text_conversion() {
        let input = b"first\n\0\xfflast";
        let mut command = shell("cat");

        let output = run_bounded_command_with_input(&mut command, input, input.len(), 32).unwrap();

        assert!(output.status.success());
        assert_eq!(output.stdout, input);
        assert!(!output.stdout_truncated);
        assert_eq!(output.stderr, b"");
    }

    #[test]
    fn stdout_and_stderr_are_drained_simultaneously() {
        const CHUNK: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        const REPEATS: usize = 2_048;
        let expected_output = CHUNK.repeat(REPEATS).into_bytes();
        let expected = expected_output.len();
        let script = format!(
            "chunk={CHUNK}; i=0; while [ \"$i\" -lt {REPEATS} ]; do printf '%s' \"$chunk\"; printf '%s' \"$chunk\" >&2; i=$((i + 1)); done"
        );
        let mut command = shell(&script);

        let output = run_bounded_command(&mut command, expected, expected).unwrap();

        assert!(output.status.success());
        assert!(!output.stdout_truncated);
        assert_eq!(output.stdout.len(), expected);
        assert_eq!(output.stderr.len(), expected);
        assert_eq!(output.stdout, expected_output);
        assert_eq!(output.stderr, expected_output);
    }

    #[test]
    fn nonzero_status_keeps_bounded_stdout_and_stderr() {
        let mut command = shell("printf output; printf error >&2; exit 7");

        let output = run_bounded_command(&mut command, 6, 5).unwrap();

        assert_eq!(output.status.code(), Some(7));
        assert_eq!(output.stdout, b"output");
        assert_eq!(output.stderr, b"error");
        assert!(!output.stdout_truncated);
    }

    #[test]
    fn command_errors_prefer_stderr_then_stdout_then_status() {
        let mut command = shell("printf 'stdout detail'; printf 'stderr detail' >&2; exit 7");
        let both = run_bounded_command(&mut command, 64, 64).unwrap();
        assert_eq!(
            bounded_command_error("failed", &both),
            "failed: stderr detail"
        );

        let mut command = shell("printf 'stdout detail'; printf '  \\n' >&2; exit 8");
        let stdout = run_bounded_command(&mut command, 64, 64).unwrap();
        assert_eq!(
            bounded_command_error("failed", &stdout),
            "failed: stdout detail"
        );

        let mut command = shell("exit 9");
        let status = run_bounded_command(&mut command, 64, 64).unwrap();
        assert_eq!(
            bounded_command_error("failed", &status),
            format!("failed (exit status {})", status.status)
        );
    }

    fn shell(script: &str) -> Command {
        let mut command = Command::new("sh");
        let _ = command.arg("-c").arg(script);
        command
    }
}
