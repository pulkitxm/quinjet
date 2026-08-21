//! Black-box tests that run the shipped binary the way a shell would.
//!
//! Everything here goes through `CARGO_BIN_EXE_quinjet` argv, a scratch
//! repository, and captured stdout, so argument parsing, dispatch, exit
//! codes, and the shape of both output faces are covered end to end.

#![expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(not(windows))]
use std::thread;
#[cfg(not(windows))]
use std::time::Duration;

use anyhow::{Context, Result, ensure};

static SCRATCH_ID: AtomicUsize = AtomicUsize::new(0);
const GIT_NULL_DEVICE: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };
const COMMAND_PATHS: &[&[&str]] = &[
    &["tui"],
    &["status"],
    &["diff"],
    &["stage"],
    &["unstage"],
    &["discard"],
    &["commit"],
    &["fetch"],
    &["pull"],
    &["push"],
    &["sync"],
    &["log"],
    &["show"],
    &["branch"],
    &["branch", "list"],
    &["branch", "switch"],
    &["branch", "create"],
    &["branch", "rename"],
    &["branch", "delete"],
    &["branch", "compare"],
    &["stash"],
    &["stash", "list"],
    &["stash", "push"],
    &["stash", "apply"],
    &["stash", "pop"],
    &["stash", "drop"],
    &["stash", "clear"],
    &["stash", "show"],
    &["worktree"],
    &["worktree", "list"],
    &["cherry-pick"],
    &["revert"],
    &["resolve"],
    &["repos"],
    &["pr"],
    &["pr", "view"],
    &["pr", "files"],
    &["pr", "diff"],
    &["pr", "conversation"],
    &["pr", "checks"],
    &["pr", "logs"],
    &["pr", "open"],
    &["pr", "merge"],
    &["pr", "admin-merge"],
    &["pr", "auto-merge"],
    &["pr", "disable-auto-merge"],
    &["pr", "dequeue"],
    &["pr", "ready"],
    &["pr", "draft"],
    &["pr", "review"],
    &["pr", "comment"],
    &["pr", "edit-last-comment"],
    &["pr", "delete-last-comment"],
    &["pr", "edit"],
    &["pr", "update-branch"],
    &["pr", "lock"],
    &["pr", "unlock"],
    &["pr", "subscribe"],
    &["pr", "unsubscribe"],
    &["pr", "allow-maintainer-edits"],
    &["pr", "disallow-maintainer-edits"],
    &["pr", "revert"],
    &["pr", "close"],
    &["pr", "reopen"],
    &["completions"],
    &["man"],
    &["capabilities"],
    &["update"],
];

fn isolate_git(command: &mut ProcessCommand) {
    for variable in [
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_CEILING_DIRECTORIES",
        "GIT_COMMON_DIR",
        "GIT_CONFIG",
        "GIT_CONFIG_COUNT",
        "GIT_DIR",
        "GIT_DISCOVERY_ACROSS_FILESYSTEM",
        "GIT_EXEC_PATH",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_TEMPLATE_DIR",
        "GIT_WORK_TREE",
    ] {
        command.env_remove(variable);
    }
    command
        .env("LC_ALL", "C")
        .env("GIT_CONFIG_GLOBAL", GIT_NULL_DEVICE)
        .env("GIT_CONFIG_NOSYSTEM", "1");
}

#[cfg(not(windows))]
fn copied_binary_output(command: &mut ProcessCommand, context: &str) -> Result<Output> {
    let mut retries = 20;
    loop {
        match command.output() {
            Ok(output) => return Ok(output),
            Err(error) if error.raw_os_error() == Some(26) && retries > 0 => {
                retries -= 1;
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error).context(context.to_owned()),
        }
    }
}

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn repository() -> Result<Self> {
        let scratch = Self::directory()?;
        scratch.git(&["init", "--initial-branch=main"])?;
        scratch.git(&["config", "user.name", "Quinjet Test"])?;
        scratch.git(&["config", "user.email", "quinjet@example.com"])?;
        scratch.git(&["config", "commit.gpgsign", "false"])?;
        scratch.write("README.md", "one\n")?;
        scratch.git(&["add", "README.md"])?;
        scratch.git(&["commit", "--message=base"])?;
        Ok(scratch)
    }

    fn directory() -> Result<Self> {
        let id = SCRATCH_ID.fetch_add(1, Ordering::Relaxed);
        let name = format!("quinjet-blackbox-{}-{id}", std::process::id());
        // nosemgrep: rust.lang.security.temp-dir.temp-dir
        let path = std::env::temp_dir().join(name);
        drop(fs::remove_dir_all(&path));
        fs::create_dir_all(&path).context("failed to create the scratch directory")?;
        Ok(Self { path })
    }

    fn write(&self, name: &str, content: &str) -> Result<()> {
        fs::write(self.path.join(name), content).with_context(|| format!("failed to write {name}"))
    }

    fn git(&self, args: &[&str]) -> Result<String> {
        let mut command = ProcessCommand::new("git");
        command.arg("-C").arg(&self.path).args(args);
        isolate_git(&mut command);
        let output = command.output().context("failed to run git")?;
        ensure!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    fn quinjet(&self, args: &[&str]) -> Result<Run> {
        run_in(Some(&self.path), args)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    fn from(output: Output) -> Result<Self> {
        Ok(Self {
            code: output
                .status
                .code()
                .context("the binary was killed by a signal")?,
            stdout: String::from_utf8(output.stdout).context("stdout was not UTF-8")?,
            stderr: String::from_utf8(output.stderr).context("stderr was not UTF-8")?,
        })
    }

    fn success(self) -> Result<Self> {
        ensure!(
            self.code == 0,
            "expected success, got exit {}: {}",
            self.code,
            self.stderr
        );
        Ok(self)
    }

    fn json(&self) -> Result<serde_json::Value> {
        serde_json::from_str(&self.stdout)
            .with_context(|| format!("stdout was not one JSON document: {}", self.stdout))
    }
}

fn run_in(directory: Option<&Path>, args: &[&str]) -> Result<Run> {
    let mut command = ProcessCommand::new(env!("CARGO_BIN_EXE_quinjet"));
    if let Some(directory) = directory {
        command.current_dir(directory);
        command.arg("-C").arg(directory);
    }
    command.args(args);
    isolate_git(&mut command);
    let output = command
        .output()
        .context("failed to run the quinjet binary")?;
    Run::from(output)
}

#[path = "cli/capabilities.rs"]
mod capabilities;
#[path = "cli/metadata.rs"]
mod metadata;
#[path = "cli/output.rs"]
mod output;
#[path = "cli/repository.rs"]
mod repository;
#[path = "cli/shell.rs"]
mod shell;
