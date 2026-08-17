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
        "capabilities",
        "update",
    ] {
        ensure!(run.stdout.contains(verb), "--help does not mention {verb}");
    }
    Ok(())
}

#[test]
fn every_subcommand_answers_help() -> Result<()> {
    for path in COMMAND_PATHS {
        let mut args = path.to_vec();
        args.push("--help");
        let run = run_in(None, &args)?;
        ensure!(run.code == 0, "{path:?} --help exited {}", run.code);
    }
    Ok(())
}

#[test]
fn root_help_leads_with_examples_and_documentation() -> Result<()> {
    let run = run_in(None, &["--help"])?.success()?;
    ensure!(run.stdout.contains("Examples:"));
    ensure!(run.stdout.contains("quinjet status --json"));
    ensure!(run.stdout.contains("Documentation:"));
    Ok(())
}

#[test]
fn unknown_verbs_and_implicit_paths_are_usage_errors() -> Result<()> {
    for value in ["statsu", "/tmp/somewhere"] {
        let run = run_in(None, &[value])?;
        ensure!(run.code == 2, "{value} exited {}", run.code);
        ensure!(
            run.stderr.contains("unrecognized subcommand"),
            "unexpected error for {value}: {}",
            run.stderr
        );
    }
    Ok(())
}

#[test]
fn clap_rejects_incomplete_and_inert_arguments() -> Result<()> {
    for args in [
        &["stage"][..],
        &["unstage"][..],
        &["discard"][..],
        &["resolve", "README.md"][..],
        &["status", "--interval", "0"][..],
        &["pr", "checks", "1", "--watch", "--exit-code"][..],
    ] {
        let run = run_in(None, args)?;
        ensure!(run.code == 2, "{args:?} exited {}", run.code);
        ensure!(!run.stderr.is_empty(), "{args:?} produced no usage error");
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
fn captured_and_json_output_never_include_progress() -> Result<()> {
    let scratch = Scratch::repository()?;
    for args in [vec!["status"], vec!["status", "--json"]] {
        let run = scratch.quinjet(&args)?.success()?;
        ensure!(
            run.stderr.is_empty(),
            "{args:?} wrote progress to captured stderr: {}",
            run.stderr
        );
    }
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
    let preview = scratch.quinjet(&["discard", "README.md"])?.success()?;
    ensure!(
        preview.stderr.is_empty(),
        "preview wrote: {}",
        preview.stderr
    );
    ensure!(
        preview.stdout.contains("Pass --yes"),
        "preview did not explain confirmation: {}",
        preview.stdout
    );
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
fn revision_mutations_preview_until_confirmed() -> Result<()> {
    let scratch = Scratch::repository()?;
    scratch.git(&["switch", "--create", "feature"])?;
    scratch.write("feature.txt", "feature\n")?;
    scratch.git(&["add", "feature.txt"])?;
    scratch.git(&["commit", "--message=feature"])?;
    let feature = scratch.git(&["rev-parse", "HEAD"])?;
    scratch.git(&["switch", "main"])?;

    let before = scratch.git(&["rev-parse", "HEAD"])?;
    let preview = scratch.quinjet(&["cherry-pick", &feature])?.success()?;
    ensure!(preview.stdout.contains("Pass --yes"));
    ensure!(scratch.git(&["rev-parse", "HEAD"])? == before);

    drop(
        scratch
            .quinjet(&["cherry-pick", &feature, "--yes"])?
            .success()?,
    );
    let applied = scratch.git(&["rev-parse", "HEAD"])?;
    ensure!(applied != before);

    let preview = scratch.quinjet(&["revert", &applied])?.success()?;
    ensure!(preview.stdout.contains("Pass --yes"));
    ensure!(scratch.git(&["rev-parse", "HEAD"])? == applied);

    drop(scratch.quinjet(&["revert", &applied, "--yes"])?.success()?);
    ensure!(scratch.git(&["rev-parse", "HEAD"])? != applied);
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
    let scratch = Scratch::directory()?;
    for shell in ["bash", "zsh", "fish", "elvish", "powershell"] {
        let run = scratch.quinjet(&["completions", shell])?.success()?;
        ensure!(
            run.stdout.contains("quinjet"),
            "{shell} completions never mention the binary"
        );
    }
    let json = scratch
        .quinjet(&["completions", "bash", "--json"])?
        .success()?;
    let document = json.json()?;
    ensure!(
        document["shell"] == "bash",
        "unexpected completions JSON: {document}"
    );
    let alias = scratch.quinjet(&["completion", "bash"])?.success()?;
    ensure!(alias.stdout.contains("quinjet"));
    Ok(())
}

#[cfg(not(windows))]
#[test]
fn shell_integration_makes_q_immediate_without_restoring_removals() -> Result<()> {
    let scratch = Scratch::directory()?;
    let bin = scratch.path.join("bin");
    let data = scratch.path.join("data");
    let state = scratch.path.join(".local/state/quinjet");
    let executable = bin.join("quinjet");
    let shortcut = bin.join("q");
    let completion = data.join("bash-completion/completions/quinjet");
    let bashrc = scratch.path.join(".bashrc");
    fs::create_dir_all(&bin)?;
    fs::create_dir_all(&state)?;
    fs::create_dir_all(completion.parent().context("completion had no parent")?)?;
    let staged = bin.join("quinjet-stage");
    fs::copy(env!("CARGO_BIN_EXE_quinjet"), &staged)?;
    fs::rename(staged, &executable)?;
    fs::write(state.join("bash-installed"), "installed\n")?;
    fs::write(&completion, "stale completion\n")?;
    fs::write(
        &bashrc,
        "# >>> quinjet shortcut >>>\nalias q='quinjet'\n# <<< quinjet shortcut <<<\n",
    )?;

    let maintain = || -> Result<Run> {
        let mut update = ProcessCommand::new(&executable);
        update
            .args(["completions", "bash", "--install", "--automatic"])
            .env("HOME", &scratch.path)
            .env("XDG_DATA_HOME", &data)
            .env("PATH", &bin)
            .env("SHELL", "/bin/bash")
            .env("PSModulePath", "inherited-but-not-active");
        isolate_git(&mut update);
        Run::from(update.output().context("failed to refresh completions")?)?.success()
    };

    fs::write(&shortcut, "unrelated q command\n")?;
    drop(maintain()?);
    ensure!(fs::read_to_string(&shortcut)? == "unrelated q command\n");
    ensure!(fs::read_to_string(&bashrc)?.contains("quinjet shortcut"));
    fs::remove_file(&shortcut)?;
    fs::remove_file(state.join("shortcut-installed"))?;

    drop(maintain()?);
    ensure!(fs::read_to_string(&completion)?.contains("complete -F _quinjet"));
    ensure!(shortcut.exists());
    ensure!(!fs::read_to_string(&bashrc)?.contains("quinjet shortcut"));
    ensure!(state.join("shortcut-installed").is_file());

    let mut path = vec![bin.clone()];
    if let Some(existing) = std::env::var_os("PATH") {
        path.extend(std::env::split_paths(&existing));
    }
    let mut invoke_q = ProcessCommand::new("q");
    invoke_q
        .arg("--version")
        .env("PATH", std::env::join_paths(path)?);
    let q = Run::from(
        invoke_q
            .output()
            .context("q was unavailable in the current shell")?,
    )?
    .success()?;
    ensure!(q.stdout.contains("quinjet"));

    fs::remove_file(&shortcut)?;
    drop(maintain()?);
    ensure!(!shortcut.exists());

    fs::remove_file(&completion)?;
    drop(maintain()?);
    ensure!(!completion.exists());

    let mut restore = ProcessCommand::new(&executable);
    restore
        .args(["completions", "bash", "--install"])
        .env("HOME", &scratch.path)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &bin)
        .env("SHELL", "/bin/bash");
    isolate_git(&mut restore);
    drop(Run::from(restore.output().context("failed to restore completions")?)?.success()?);
    ensure!(completion.exists());
    ensure!(shortcut.exists());
    Ok(())
}

#[cfg(windows)]
#[test]
fn shell_integration_makes_q_immediate_on_windows() -> Result<()> {
    let scratch = Scratch::directory()?;
    let bin = scratch.path.join("bin");
    let data = scratch.path.join("data");
    let executable = bin.join("quinjet.exe");
    let shortcut = bin.join("q.cmd");
    fs::create_dir_all(&bin)?;
    let staged = bin.join("quinjet-stage.exe");
    fs::copy(env!("CARGO_BIN_EXE_quinjet"), &staged)?;
    fs::rename(staged, &executable)?;

    let mut install = ProcessCommand::new(&executable);
    install
        .args(["completions", "bash", "--install"])
        .env("HOME", &scratch.path)
        .env("USERPROFILE", &scratch.path)
        .env("LOCALAPPDATA", scratch.path.join("local"))
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &bin);
    isolate_git(&mut install);
    drop(
        Run::from(
            install
                .output()
                .context("failed to install shell integration")?,
        )?
        .success()?,
    );
    ensure!(shortcut.exists());

    let mut path = vec![bin];
    if let Some(existing) = std::env::var_os("PATH") {
        path.extend(std::env::split_paths(&existing));
    }
    let mut invoke_q = ProcessCommand::new("cmd.exe");
    invoke_q
        .args(["/D", "/C", "q --version"])
        .env("PATH", std::env::join_paths(path)?);
    let q = Run::from(
        invoke_q
            .output()
            .context("q was unavailable in the current shell")?,
    )?
    .success()?;
    ensure!(q.stdout.contains("quinjet"));
    Ok(())
}

#[test]
fn capabilities_describe_the_installed_command_tree() -> Result<()> {
    let run = run_in(None, &["capabilities", "--json"])?.success()?;
    let document = run.json()?;
    ensure!(document["schemaVersion"] == 1);
    ensure!(document["version"] == env!("CARGO_PKG_VERSION"));
    let commands = document["commands"]
        .as_array()
        .context("capabilities commands were not an array")?;
    for path in [
        "quinjet status",
        "quinjet branch create",
        "quinjet pr checks",
        "quinjet capabilities",
    ] {
        ensure!(
            commands.iter().any(|command| command["path"] == path),
            "capabilities omitted {path}"
        );
    }
    let completion = commands
        .iter()
        .find(|command| command["path"] == "quinjet completions")
        .context("capabilities omitted completions")?;
    ensure!(
        completion["arguments"][0]["possibleValues"]
            == serde_json::json!(["bash", "elvish", "fish", "powershell", "zsh"])
    );
    let stage = commands
        .iter()
        .find(|command| command["path"] == "quinjet stage")
        .context("capabilities omitted stage")?;
    ensure!(
        stage["usage"]
            .as_str()
            .is_some_and(|usage| usage.contains("<PATH|--all>"))
    );
    let all = stage["arguments"]
        .as_array()
        .and_then(|arguments| arguments.iter().find(|argument| argument["id"] == "all"))
        .context("stage capabilities omitted --all")?;
    ensure!(all["action"] == "set_true");
    ensure!(all["minValues"] == 0 && all["maxValues"] == 0);
    ensure!(all["possibleValues"] == serde_json::json!([]));
    let selection = stage["groups"]
        .as_array()
        .and_then(|groups| {
            groups
                .iter()
                .find(|group| group["arguments"] == serde_json::json!(["paths", "all"]))
        })
        .context("stage capabilities omitted its required selection group")?;
    ensure!(selection["required"] == true);
    ensure!(selection["multiple"] == false);

    let status = commands
        .iter()
        .find(|command| command["path"] == "quinjet status")
        .context("capabilities omitted status")?;
    let interval = status["arguments"]
        .as_array()
        .and_then(|arguments| {
            arguments
                .iter()
                .find(|argument| argument["id"] == "interval")
        })
        .context("status capabilities omitted --interval")?;
    ensure!(interval["action"] == "set");
    ensure!(interval["defaultValues"] == serde_json::json!(["2"]));
    Ok(())
}

#[cfg(not(windows))]
#[test]
fn bash_accepts_the_generated_completion_script() -> Result<()> {
    let scratch = Scratch::directory()?;
    let run = scratch.quinjet(&["completions", "bash"])?.success()?;
    scratch.write("quinjet.bash", &run.stdout)?;
    let mut command = ProcessCommand::new("bash");
    command.arg("-n").arg(scratch.path.join("quinjet.bash"));
    isolate_git(&mut command);
    let output = command
        .output()
        .context("failed to validate bash completions")?;
    ensure!(
        output.status.success(),
        "bash rejected completions: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn man_prints_one_page_and_writes_one_per_command() -> Result<()> {
    let scratch = Scratch::directory()?;
    let page = scratch.quinjet(&["man"])?.success()?;
    ensure!(
        page.stdout.contains(".TH QUINJET"),
        "man page misses its title header"
    );
    let target = scratch.path.join("man");
    let target_argument = target.display().to_string();
    drop(
        scratch
            .quinjet(&["man", "--dir", &target_argument])?
            .success()?,
    );
    ensure!(
        target.join("quinjet.1").is_file(),
        "the top page was not written"
    );
    ensure!(
        target.join("quinjet-branch-create.1").is_file(),
        "the nested page was not written"
    );
    let nested = fs::read_to_string(target.join("quinjet-branch-create.1"))?;
    ensure!(
        nested.contains("quinjet branch create"),
        "nested synopsis lost its command path: {nested}"
    );
    ensure!(
        nested.contains("\\-\\-json") && nested.contains("\\-C"),
        "nested page lost global options: {nested}"
    );
    Ok(())
}
