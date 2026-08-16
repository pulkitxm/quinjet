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
use std::process::Output;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result, ensure};

static SCRATCH_ID: AtomicUsize = AtomicUsize::new(0);

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
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .args(args)
            .env("LC_ALL", "C")
            .output()
            .context("failed to run git")?;
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
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_quinjet"));
    if let Some(directory) = directory {
        command.arg("-C").arg(directory);
    }
    let output = command
        .args(args)
        .env("LC_ALL", "C")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .context("failed to run the quinjet binary")?;
    Run::from(output)
}

#[test]
fn version_names_the_binary() -> Result<()> {
    let run = run_in(None, &["--version"])?.success()?;
    ensure!(
        run.stdout.starts_with("quinjet "),
        "unexpected version line: {}",
        run.stdout
    );
    Ok(())
}

#[test]
fn help_lists_every_group_verb() -> Result<()> {
    let run = run_in(None, &["--help"])?.success()?;
    for verb in [
        "tui",
        "status",
        "diff",
        "stage",
        "unstage",
        "discard",
        "commit",
        "fetch",
        "pull",
        "push",
        "sync",
        "log",
        "show",
        "branch",
        "stash",
        "cherry-pick",
        "revert",
        "resolve",
        "repos",
        "pr",
        "completions",
        "man",
    ] {
        ensure!(run.stdout.contains(verb), "--help does not mention {verb}");
    }
    Ok(())
}

#[test]
fn every_subcommand_answers_help() -> Result<()> {
    for path in [
        vec!["status"],
        vec!["diff"],
        vec!["branch", "compare"],
        vec!["stash", "show"],
        vec!["pr", "logs"],
        vec!["completions"],
        vec!["man"],
    ] {
        let mut args = path.clone();
        args.push("--help");
        let run = run_in(None, &args)?;
        ensure!(run.code == 0, "{path:?} --help exited {}", run.code);
    }
    Ok(())
}

#[test]
fn unknown_flags_are_usage_errors() -> Result<()> {
    let run = run_in(None, &["status", "--no-such-flag"])?;
    ensure!(run.code == 2, "expected exit 2, got {}", run.code);
    ensure!(
        run.stderr.contains("--no-such-flag"),
        "usage error does not name the flag"
    );
    Ok(())
}

#[test]
fn a_missing_repository_is_a_plain_failure() -> Result<()> {
    let scratch = Scratch::directory()?;
    let run = scratch.quinjet(&["status"])?;
    ensure!(run.code == 1, "expected exit 1, got {}", run.code);
    ensure!(
        run.stderr.contains("error:"),
        "failure did not report on stderr: {}",
        run.stderr
    );
    Ok(())
}

#[test]
fn status_reports_the_branch_in_both_faces() -> Result<()> {
    let scratch = Scratch::repository()?;
    let text = scratch.quinjet(&["status"])?.success()?;
    ensure!(
        text.stdout.contains("main"),
        "status does not name the branch: {}",
        text.stdout
    );
    let json = scratch.quinjet(&["status", "--json"])?.success()?;
    let document = json.json()?;
    ensure!(
        document["branch"]["head"] == "main",
        "unexpected JSON branch: {document}"
    );
    Ok(())
}

#[test]
fn stage_commit_log_show_round_trip() -> Result<()> {
    let scratch = Scratch::repository()?;
    scratch.write("feature.txt", "feature\n")?;
    drop(scratch.quinjet(&["stage", "feature.txt"])?.success()?);
    drop(
        scratch
            .quinjet(&["commit", "--message", "add the feature"])?
            .success()?,
    );
    let log = scratch.quinjet(&["log", "-n", "1"])?.success()?;
    ensure!(
        log.stdout.contains("add the feature"),
        "log misses the commit: {}",
        log.stdout
    );
    let show = scratch.quinjet(&["show"])?.success()?;
    ensure!(
        show.stdout.contains("feature.txt"),
        "show misses the file: {}",
        show.stdout
    );
    Ok(())
}

#[test]
fn diff_shows_changes_and_stage_all_clears_them() -> Result<()> {
    let scratch = Scratch::repository()?;
    scratch.write("README.md", "one\ntwo\n")?;
    let diff = scratch.quinjet(&["diff"])?.success()?;
    ensure!(
        diff.stdout.contains("two"),
        "diff misses the new line: {}",
        diff.stdout
    );
    drop(scratch.quinjet(&["stage", "--all"])?.success()?);
    let staged = scratch.quinjet(&["diff", "--staged"])?.success()?;
    ensure!(
        staged.stdout.contains("two"),
        "staged diff misses the new line: {}",
        staged.stdout
    );
    Ok(())
}

#[test]
fn discard_previews_without_yes_and_acts_with_it() -> Result<()> {
    let scratch = Scratch::repository()?;
    scratch.write("README.md", "one\nchanged\n")?;
    drop(scratch.quinjet(&["discard", "README.md"])?);
    let preserved = fs::read_to_string(scratch.path.join("README.md"))?;
    ensure!(
        preserved.contains("changed"),
        "a preview discarded the change"
    );
    drop(
        scratch
            .quinjet(&["discard", "README.md", "--yes"])?
            .success()?,
    );
    let restored = fs::read_to_string(scratch.path.join("README.md"))?;
    ensure!(restored == "one\n", "discard left: {restored}");
    Ok(())
}

#[test]
fn branch_lifecycle_round_trips() -> Result<()> {
    let scratch = Scratch::repository()?;
    drop(
        scratch
            .quinjet(&["branch", "create", "feature"])?
            .success()?,
    );
    drop(
        scratch
            .quinjet(&["branch", "rename", "feature", "renamed"])?
            .success()?,
    );
    drop(scratch.quinjet(&["branch", "switch", "main"])?.success()?);
    let listed = scratch.quinjet(&["branch", "list"])?.success()?;
    ensure!(
        listed.stdout.contains("renamed"),
        "branch list misses the branch: {}",
        listed.stdout
    );
    drop(
        scratch
            .quinjet(&["branch", "delete", "renamed", "--yes"])?
            .success()?,
    );
    let after = scratch.quinjet(&["branch", "list"])?.success()?;
    ensure!(
        !after.stdout.contains("renamed"),
        "branch delete left the branch: {}",
        after.stdout
    );
    Ok(())
}

#[test]
fn stash_push_and_pop_round_trips() -> Result<()> {
    let scratch = Scratch::repository()?;
    scratch.write("README.md", "one\nstashed\n")?;
    drop(
        scratch
            .quinjet(&["stash", "push", "--message", "held"])?
            .success()?,
    );
    let clean = fs::read_to_string(scratch.path.join("README.md"))?;
    ensure!(clean == "one\n", "stash push left: {clean}");
    let listed = scratch.quinjet(&["stash", "list"])?.success()?;
    ensure!(
        listed.stdout.contains("held"),
        "stash list misses the entry: {}",
        listed.stdout
    );
    drop(scratch.quinjet(&["stash", "pop"])?.success()?);
    let restored = fs::read_to_string(scratch.path.join("README.md"))?;
    ensure!(
        restored.contains("stashed"),
        "stash pop did not restore: {restored}"
    );
    Ok(())
}

#[test]
fn json_output_is_one_document_per_invocation() -> Result<()> {
    let scratch = Scratch::repository()?;
    for args in [
        vec!["status", "--json"],
        vec!["log", "-n", "2", "--json"],
        vec!["branch", "list", "--json"],
        vec!["stash", "list", "--json"],
        vec!["diff", "--json"],
    ] {
        let run = scratch.quinjet(&args)?.success()?;
        drop(run.json().with_context(|| format!("for {args:?}"))?);
    }
    Ok(())
}

#[test]
fn completions_cover_every_supported_shell() -> Result<()> {
    for shell in ["bash", "zsh", "fish", "elvish", "powershell"] {
        let run = run_in(None, &["completions", shell])?.success()?;
        ensure!(
            run.stdout.contains("quinjet"),
            "{shell} completions never mention the binary"
        );
    }
    let json = run_in(None, &["completions", "bash", "--json"])?.success()?;
    let document = json.json()?;
    ensure!(
        document["shell"] == "bash",
        "unexpected completions JSON: {document}"
    );
    Ok(())
}

#[test]
fn man_prints_one_page_and_writes_one_per_command() -> Result<()> {
    let page = run_in(None, &["man"])?.success()?;
    ensure!(
        page.stdout.contains(".TH QUINJET"),
        "man page misses its title header"
    );
    let scratch = Scratch::directory()?;
    let target = scratch.path.join("man");
    let target_argument = target.display().to_string();
    drop(run_in(None, &["man", "--dir", &target_argument])?.success()?);
    ensure!(
        target.join("quinjet.1").is_file(),
        "the top page was not written"
    );
    ensure!(
        target.join("quinjet-branch-create.1").is_file(),
        "the nested page was not written"
    );
    Ok(())
}
