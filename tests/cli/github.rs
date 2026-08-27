use std::os::unix::fs::PermissionsExt;

use super::*;

const GH_SCRIPT: &str = r#"#!/bin/sh
input=$(cat)
{
  printf 'argv'
  for argument in "$@"; do
    printf '\t%s' "$argument"
  done
  printf '\n'
  printf 'env\t%s\t%s\t%s\t%s\n' "$GH_PROMPT_DISABLED" "$GH_PAGER" "$GH_NO_UPDATE_NOTIFIER" "$NO_COLOR"
  printf 'stdin\t%s\n' "$input"
} >> "$FAKE_GH_CAPTURE"
case "$* $input" in
  *"stackEntry"*)
    printf '{"data":{"repository":{"pullRequest":{"stackEntry":{"position":2},"stack":{"id":"STACK_node","number":12,"size":2,"baseRefName":"main","entries":{"totalCount":2,"nodes":[{"id":"ENTRY_1","position":1,"pullRequest":{"id":"PR_41","number":41,"title":"Build stack model","author":{"login":"octocat"},"state":"OPEN","isDraft":false,"updatedAt":"2026-08-21T01:00:00Z","url":"https://github.com/acme/project/pull/41","baseRefName":"main","baseRefOid":"%s","headRefName":"stack-model","headRefOid":"%s","headRepository":{"nameWithOwner":"acme/project"},"isCrossRepository":false,"additions":1,"deletions":0,"changedFiles":1,"mergeStateStatus":"CLEAN","mergeable":"MERGEABLE","reviewDecision":"APPROVED","mergeQueueEntry":null,"commits":{"nodes":[{"commit":{"statusCheckRollup":{"state":"SUCCESS"}}}]} }},{"id":"ENTRY_2","position":2,"pullRequest":{"id":"PR_42","number":42,"title":"Add stack view","author":{"login":"octocat"},"state":"OPEN","isDraft":false,"updatedAt":"2026-08-21T02:00:00Z","url":"https://github.com/acme/project/pull/42","baseRefName":"stack-model","baseRefOid":"%s","headRefName":"stack-view","headRefOid":"%s","headRepository":{"nameWithOwner":"acme/project"},"isCrossRepository":false,"additions":0,"deletions":0,"changedFiles":0,"mergeStateStatus":"CLEAN","mergeable":"MERGEABLE","reviewDecision":"REVIEW_REQUIRED","mergeQueueEntry":null,"commits":{"nodes":[{"commit":{"statusCheckRollup":{"state":"PENDING"}}}]}}}]}}}}}}' "$FAKE_BASE_OID" "$FAKE_HEAD_OID" "$FAKE_HEAD_OID" "$FAKE_HEAD_OID"
    ;;
  *"older"*)
    printf '{"data":{"repository":{"pullRequest":{"baseRefOid":"%s","headRefOid":"%s","commits":{"totalCount":2,"nodes":[{"commit":{"oid":"%s","abbreviatedOid":"base000","messageHeadline":"build the base","authoredDate":"2026-08-20T01:00:00Z","committedDate":"2026-08-20T01:00:00Z","url":"https://github.com/acme/project/commit/base","author":{"name":"Octo Cat","user":{"login":"octocat"}},"committer":{"name":"Octo Cat","user":{"login":"octocat"}}}}],"pageInfo":{"hasPreviousPage":false,"startCursor":"base"}}}}}}' "$FAKE_BASE_OID" "$FAKE_HEAD_OID" "$FAKE_BASE_OID"
    ;;
  *"commits(last:100,before:"*)
    printf '{"data":{"repository":{"pullRequest":{"baseRefOid":"%s","headRefOid":"%s","commits":{"totalCount":2,"nodes":[{"commit":{"oid":"%s","abbreviatedOid":"feature0","messageHeadline":"add the feature","authoredDate":"2026-08-21T02:00:00Z","committedDate":"2026-08-21T02:00:00Z","url":"https://github.com/acme/project/commit/feature","author":{"name":"Octo Cat","user":{"login":"octocat"}},"committer":{"name":"Octo Cat","user":{"login":"octocat"}}}}],"pageInfo":{"hasPreviousPage":true,"startCursor":"older"}}}}}}' "$FAKE_BASE_OID" "$FAKE_HEAD_OID" "$FAKE_HEAD_OID"
    ;;
  *"number=42"*)
    printf 'PR_node\t42\tAdd feature\tBody from fixture\toctocat\tOPEN\tfalse\t2026-08-21T02:00:00Z\thttps://github.com/acme/project/pull/42\tmain\tfeature\tacme/project\tfalse\t1\t0\t1\t%s\t%s\t2026-08-20T01:00:00Z\tfalse\ttrue\tfalse\ttrue\ttrue\ttrue\ttrue\ttrue\tSUBSCRIBED\tCLEAN\tMERGEABLE\ttrue\ttrue\t\t\t0\t\t\tAPPROVED\n' "$FAKE_BASE_OID" "$FAKE_HEAD_OID"
    ;;
  "pr checks 42 "*)
    printf 'Unit tests\tCI\tSUCCESS\tpass\tAll tests passed\thttps://github.com/acme/project/actions/runs/77/job/123\t2026-08-21T01:00:00Z\t2026-08-21T01:02:00Z\n'
    printf 'Lint\tQuality\tIN_PROGRESS\tpending\tChecking style\thttps://github.com/acme/project/actions/runs/78/job/124\t2026-08-21T01:03:00Z\t\n'
    ;;
  *"actions/jobs/123/logs"*)
    printf '2026-08-21T01:00:01Z preparing runner\n2026-08-21T01:01:01Z tests passed\n'
    ;;
  *"actions/jobs/123"*)
    printf '1\tSet up\tcompleted\tsuccess\t2026-08-21T01:00:00Z\t2026-08-21T01:00:30Z\n'
    printf '2\tRun tests\tcompleted\tsuccess\t2026-08-21T01:00:31Z\t2026-08-21T01:02:00Z\n'
    ;;
  "pr close 42 "*|"pr comment 42 "*|"pr merge 42 "*)
    ;;
  *)
    printf 'unexpected fake gh invocation: %s\n' "$*" >&2
    exit 91
    ;;
esac
"#;

const OPEN_SCRIPT: &str = r#"#!/bin/sh
printf '%s\n' "$*" >> "$FAKE_OPEN_CAPTURE"
"#;

#[derive(Debug, PartialEq, Eq)]
struct RepositoryState {
    head: String,
    refs: String,
    index: String,
    status: String,
    readme: String,
    staged: String,
    untracked: String,
}

struct GitHubFixture {
    repository: Scratch,
    base_oid: String,
    head_oid: String,
    bin: PathBuf,
    gh_capture: PathBuf,
    open_capture: PathBuf,
}

impl GitHubFixture {
    fn new() -> Result<Self> {
        let repository = Scratch::repository()?;
        let base_oid = repository.git(&["rev-parse", "HEAD"])?;
        repository.git(&["switch", "--create", "feature"])?;
        repository.write("feature.txt", "from pull request\n")?;
        repository.git(&["add", "feature.txt"])?;
        repository.git(&["commit", "--message=feature"])?;
        let head_oid = repository.git(&["rev-parse", "HEAD"])?;
        repository.git(&["switch", "main"])?;
        repository.git(&[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/project.git",
        ])?;
        repository.write("README.md", "local worktree change\n")?;
        repository.write("staged.txt", "local staged change\n")?;
        repository.git(&["add", "staged.txt"])?;
        repository.write("untracked.txt", "local untracked change\n")?;
        let bin = repository.environment.join("fake-bin");
        fs::create_dir_all(&bin)?;
        executable(&bin.join("gh"), GH_SCRIPT)?;
        executable(&bin.join("open"), OPEN_SCRIPT)?;
        executable(&bin.join("xdg-open"), OPEN_SCRIPT)?;
        let gh_capture = repository.environment.join("gh-capture");
        let open_capture = repository.environment.join("open-capture");
        Ok(Self {
            repository,
            base_oid,
            head_oid,
            bin,
            gh_capture,
            open_capture,
        })
    }

    fn command(&self, args: &[&str]) -> Result<ProcessCommand> {
        let mut paths = vec![self.bin.clone()];
        if let Some(current) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&current));
        }
        let mut command = self.repository.quinjet_command(args);
        command
            .env("PATH", std::env::join_paths(paths)?)
            .env("FAKE_GH_CAPTURE", &self.gh_capture)
            .env("FAKE_OPEN_CAPTURE", &self.open_capture)
            .env("FAKE_BASE_OID", &self.base_oid)
            .env("FAKE_HEAD_OID", &self.head_oid);
        Ok(command)
    }

    fn run(&self, args: &[&str]) -> Result<Run> {
        Run::from(
            self.command(args)?
                .output()
                .context("failed to run Quinjet with fake GitHub")?,
        )
    }

    fn read(&self, args: &[&str]) -> Result<Run> {
        let before = self.state()?;
        let run = self.run(args)?;
        let after = self.state()?;
        ensure!(
            before == after,
            "GitHub read changed repository state: {before:?} != {after:?}"
        );
        Ok(run)
    }

    fn state(&self) -> Result<RepositoryState> {
        Ok(RepositoryState {
            head: self.repository.git(&["rev-parse", "HEAD"])?,
            refs: self
                .repository
                .git(&["for-each-ref", "--format=%(refname) %(objectname)"])?,
            index: self.repository.git(&["ls-files", "--stage"])?,
            status: self
                .repository
                .git(&["status", "--porcelain=v1", "--untracked-files=all"])?,
            readme: fs::read_to_string(self.repository.path.join("README.md"))?,
            staged: fs::read_to_string(self.repository.path.join("staged.txt"))?,
            untracked: fs::read_to_string(self.repository.path.join("untracked.txt"))?,
        })
    }

    fn gh_calls(&self) -> Result<String> {
        fs::read_to_string(&self.gh_capture).context("fake gh did not record a call")
    }

    fn clear_gh_calls(&self) {
        drop(fs::remove_file(&self.gh_capture));
    }

    fn assert_noninteractive_transport(&self) -> Result<()> {
        let calls = self.gh_calls()?;
        let mut environment = calls.lines().filter(|line| line.starts_with("env\t"));
        ensure!(
            environment.clone().count() > 0,
            "fake gh recorded no environment"
        );
        ensure!(environment.all(|line| line == "env\t1\tcat\t1\t1"));
        ensure!(
            calls
                .lines()
                .filter(|line| line.starts_with("stdin\t"))
                .all(|line| line == "stdin\t")
        );
        Ok(())
    }
}

fn executable(path: &Path, contents: &str) -> Result<()> {
    fs::write(path, contents)?;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

fn wait_for_capture(path: &Path) -> Result<String> {
    for _ in 0..100 {
        if let Ok(contents) = fs::read_to_string(path)
            && !contents.is_empty()
        {
            return Ok(contents);
        }
        thread::sleep(Duration::from_millis(10));
    }
    fs::read_to_string(path).context("the fake opener did not run")
}

#[test]
fn pull_request_view_has_plain_and_json_faces_without_touching_local_work() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let plain = fixture.read(&["pr", "view", "42"])?.success()?;

    ensure!(plain.stderr.is_empty(), "{}", plain.stderr);
    ensure!(plain.stdout.contains("#42  Add feature"));
    ensure!(plain.stdout.contains("Body from fixture"));
    ensure!(
        plain
            .stdout
            .contains("https://github.com/acme/project/pull/42")
    );

    let json = fixture
        .read(&["pr", "view", "42", "--refresh", "--json"])?
        .success()?;
    let value = json.json()?;
    ensure!(value["pullRequest"]["number"] == 42);
    ensure!(value["pullRequest"]["baseOid"] == fixture.base_oid);
    ensure!(value["pullRequest"]["headOid"] == fixture.head_oid);
    fixture.assert_noninteractive_transport()
}

#[test]
fn pull_request_files_uses_real_local_commits_in_plain_and_json() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let plain = fixture.read(&["pr", "files", "42"])?.success()?;

    ensure!(plain.stderr.is_empty(), "{}", plain.stderr);
    ensure!(plain.stdout.contains("A feature.txt"), "{}", plain.stdout);
    ensure!(plain.stdout.contains("+1 -0"), "{}", plain.stdout);

    let json = fixture
        .read(&["pr", "files", "42", "--json"])?
        .success()?
        .json()?;
    ensure!(json["files"][0]["path"] == "feature.txt");
    ensure!(json["files"][0]["status"] == "added");
    ensure!(json["totalFiles"] == 1);
    Ok(())
}

#[test]
fn pull_request_commits_paginate_cache_and_render_in_both_modes() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let plain = fixture.read(&["pr", "commits", "42"])?.success()?;

    ensure!(plain.stderr.is_empty(), "{}", plain.stderr);
    let base = plain
        .stdout
        .find("build the base")
        .context("plain commits omitted the older commit")?;
    let feature = plain
        .stdout
        .find("add the feature")
        .context("plain commits omitted the head commit")?;
    ensure!(base < feature, "{}", plain.stdout);

    let json = fixture
        .read(&["pr", "commits", "42", "--json"])?
        .success()?
        .json()?;
    ensure!(
        json["commits"]
            .as_array()
            .is_some_and(|commits| commits.len() == 2)
    );
    ensure!(json["commits"][0]["oid"] == fixture.base_oid);
    ensure!(json["commits"][1]["oid"] == fixture.head_oid);
    ensure!(json["totalCommits"] == 2);
    ensure!(json["truncated"] == false);
    ensure!(json["fromCache"] == true);

    let calls = fixture.gh_calls()?;
    ensure!(calls.matches("commits(last:100,before:").count() == 2);
    Ok(())
}

#[test]
fn pull_request_diff_renders_the_real_patch_in_plain_and_json() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let plain = fixture.read(&["pr", "diff", "42"])?.success()?;

    ensure!(plain.stderr.is_empty(), "{}", plain.stderr);
    ensure!(plain.stdout.contains("feature.txt"), "{}", plain.stdout);
    ensure!(
        plain.stdout.contains("+from pull request"),
        "{}",
        plain.stdout
    );

    let json = fixture
        .read(&["pr", "diff", "42", "--json"])?
        .success()?
        .json()?;
    ensure!(json["title"] == "PR #42");
    ensure!(json.to_string().contains("from pull request"));
    Ok(())
}

#[test]
fn stack_reads_show_the_ladder_and_exact_composed_diff() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let view = fixture.read(&["stack", "view", "42"])?.success()?;
    ensure!(view.stdout.contains("Stack #12"), "{}", view.stdout);
    ensure!(view.stdout.contains("#41"), "{}", view.stdout);
    ensure!(view.stdout.contains(">   2  #42"), "{}", view.stdout);

    let json = fixture
        .read(&["stack", "view", "42", "--json"])?
        .success()?
        .json()?;
    ensure!(json["stack"]["number"] == 12);
    ensure!(json["stack"]["selectedPosition"] == 2);
    ensure!(
        json["stack"]["members"]
            .as_array()
            .is_some_and(|members| members.len() == 2)
    );

    let files = fixture
        .read(&["stack", "files", "42", "--from", "1", "--to", "2"])?
        .success()?;
    ensure!(files.stdout.contains("A feature.txt"), "{}", files.stdout);

    let diff = fixture
        .read(&["stack", "diff", "42", "--from", "1", "--to", "2"])?
        .success()?;
    ensure!(diff.stdout.contains("feature.txt"), "{}", diff.stdout);
    ensure!(
        diff.stdout.contains("+from pull request"),
        "{}",
        diff.stdout
    );
    fixture.assert_noninteractive_transport()
}

#[test]
fn pull_request_checks_cover_plain_json_and_exit_code_modes() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let plain = fixture.read(&["pr", "checks", "42"])?.success()?;

    ensure!(plain.stdout.contains("Unit tests"));
    ensure!(plain.stdout.contains("Lint"));
    ensure!(plain.stdout.contains("1 passed, 1 pending, 0 failed"));

    let json = fixture
        .read(&["pr", "checks", "42", "--json"])?
        .success()?
        .json()?;
    ensure!(
        json["checks"]
            .as_array()
            .is_some_and(|checks| checks.len() == 2)
    );
    ensure!(json.to_string().contains("\"status\":\"pending\""));

    let gated = fixture.read(&["pr", "checks", "42", "--exit-code"])?;
    ensure!(gated.code == 1, "{}", gated.stderr);
    ensure!(gated.stderr.is_empty(), "{}", gated.stderr);
    ensure!(gated.stdout.contains("1 pending"));
    Ok(())
}

#[test]
fn pull_request_logs_render_fake_steps_and_runner_output() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let plain = fixture
        .read(&["pr", "logs", "42", "Unit tests"])?
        .success()?;

    ensure!(plain.stdout.contains("Set up"), "{}", plain.stdout);
    ensure!(plain.stdout.contains("Run tests"), "{}", plain.stdout);
    ensure!(
        plain.stdout.contains("preparing runner"),
        "{}",
        plain.stdout
    );
    ensure!(plain.stdout.contains("tests passed"), "{}", plain.stdout);

    let json = fixture
        .read(&["pr", "logs", "42", "Unit tests", "--json"])?
        .success()?
        .json()?;
    ensure!(
        json["steps"]
            .as_array()
            .is_some_and(|steps| steps.len() == 2)
    );
    ensure!(json.to_string().contains("tests passed"));
    Ok(())
}

#[test]
fn pull_request_open_uses_the_selected_check_url() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let opened = fixture
        .read(&["pr", "open", "42", "--check", "Unit tests"])?
        .success()?;

    let expected = "https://github.com/acme/project/actions/runs/77/job/123";
    ensure!(opened.stderr.is_empty(), "{}", opened.stderr);
    ensure!(opened.stdout == format!("Opened {expected}\n"));
    ensure!(wait_for_capture(&fixture.open_capture)?.trim() == expected);
    ensure!(fixture.gh_calls()?.contains("argv\tpr\tchecks\t42"));
    Ok(())
}

#[test]
fn close_preview_does_not_dispatch_but_yes_uses_direct_transport() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let preview = fixture.read(&["pr", "close", "42"])?.success()?;

    ensure!(preview.stdout.contains("Would close #42"));
    ensure!(!fixture.gh_calls()?.contains("argv\tpr\tclose"));
    fixture.clear_gh_calls();

    let applied = fixture
        .read(&["pr", "close", "42", "--yes", "--json"])?
        .success()?;

    ensure!(applied.json()?["message"] == "Closed #42");
    ensure!(
        fixture
            .gh_calls()?
            .contains("argv\tpr\tclose\t42\t--repo\thttps://github.com/acme/project")
    );
    fixture.assert_noninteractive_transport()
}

#[test]
fn merge_preview_does_not_dispatch_but_yes_pins_the_head_oid() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let preview = fixture
        .read(&["pr", "merge", "42", "--squash"])?
        .success()?;

    ensure!(preview.stdout.contains("Would squash and merge #42"));
    ensure!(!fixture.gh_calls()?.contains("argv\tpr\tmerge"));
    fixture.clear_gh_calls();

    let applied = fixture
        .read(&["pr", "merge", "42", "--squash", "--delete-branch", "--yes"])?
        .success()?;

    ensure!(applied.stdout == "Squashed and merged #42\n");
    let calls = fixture.gh_calls()?;
    ensure!(
        calls.contains("argv\tpr\tmerge\t42\t--repo\thttps://github.com/acme/project\t--squash")
    );
    ensure!(calls.contains(&format!("--match-head-commit\t{}", fixture.head_oid)));
    ensure!(calls.contains("--delete-branch"));
    Ok(())
}

#[test]
fn comment_preview_does_not_dispatch_but_yes_passes_the_body() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let preview = fixture
        .read(&["pr", "comment", "42", "Looks good"])?
        .success()?;

    ensure!(preview.stdout.contains("Would comment on pull request #42"));
    ensure!(!fixture.gh_calls()?.contains("argv\tpr\tcomment"));
    fixture.clear_gh_calls();

    let applied = fixture
        .read(&["pr", "comment", "42", "Looks good", "--yes"])?
        .success()?;

    ensure!(applied.stdout == "Commented on #42\n");
    ensure!(fixture.gh_calls()?.contains(
        "argv\tpr\tcomment\t42\t--repo\thttps://github.com/acme/project\t--body\tLooks good"
    ));
    Ok(())
}
