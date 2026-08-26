use std::os::unix::fs::PermissionsExt;

use super::*;

const GH_SCRIPT: &str = r#"#!/bin/sh
{
  printf 'argv'
  for argument in "$@"; do
    printf '\t%s' "$argument"
  done
  printf '\n'
  printf 'env\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$GH_PROMPT_DISABLED" "$GH_PAGER" "$GH_NO_UPDATE_NOTIFIER" "$NO_COLOR" "$GIT_TERMINAL_PROMPT" "$GIT_EDITOR" "$GIT_SEQUENCE_EDITOR" "$EDITOR" "$VISUAL"
  printf 'force-tty\t%s\n' "$GH_FORCE_TTY"
} >> "$FAKE_GH_CAPTURE"
if [ "$FAKE_GH_LARGE" = 1 ]; then
  i=0
  while [ "$i" -lt 5000 ]; do
    printf '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef' >&2
    i=$((i + 1))
  done
  exit 0
fi
case "$*" in
  "stack submit --auto --open --remote upstream") printf 'Submitted pull requests 41 and 42\n' >&2 ;;
esac
"#;

struct StackFixture {
    repository: Scratch,
    bin: PathBuf,
    capture: PathBuf,
}

impl StackFixture {
    fn new() -> Result<Self> {
        let repository = Scratch::repository()?;
        let bin = repository.environment.join("fake-bin");
        fs::create_dir_all(&bin)?;
        let executable = bin.join("gh");
        fs::write(&executable, GH_SCRIPT)?;
        let mut permissions = fs::metadata(&executable)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions)?;
        let capture = repository.environment.join("gh-capture");
        Ok(Self {
            repository,
            bin,
            capture,
        })
    }

    fn run(&self, args: &[&str]) -> Result<Run> {
        let mut command = self.command(args)?;
        Run::from(
            command
                .output()
                .context("failed to run stack lifecycle command")?,
        )
    }

    fn run_with_large_stderr(&self, args: &[&str]) -> Result<Run> {
        let mut command = self.command(args)?;
        let output = command
            .env("FAKE_GH_LARGE", "1")
            .output()
            .context("failed to run stack lifecycle command")?;
        Run::from(output)
    }

    fn command(&self, args: &[&str]) -> Result<ProcessCommand> {
        let mut paths = vec![self.bin.clone()];
        if let Some(current) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&current));
        }
        let mut command = self.repository.quinjet_command(args);
        command
            .env("PATH", std::env::join_paths(paths)?)
            .env("FAKE_GH_CAPTURE", &self.capture)
            .env("GH_FORCE_TTY", "1");
        Ok(command)
    }

    fn calls(&self) -> Result<String> {
        fs::read_to_string(&self.capture).context("fake gh did not record a call")
    }
}

#[test]
fn stack_lifecycle_previews_before_using_noninteractive_transport() -> Result<()> {
    let fixture = StackFixture::new()?;
    let preview = fixture
        .run(&["stack", "submit", "--open", "--git-remote", "upstream"])?
        .success()?;
    ensure!(preview.stdout == "Would submit the active stack. Pass --yes to continue.\n");
    ensure!(!fixture.capture.exists());

    let confirmed = fixture
        .run(&[
            "stack",
            "submit",
            "--open",
            "--git-remote",
            "upstream",
            "--yes",
        ])?
        .success()?;
    ensure!(confirmed.stdout == "Submitted pull requests 41 and 42\n");
    let calls = fixture.calls()?;
    ensure!(calls.contains("argv\tstack\tsubmit\t--auto\t--open\t--remote\tupstream\n"));
    ensure!(calls.contains("env\t1\tcat\t1\t1\t0\ttrue\ttrue\ttrue\ttrue\n"));
    ensure!(calls.contains("force-tty\t\n"));
    Ok(())
}

#[test]
fn stack_lifecycle_delegates_every_supported_command() -> Result<()> {
    let fixture = StackFixture::new()?;
    let cases: &[&[&str]] = &[
        &["stack", "init", "api", "ui", "--base", "main", "--yes"],
        &[
            "stack",
            "add",
            "tests",
            "--message",
            "Add tests",
            "--all",
            "--yes",
        ],
        &["stack", "checkout", "42", "--yes"],
        &["stack", "modify", "--abort", "--yes"],
        &["stack", "unstack", "7", "--local", "--yes"],
        &["stack", "link", "41", "42", "--open", "--yes"],
        &["stack", "merge", "42", "--squash", "--yes"],
        &["stack", "push", "--git-remote", "origin", "--yes"],
        &[
            "stack",
            "rebase",
            "api",
            "--downstack",
            "--no-trunk",
            "--yes",
        ],
        &["stack", "submit", "--open", "--yes"],
        &["stack", "sync", "--prune", "--yes"],
        &["stack", "bottom", "--yes"],
        &["stack", "down", "2", "--yes"],
        &["stack", "top", "--yes"],
        &["stack", "trunk", "--yes"],
        &["stack", "up", "3", "--yes"],
    ];
    for arguments in cases {
        drop(fixture.run(arguments)?.success()?);
    }
    let calls = fixture.calls()?;
    let actual: Vec<&str> = calls
        .lines()
        .filter(|line| line.starts_with("argv\t"))
        .collect();
    let expected = [
        "argv\tstack\tinit\t--base\tmain\t--\tapi\tui",
        "argv\tstack\tadd\t--all\t--message\tAdd tests\t--\ttests",
        "argv\tstack\tcheckout\t--\t42",
        "argv\tstack\tmodify\t--abort",
        "argv\tstack\tunstack\t--local\t--\t7",
        "argv\tstack\tlink\t--open\t--\t41\t42",
        "argv\tstack\tmerge\t--squash\t--yes\t--\t42",
        "argv\tstack\tpush\t--remote\torigin",
        "argv\tstack\trebase\t--downstack\t--no-trunk\t--\tapi",
        "argv\tstack\tsubmit\t--auto\t--open",
        "argv\tstack\tsync\t--prune",
        "argv\tstack\tbottom",
        "argv\tstack\tdown\t2",
        "argv\tstack\ttop",
        "argv\tstack\ttrunk",
        "argv\tstack\tup\t3",
    ];
    ensure!(actual == expected);
    Ok(())
}

#[test]
fn stack_lifecycle_rejects_interactive_or_ambiguous_forms() -> Result<()> {
    let fixture = StackFixture::new()?;
    for arguments in [
        ["stack", "checkout", "--yes"].as_slice(),
        ["stack", "checkout", "", "--yes"].as_slice(),
        ["stack", "init", "--yes"].as_slice(),
        ["stack", "init", "", "--yes"].as_slice(),
        ["stack", "modify", "--yes"].as_slice(),
        ["stack", "add", "", "--yes"].as_slice(),
        ["stack", "add", "feature", "--all", "--yes"].as_slice(),
        ["stack", "add", "feature", "--message", "", "--yes"].as_slice(),
        ["stack", "link", "feature", "--yes"].as_slice(),
        ["stack", "link", "--yes"].as_slice(),
        ["stack", "merge", "--yes"].as_slice(),
        ["stack", "merge", "zero", "--squash", "--yes"].as_slice(),
        ["stack", "unstack", "0", "--yes"].as_slice(),
        ["stack", "rebase", "", "--yes"].as_slice(),
        ["stack", "push", "--git-remote", "", "--yes"].as_slice(),
    ] {
        let run = fixture.run(arguments)?;
        ensure!(run.code == 2, "expected usage error for {arguments:?}");
    }
    ensure!(!fixture.capture.exists());
    Ok(())
}

#[test]
fn stack_lifecycle_rejects_truncated_success_output() -> Result<()> {
    let fixture = StackFixture::new()?;
    let run = fixture.run_with_large_stderr(&["stack", "top", "--yes"])?;
    ensure!(run.code == 1);
    ensure!(
        run.stderr
            .contains("gh stack output exceeded the safety limit")
    );
    Ok(())
}
