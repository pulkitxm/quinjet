use std::borrow::Cow;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, ensure};

use crate::cli::Emitter;

pub(super) fn run(out: &Emitter) -> Result<u8> {
    out.finish_progress();
    out.note(
        "warning: Homebrew owns this installation; running `brew upgrade quinjet` for you. You can run that command directly next time.",
    );
    let mut command = upgrade_command();
    if out.json {
        let output = command
            .stdin(Stdio::null())
            .output()
            .context("failed to start `brew upgrade quinjet`")?;
        ensure!(
            output.status.success(),
            "`brew upgrade quinjet` failed: {}",
            failure(&output.stdout, &output.stderr)
        );
        out.message("Homebrew upgraded Quinjet")?;
    } else {
        let status = command
            .status()
            .context("failed to start `brew upgrade quinjet`")?;
        ensure!(status.success(), "`brew upgrade quinjet` failed");
    }
    Ok(0)
}

#[expect(
    unused_results,
    reason = "building a process command mutates and returns the command"
)]
fn upgrade_command() -> Command {
    let mut command = Command::new("brew");
    command.args(["upgrade", "quinjet"]);
    command
}

fn failure<'a>(stdout: &'a [u8], stderr: &'a [u8]) -> Cow<'a, str> {
    let stderr = String::from_utf8_lossy(stderr);
    if stderr.trim().is_empty() {
        String::from_utf8_lossy(stdout)
    } else {
        stderr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrade_uses_the_formula_command() {
        let command = upgrade_command();
        assert_eq!(command.get_program(), "brew");
        assert!(command.get_args().eq(["upgrade", "quinjet"]));
    }

    #[test]
    fn failure_prefers_stderr() {
        assert_eq!(failure(b"stdout", b"stderr"), "stderr");
        assert_eq!(failure(b"stdout", b"\n"), "stdout");
    }
}
