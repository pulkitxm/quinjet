#![doc = " Black-box tests that run the shipped binary the way a shell would."]
#![doc = ""]
#![doc = " Everything here goes through `CARGO_BIN_EXE_quinjet` argv, a scratch"]
#![doc = " repository, and captured stdout, so argument parsing, dispatch, exit"]
#![doc = " codes, and the shape of both output faces are covered end to end."]
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

fn next_scratch_path(kind: &str) -> PathBuf {
    let id = SCRATCH_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "quinjet-blackbox-{kind}-{}-{id}",
        std::process::id()
    ))
}
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

fn isolate_quinjet(command: &mut ProcessCommand, root: &Path) {
    command
        .env("HOME", root)
        .env("USERPROFILE", root)
        .env("XDG_BIN_HOME", root.join("bin"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("QUINJET_CACHE_DIR", root.join("quinjet-cache"));
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
    environment: PathBuf,
}

impl Scratch {
    fn repository() -> Result<Self> {
        let scratch = Self::unborn_repository()?;
        scratch.write("README.md", "one\n")?;
        scratch.git(&["add", "README.md"])?;
        scratch.git(&["commit", "--message=base"])?;
        Ok(scratch)
    }

    fn unborn_repository() -> Result<Self> {
        let scratch = Self::directory()?;
        scratch.git(&["init", "--initial-branch=main"])?;
        scratch.git(&["config", "user.name", "Quinjet Test"])?;
        scratch.git(&["config", "user.email", "quinjet@example.com"])?;
        scratch.git(&["config", "commit.gpgsign", "false"])?;
        Ok(scratch)
    }

    fn directory() -> Result<Self> {
        let path = next_scratch_path("files");
        let environment = next_scratch_path("environment");
        drop(fs::remove_dir_all(&path));
        drop(fs::remove_dir_all(&environment));
        fs::create_dir_all(&path).context("failed to create the scratch directory")?;
        fs::create_dir_all(&environment).context("failed to create the test environment")?;
        Ok(Self { path, environment })
    }

    fn write(&self, name: &str, content: &str) -> Result<()> {
        let path = self.path.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create the parent of {name}"))?;
        }
        fs::write(path, content).with_context(|| format!("failed to write {name}"))
    }

    fn git_run(&self, args: &[&str]) -> Result<Run> {
        let mut command = ProcessCommand::new("git");
        command.arg("-C").arg(&self.path).args(args);
        isolate_git(&mut command);
        Run::from(command.output().context("failed to run git")?)
    }

    fn git(&self, args: &[&str]) -> Result<String> {
        let run = self.git_run(args)?.success()?;
        Ok(run.stdout.trim().to_owned())
    }

    fn quinjet_command(&self, args: &[&str]) -> ProcessCommand {
        command_in(Some(&self.path), args, &self.environment)
    }

    fn quinjet(&self, args: &[&str]) -> Result<Run> {
        let mut command = self.quinjet_command(args);
        Run::from(
            command
                .output()
                .context("failed to run the quinjet binary")?,
        )
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
        drop(fs::remove_dir_all(&self.environment));
    }
}

struct IsolatedEnvironment {
    path: PathBuf,
}

impl IsolatedEnvironment {
    fn new() -> Result<Self> {
        let path = next_scratch_path("environment");
        drop(fs::remove_dir_all(&path));
        fs::create_dir_all(&path).context("failed to create the test environment")?;
        Ok(Self { path })
    }
}

impl Drop for IsolatedEnvironment {
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
    let environment = IsolatedEnvironment::new()?;
    let mut command = command_in(directory, args, &environment.path);
    let output = command
        .output()
        .context("failed to run the quinjet binary")?;
    Run::from(output)
}

fn command_in(directory: Option<&Path>, args: &[&str], environment: &Path) -> ProcessCommand {
    let mut command = ProcessCommand::new(env!("CARGO_BIN_EXE_quinjet"));
    if let Some(directory) = directory {
        command.current_dir(directory);
        command.arg("-C").arg(directory);
    }
    command.args(args);
    isolate_git(&mut command);
    isolate_quinjet(&mut command, environment);
    command
}

#[path = "cli/capabilities.rs"]
mod capabilities;
#[cfg(unix)]
#[path = "cli/github.rs"]
mod github;
#[path = "cli/metadata.rs"]
mod metadata;
#[path = "cli/output.rs"]
mod output;
#[path = "cli/remotes.rs"]
mod remotes;
#[path = "cli/repository.rs"]
mod repository;
#[path = "cli/shell.rs"]
mod shell;
