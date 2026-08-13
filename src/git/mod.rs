pub mod diff;
pub mod history;
pub mod status;
pub mod worker;

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result, bail};

use self::diff::{CommitDetails, DiffDocument, parse_diff};
use self::history::{Commit, LOG_FORMAT, parse_log};
use self::status::{Change, ChangeArea, ChangeStatus, RepoStatus, parse_porcelain_v2};

const MAX_DIFF_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_HISTORY_PAGE: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    pub current: bool,
    pub upstream: Option<String>,
    pub relative_date: String,
    pub short_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictChoice {
    Ours,
    Theirs,
}

#[derive(Debug, Clone)]
pub enum GitOperation {
    Stage(Vec<PathBuf>),
    StageAll,
    Unstage(Vec<PathBuf>),
    UnstageAll,
    Discard(Vec<Change>),
    Commit {
        message: String,
        amend: bool,
    },
    Fetch,
    Pull,
    Push,
    Sync,
    Checkout(String),
    CreateBranch {
        name: String,
        start: Option<String>,
    },
    DeleteBranch(String),
    Stash,
    StashPop,
    ResolveConflict {
        path: PathBuf,
        choice: ConflictChoice,
    },
    CherryPick(String),
    Revert(String),
}

impl GitOperation {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Stage(_) => "Staging change",
            Self::StageAll => "Staging all changes",
            Self::Unstage(_) => "Unstaging change",
            Self::UnstageAll => "Unstaging all changes",
            Self::Discard(_) => "Discarding changes",
            Self::Commit { amend: true, .. } => "Amending commit",
            Self::Commit { amend: false, .. } => "Creating commit",
            Self::Fetch => "Fetching remotes",
            Self::Pull => "Pulling changes",
            Self::Push => "Pushing changes",
            Self::Sync => "Synchronizing changes",
            Self::Checkout(_) => "Switching branch",
            Self::CreateBranch { .. } => "Creating branch",
            Self::DeleteBranch(_) => "Deleting branch",
            Self::Stash => "Stashing changes",
            Self::StashPop => "Applying stash",
            Self::ResolveConflict { .. } => "Resolving conflict",
            Self::CherryPick(_) => "Cherry-picking commit",
            Self::Revert(_) => "Reverting commit",
        }
    }

    pub fn changes_history(&self) -> bool {
        matches!(
            self,
            Self::Commit { .. }
                | Self::Pull
                | Self::Push
                | Self::Sync
                | Self::Checkout(_)
                | Self::CreateBranch { .. }
                | Self::DeleteBranch(_)
                | Self::CherryPick(_)
                | Self::Revert(_)
        )
    }
}

#[derive(Debug, Clone)]
pub struct Repository {
    root: PathBuf,
}

impl Repository {
    pub fn discover(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["rev-parse", "--show-toplevel"])
            .env("LC_ALL", "C")
            .output()
            .with_context(|| "failed to run Git; is `git` installed?")?;

        if !output.status.success() {
            bail!("{}", command_error("Not a Git repository", &output));
        }

        let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if root.is_empty() {
            bail!("Git returned an empty repository root");
        }

        Ok(Self {
            root: PathBuf::from(root),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn name(&self) -> String {
        self.root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.root.to_string_lossy().into_owned())
    }

    pub fn status(&self) -> Result<RepoStatus> {
        let output = self.checked([
            OsString::from("status"),
            OsString::from("--porcelain=v2"),
            OsString::from("--branch"),
            OsString::from("-z"),
            OsString::from("--untracked-files=all"),
            OsString::from("--ignore-submodules=none"),
        ])?;
        Ok(parse_porcelain_v2(&output))
    }

    pub fn history(&self, skip: usize, limit: usize) -> Result<Vec<Commit>> {
        let limit = if limit == 0 {
            DEFAULT_HISTORY_PAGE
        } else {
            limit
        };
        let args = vec![
            OsString::from("log"),
            OsString::from("--all"),
            OsString::from("--topo-order"),
            OsString::from("--decorate=short"),
            OsString::from("--no-color"),
            OsString::from(format!("--skip={skip}")),
            OsString::from(format!("--max-count={limit}")),
            OsString::from(format!("--format={LOG_FORMAT}")),
        ];
        let output = self.checked(args)?;
        Ok(parse_log(&output))
    }

    pub fn diff_for_changes(&self, changes: &[Change], expanded: bool) -> Result<DiffDocument> {
        let Some(first) = changes.first() else {
            return Ok(DiffDocument::empty("Working Tree", "No changes selected"));
        };
        if changes.len() == 1 {
            return self.diff_for_change(first, expanded);
        }
        let area = first.area;
        let mut patch = Vec::new();
        let mut truncated = false;
        for change in changes.iter().filter(|change| change.area == area) {
            let document = self.raw_diff_for_change(change, expanded)?;
            let remaining = MAX_DIFF_BYTES.saturating_sub(patch.len());
            if document.len() > remaining {
                let mut document = document;
                truncated |= truncate(&mut document, remaining);
                patch.extend(document);
                break;
            }
            patch.extend(document);
        }
        Ok(parse_diff(
            &patch,
            format!("{}  {} files", area.label(), changes.len()),
            None,
            truncated,
        ))
    }

    pub fn diff_for_change(&self, change: &Change, expanded: bool) -> Result<DiffDocument> {
        let mut output = self.raw_diff_for_change(change, expanded)?;
        let truncated = truncate(&mut output, MAX_DIFF_BYTES);
        let title = format!(
            "{} — {} {}",
            change.display_path(),
            change.area.label(),
            change.status.label()
        );
        Ok(parse_diff(&output, title, Some(&change.path), truncated))
    }

    fn raw_diff_for_change(&self, change: &Change, expanded: bool) -> Result<Vec<u8>> {
        if change.status == ChangeStatus::Untracked {
            return self.untracked_patch(change);
        }

        let mut args = vec![
            OsString::from("diff"),
            OsString::from("--no-color"),
            OsString::from("--no-ext-diff"),
            OsString::from("--find-renames"),
            OsString::from(if expanded {
                "--unified=1000000"
            } else {
                "--unified=3"
            }),
        ];
        if change.area == ChangeArea::Staged {
            args.push(OsString::from("--cached"));
        }
        if change.area == ChangeArea::Conflict {
            args.push(OsString::from("--cc"));
        }
        args.push(OsString::from("--"));
        args.push(change.path.as_os_str().to_owned());
        self.checked(args)
    }

    pub fn commit_detail(&self, commit: &Commit) -> Result<DiffDocument> {
        let mut output = self.checked([
            OsString::from("show"),
            OsString::from("--no-color"),
            OsString::from("--no-ext-diff"),
            OsString::from("--find-renames"),
            OsString::from("--patch"),
            OsString::from("--format="),
            OsString::from(&commit.id),
        ])?;
        let truncated = truncate(&mut output, MAX_DIFF_BYTES);
        let mut document = parse_diff(
            &output,
            format!("{} — {}", commit.short_id, commit.subject),
            None,
            truncated,
        );
        document.commit_details = Some(CommitDetails {
            id: commit.id.clone(),
            subject: commit.subject.clone(),
            author: commit.author.clone(),
            author_email: commit.author_email.clone(),
            authored_at: commit.authored_at.clone(),
            committer: commit.committer.clone(),
            committer_email: commit.committer_email.clone(),
            committed_at: commit.committed_at.clone(),
        });
        Ok(document)
    }

    pub fn branches(&self) -> Result<Vec<Branch>> {
        let output = self.checked([
            OsString::from("for-each-ref"),
            OsString::from("--sort=-committerdate"),
            OsString::from(
                "--format=%(refname:short)%1f%(HEAD)%1f%(upstream:short)%1f%(committerdate:relative)%1f%(objectname:short)%1e",
            ),
            OsString::from("refs/heads"),
        ])?;

        let mut branches = Vec::new();
        for record in output.split(|byte| *byte == 0x1e) {
            let record = trim_ascii(record);
            if record.is_empty() {
                continue;
            }
            let fields: Vec<_> = record.split(|byte| *byte == 0x1f).collect();
            if fields.len() < 5 {
                continue;
            }
            let upstream = text(fields[2]);
            branches.push(Branch {
                name: text(fields[0]),
                current: fields[1] == b"*",
                upstream: (!upstream.is_empty()).then_some(upstream),
                relative_date: text(fields[3]),
                short_id: text(fields[4]),
            });
        }
        Ok(branches)
    }

    pub fn perform(&self, operation: &GitOperation) -> Result<String> {
        match operation {
            GitOperation::Stage(paths) => {
                self.with_paths(["add"], paths)?;
                Ok(plural_message(
                    paths.len(),
                    "change staged",
                    "changes staged",
                ))
            }
            GitOperation::StageAll => {
                self.checked(strings(["add", "-A"]))?;
                Ok("All changes staged".to_owned())
            }
            GitOperation::Unstage(paths) => {
                self.unstage(paths)?;
                Ok(plural_message(
                    paths.len(),
                    "change unstaged",
                    "changes unstaged",
                ))
            }
            GitOperation::UnstageAll => {
                self.unstage_all()?;
                Ok("All changes unstaged".to_owned())
            }
            GitOperation::Discard(changes) => {
                self.discard(changes)?;
                Ok(plural_message(
                    changes.len(),
                    "change discarded",
                    "changes discarded",
                ))
            }
            GitOperation::Commit { message, amend } => {
                if message.trim().is_empty() {
                    bail!("Commit message cannot be empty");
                }
                let mut args = vec![OsString::from("commit")];
                if *amend {
                    args.push(OsString::from("--amend"));
                }
                args.push(OsString::from("--message"));
                args.push(OsString::from(message));
                self.checked(args)?;
                Ok(if *amend {
                    "Commit amended".to_owned()
                } else {
                    "Commit created".to_owned()
                })
            }
            GitOperation::Fetch => {
                self.checked(strings(["fetch", "--all", "--prune"]))?;
                Ok("Fetch complete".to_owned())
            }
            GitOperation::Pull => {
                self.checked(strings(["pull"]))?;
                Ok("Pull complete".to_owned())
            }
            GitOperation::Push => {
                self.push()?;
                Ok("Push complete".to_owned())
            }
            GitOperation::Sync => {
                self.checked(strings(["pull"]))?;
                self.push()?;
                Ok("Synchronization complete".to_owned())
            }
            GitOperation::Checkout(branch) => {
                self.checked(strings(["switch", "--", branch]))?;
                Ok(format!("Switched to {branch}"))
            }
            GitOperation::CreateBranch { name, start } => {
                self.validate_branch_name(name)?;
                let mut args = vec![
                    OsString::from("switch"),
                    OsString::from("--create"),
                    OsString::from(name),
                ];
                if let Some(start) = start {
                    args.push(OsString::from(start));
                }
                self.checked(args)?;
                Ok(format!("Created and switched to {name}"))
            }
            GitOperation::DeleteBranch(branch) => {
                self.checked(strings(["branch", "--delete", "--", branch]))?;
                Ok(format!("Deleted {branch}"))
            }
            GitOperation::Stash => {
                self.checked(strings([
                    "stash",
                    "push",
                    "--include-untracked",
                    "--message",
                    "Quinjet stash",
                ]))?;
                Ok("Changes stashed".to_owned())
            }
            GitOperation::StashPop => {
                self.checked(strings(["stash", "pop"]))?;
                Ok("Stash applied".to_owned())
            }
            GitOperation::ResolveConflict { path, choice } => {
                let side = match choice {
                    ConflictChoice::Ours => "--ours",
                    ConflictChoice::Theirs => "--theirs",
                };
                self.with_paths(["checkout", side], std::slice::from_ref(path))?;
                self.with_paths(["add"], std::slice::from_ref(path))?;
                Ok(format!("Accepted {side} for {}", path.to_string_lossy()))
            }
            GitOperation::CherryPick(commit) => {
                self.checked(strings(["cherry-pick", commit]))?;
                Ok(format!("Cherry-picked {}", short_id(commit)))
            }
            GitOperation::Revert(commit) => {
                self.checked(strings(["revert", "--no-edit", commit]))?;
                Ok(format!("Reverted {}", short_id(commit)))
            }
        }
    }

    fn untracked_patch(&self, change: &Change) -> Result<Vec<u8>> {
        let path = safe_worktree_path(&self.root, &change.path)?;
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to read {}", change.display_path()))?;
        let display_path = change.display_path();
        if !metadata.is_file() {
            return Ok(format!(
                "diff --git a/{display_path} b/{display_path}\nnew file mode 100644\nBinary files /dev/null and b/{display_path} differ\n"
            )
            .into_bytes());
        }

        let contents =
            fs::read(&path).with_context(|| format!("failed to read {}", change.display_path()))?;
        if contents.contains(&0) {
            return Ok(format!(
                "diff --git a/{display_path} b/{display_path}\nnew file mode 100644\nBinary files /dev/null and b/{display_path} differ\n"
            )
            .into_bytes());
        }

        let body = String::from_utf8_lossy(&contents);
        let line_count = body.lines().count();
        let mut patch = format!(
            "diff --git a/{display_path} b/{display_path}\nnew file mode 100644\n--- /dev/null\n+++ b/{display_path}\n@@ -0,0 +1,{line_count} @@\n"
        );
        for line in body.split_inclusive('\n') {
            patch.push('+');
            patch.push_str(line);
        }
        if !body.is_empty() && !body.ends_with('\n') {
            patch.push('\n');
            patch.push_str("\\ No newline at end of file\n");
        }
        Ok(patch.into_bytes())
    }

    fn discard(&self, changes: &[Change]) -> Result<()> {
        let mut restore_worktree = Vec::new();
        let mut restore_both = Vec::new();
        for change in changes {
            if change.status == ChangeStatus::Untracked {
                let path = safe_worktree_path(&self.root, &change.path)?;
                let metadata = fs::symlink_metadata(&path)
                    .with_context(|| format!("failed to inspect {}", change.display_path()))?;
                if metadata.is_dir() && !metadata.file_type().is_symlink() {
                    fs::remove_dir_all(&path)?;
                } else {
                    fs::remove_file(&path)?;
                }
            } else if change.area == ChangeArea::Staged {
                restore_both.push(change.path.clone());
            } else {
                restore_worktree.push(change.path.clone());
            }
        }

        if !restore_worktree.is_empty() {
            self.with_paths(["restore", "--worktree"], &restore_worktree)?;
        }
        if !restore_both.is_empty() {
            self.with_paths(
                ["restore", "--staged", "--worktree", "--source=HEAD"],
                &restore_both,
            )?;
        }
        Ok(())
    }

    fn unstage(&self, paths: &[PathBuf]) -> Result<()> {
        if self.has_head() {
            self.with_paths(["restore", "--staged"], paths)?;
        } else {
            self.with_paths(["rm", "--cached", "--ignore-unmatch"], paths)?;
        }
        Ok(())
    }

    fn unstage_all(&self) -> Result<()> {
        if self.has_head() {
            self.checked(strings(["reset", "--mixed", "--quiet", "HEAD", "--"]))?;
        } else {
            let output = self.run(strings(["rm", "--recursive", "--cached", "."]))?;
            if !output.status.success() && !self.status()?.changes.is_empty() {
                bail!("{}", command_error("Unable to unstage changes", &output));
            }
        }
        Ok(())
    }

    fn push(&self) -> Result<()> {
        let status = self.status()?;
        if status.branch.upstream.is_some() {
            self.checked(strings(["push"]))?;
            return Ok(());
        }

        let origin = self.run(strings(["remote", "get-url", "origin"]))?;
        if !origin.status.success() {
            bail!("Current branch has no upstream and no `origin` remote exists");
        }
        self.checked(strings(["push", "--set-upstream", "origin", "HEAD"]))?;
        Ok(())
    }

    fn validate_branch_name(&self, name: &str) -> Result<()> {
        if name.trim().is_empty() {
            bail!("Branch name cannot be empty");
        }
        self.checked(strings(["check-ref-format", "--branch", name]))?;
        Ok(())
    }

    fn has_head(&self) -> bool {
        self.run(strings(["rev-parse", "--verify", "HEAD"]))
            .is_ok_and(|output| output.status.success())
    }

    fn with_paths<const N: usize>(&self, prefix: [&str; N], paths: &[PathBuf]) -> Result<Vec<u8>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        let mut args: Vec<OsString> = prefix.into_iter().map(OsString::from).collect();
        args.push(OsString::from("--"));
        args.extend(paths.iter().map(|path| path.as_os_str().to_owned()));
        self.checked(args)
    }

    fn checked<I, S>(&self, args: I) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.run(args)?;
        if !output.status.success() {
            bail!("{}", command_error("Git command failed", &output));
        }
        Ok(output.stdout)
    }

    fn run<I, S>(&self, args: I) -> Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(&self.root)
            .args(["-c", "core.quotepath=false"])
            .args(args)
            .env("LC_ALL", "C")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_TERMINAL_PROMPT", "0");
        command
            .output()
            .with_context(|| format!("failed to execute Git in {}", self.root.display()))
    }
}

fn strings<const N: usize>(values: [&str; N]) -> [OsString; N] {
    values.map(OsString::from)
}

fn command_error(context: &str, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let details = if !stderr.is_empty() { stderr } else { stdout };
    if details.is_empty() {
        format!("{context} (exit status {})", output.status)
    } else {
        format!("{context}: {details}")
    }
}

fn safe_worktree_path(root: &Path, relative: &Path) -> Result<PathBuf> {
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!(
            "refusing to access path outside the repository: {}",
            relative.display()
        );
    }
    Ok(root.join(relative))
}

fn truncate(bytes: &mut Vec<u8>, maximum: usize) -> bool {
    if bytes.len() <= maximum {
        return false;
    }
    bytes.truncate(maximum);
    while bytes.last().is_some_and(|byte| *byte != b'\n') {
        bytes.pop();
    }
    true
}

fn trim_ascii(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(u8::is_ascii_whitespace) {
        value = &value[1..];
    }
    while value.last().is_some_and(u8::is_ascii_whitespace) {
        value = &value[..value.len() - 1];
    }
    value
}

fn text(value: &[u8]) -> String {
    String::from_utf8_lossy(value).into_owned()
}

fn plural_message(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("1 {singular}")
    } else {
        format!("{count} {plural}")
    }
}

fn short_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_paths_outside_worktree() {
        let root = Path::new("/tmp/repository");
        assert!(safe_worktree_path(root, Path::new("../secret")).is_err());
        assert!(safe_worktree_path(root, Path::new("/etc/passwd")).is_err());
        assert_eq!(
            safe_worktree_path(root, Path::new("src/main.rs")).unwrap(),
            PathBuf::from("/tmp/repository/src/main.rs")
        );
    }

    #[test]
    fn truncates_at_line_boundary() {
        let mut input = b"first\nsecond\nthird\n".to_vec();
        assert!(truncate(&mut input, 15));
        assert_eq!(input, b"first\nsecond\n");
    }
}
