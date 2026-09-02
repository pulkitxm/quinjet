#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " Run one verification command inside a session's worktree and record"]
#[doc = " what it did. The command is spawned with its arguments as argv, never"]
#[doc = " through a shell, so nothing in a pull request can turn a recorded"]
#[doc = " command into a different one. It runs against the session's own"]
#[doc = " checkout, not the repository the reviewer is sitting in, which is why"]
#[doc = " this is not a method on one."]
pub(crate) fn run_work_verification(
    session: &WorkSession,
    argv: &[String],
    ran_at: String,
) -> Result<WorkVerification> {
    let directory = session_worktree(session)?;
    let (program, arguments) = argv
        .split_first()
        .context("a verification needs a command to run")?;
    if program.trim().is_empty() {
        bail!("a verification needs a command to run");
    }
    let mut command = Command::new(program);
    let _ = command
        .current_dir(directory)
        .args(arguments)
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0");
    let output = run_bounded_command(&mut command, MAX_RECORDED_OUTPUT, MAX_COMMAND_STDERR)
        .with_context(|| format!("failed to run `{program}` in the session worktree"))?;
    let exit_code = output.status.code().unwrap_or(-1);
    Ok(WorkVerification {
        command: argv.to_vec(),
        exit_code,
        passed: output.status.success(),
        ran_at,
        output: recorded_output(&output.stdout, &output.stderr),
    })
}

#[doc = " What the session has changed since the commit it started from,"]
#[doc = " including files it has not staged. A session's work is whatever is"]
#[doc = " in its worktree, not whatever it happens to have committed."]
pub(crate) fn work_diff(session: &WorkSession) -> Result<WorkDiff> {
    let directory = session_worktree(session)?;
    let files = worktree_git(
        directory,
        &[
            OsString::from("diff"),
            OsString::from("--name-only"),
            OsString::from(&session.start_oid),
        ],
        MAX_SESSION_PATCH_BYTES,
    )?;
    let (patch, truncated) = worktree_git_bounded(
        directory,
        &[
            OsString::from("diff"),
            OsString::from("--no-color"),
            OsString::from("--no-ext-diff"),
            OsString::from("--find-renames"),
            OsString::from("--patch"),
            OsString::from("--unified=3"),
            OsString::from(&session.start_oid),
        ],
        MAX_SESSION_PATCH_BYTES,
    )?;
    Ok(WorkDiff {
        id: session.id.clone(),
        start_oid: session.start_oid.clone(),
        files: String::from_utf8_lossy(&files)
            .lines()
            .map(str::to_owned)
            .filter(|line| !line.is_empty())
            .collect(),
        patch: String::from_utf8_lossy(&patch).into_owned(),
        truncated,
    })
}

pub(super) fn session_worktree(session: &WorkSession) -> Result<&Path> {
    let worktree = session
        .worktree
        .as_deref()
        .ok_or_else(|| anyhow!("session {} has no worktree", session.id))?;
    if !worktree.is_dir() {
        bail!(
            "session {}'s worktree is missing from {}",
            session.id,
            worktree.display()
        );
    }
    Ok(worktree)
}

pub(super) fn worktree_git(directory: &Path, args: &[OsString], limit: usize) -> Result<Vec<u8>> {
    let (output, _) = worktree_git_bounded(directory, args, limit)?;
    Ok(output)
}

fn worktree_git_bounded(
    directory: &Path,
    args: &[OsString],
    limit: usize,
) -> Result<(Vec<u8>, bool)> {
    let mut command = Command::new("git");
    let _ = command
        .arg("-C")
        .arg(directory)
        .args(["-c", "core.quotepath=false"])
        .args(args)
        .env("LC_ALL", "C")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0");
    let output = run_bounded_command(&mut command, limit, MAX_COMMAND_STDERR)
        .with_context(|| format!("failed to execute Git in {}", directory.display()))?;
    if !output.status.success() && !output.stdout_truncated {
        bail!(
            "Git failed in the session worktree: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok((output.stdout, output.stdout_truncated))
}

#[doc = " The tail of what a command wrote. Standard error comes last because"]
#[doc = " that is where a failing command usually says why."]
fn recorded_output(stdout: &[u8], stderr: &[u8]) -> String {
    let mut text = String::new();
    for stream in [stdout, stderr] {
        let chunk = String::from_utf8_lossy(stream);
        let chunk = chunk.trim_end();
        if chunk.is_empty() {
            continue;
        }
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(chunk);
    }
    text
}
