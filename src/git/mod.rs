pub(crate) mod diff;
pub(crate) mod github;
pub(crate) mod history;
pub(crate) mod status;
pub(crate) mod worker;

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;

use self::diff::{
    CommitDetails, DiffDocument, DiffFileIndexEntry, DiffIndex, DiffLineCounts, parse_diff,
    parse_numstat,
};
use self::github::{bounded_command_error, run_bounded_command};
use self::history::{Commit, LOG_FORMAT, parse_log};
use self::status::{Change, ChangeArea, ChangeStatus, RepoStatus, parse_porcelain_v2};

const MAX_DIFF_BYTES: usize = 8 * 1024 * 1024;
const MAX_DIFF_INDEX_BYTES: usize = 8 * 1024 * 1024;
const MAX_DIFF_INDEX_FILES: usize = 16_384;
const MAX_GIT_ERROR_BYTES: usize = 128 * 1024;
const DEFAULT_HISTORY_PAGE: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Branch {
    pub name: String,
    pub current: bool,
    pub upstream: Option<String>,
    pub relative_date: String,
    pub short_id: String,
}

/// A local or remote-tracking branch that can be inspected without changing HEAD.
/// `reference` is always a full ref emitted by Git and is used only as a revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryBranch {
    pub name: String,
    pub reference: String,
    pub current: bool,
    pub remote: bool,
    pub relative_date: String,
    pub short_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Stash {
    pub reference: String,
    pub message: String,
    pub branch: String,
    pub relative_date: String,
    pub short_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Worktree {
    pub path: PathBuf,
    pub head: String,
    pub branch: Option<String>,
    pub current: bool,
    pub bare: bool,
    pub detached: bool,
    pub locked: Option<String>,
    pub prunable: Option<String>,
}

impl Worktree {
    pub(crate) fn short_head(&self) -> &str {
        self.head.get(..8).unwrap_or(&self.head)
    }

    pub(crate) fn branch_label(&self) -> String {
        self.branch.as_deref().map_or_else(
            || {
                if self.bare {
                    "bare".to_owned()
                } else if self.detached {
                    "detached".to_owned()
                } else {
                    "-".to_owned()
                }
            },
            ToOwned::to_owned,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectGroup {
    pub name: String,
    pub common_dir: PathBuf,
    pub worktrees: Vec<Worktree>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LocalDiffRequest {
    Changes {
        changes: Vec<Change>,
        version: u64,
        expanded: bool,
    },
    Commit {
        commit: Box<Commit>,
        expanded: bool,
    },
    Branch {
        branch: Box<HistoryBranch>,
        current: String,
        current_oid: Option<String>,
        expanded: bool,
    },
    Stash {
        stash: Box<Stash>,
        expanded: bool,
    },
}

pub(crate) struct PreparedLocalDiff {
    repository: Repository,
    request: LocalDiffRequest,
    index: DiffIndex,
}

impl PreparedLocalDiff {
    pub(crate) fn index(&self) -> DiffIndex {
        self.index.clone()
    }

    pub(crate) fn diff_file(&self, path: &Path) -> Result<DiffDocument> {
        self.repository
            .local_diff_file(&self.request, &self.index, path)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ConflictChoice {
    Ours,
    Theirs,
}

#[derive(Debug, Clone)]
pub(crate) enum GitOperation {
    Stage(Vec<PathBuf>),
    StageAll,
    Unstage(Vec<PathBuf>),
    UnstageAll,
    Discard(Vec<Change>),
    Remove(Vec<PathBuf>),
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
    RenameBranch {
        old: String,
        new: String,
    },
    DeleteBranch(String),
    StashPush {
        message: String,
        include_untracked: bool,
        staged: bool,
        paths: Vec<PathBuf>,
    },
    StashApply(String),
    StashPop(Option<String>),
    StashDrop(String),
    StashClear,
    ResolveConflict {
        path: PathBuf,
        choice: ConflictChoice,
    },
    CherryPick(String),
    Revert(String),
}

impl GitOperation {
    pub(crate) const fn label(&self) -> &'static str {
        match self {
            Self::Stage(_) => "Staging change",
            Self::StageAll => "Staging all changes",
            Self::Unstage(_) => "Unstaging change",
            Self::UnstageAll => "Unstaging all changes",
            Self::Discard(_) => "Discarding changes",
            Self::Remove(_) => "Removing files",
            Self::Commit { amend: true, .. } => "Amending commit",
            Self::Commit { amend: false, .. } => "Creating commit",
            Self::Fetch => "Fetching remotes",
            Self::Pull => "Pulling changes",
            Self::Push => "Pushing changes",
            Self::Sync => "Synchronizing changes",
            Self::Checkout(_) => "Switching branch",
            Self::CreateBranch { .. } => "Creating branch",
            Self::RenameBranch { .. } => "Renaming branch",
            Self::DeleteBranch(_) => "Deleting branch",
            Self::StashPush { .. } => "Stashing changes",
            Self::StashApply(_) => "Applying stash",
            Self::StashPop(_) => "Popping stash",
            Self::StashDrop(_) => "Dropping stash",
            Self::StashClear => "Dropping all stashes",
            Self::ResolveConflict { .. } => "Resolving conflict",
            Self::CherryPick(_) => "Cherry-picking commit",
            Self::Revert(_) => "Reverting commit",
        }
    }

    pub(crate) const fn changes_history(&self) -> bool {
        matches!(
            self,
            Self::Commit { .. }
                | Self::Pull
                | Self::Push
                | Self::Sync
                | Self::Checkout(_)
                | Self::CreateBranch { .. }
                | Self::RenameBranch { .. }
                | Self::DeleteBranch(_)
                | Self::CherryPick(_)
                | Self::Revert(_)
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Repository {
    root: PathBuf,
}

impl Repository {
    pub(crate) fn discover(path: impl AsRef<Path>) -> Result<Self> {
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

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn clone_for_worker(&self) -> Self {
        self.clone()
    }

    pub(crate) fn name(&self) -> String {
        self.root.file_name().map_or_else(
            || self.root.to_string_lossy().into_owned(),
            |name| name.to_string_lossy().into_owned(),
        )
    }

    pub(crate) fn status(&self) -> Result<RepoStatus> {
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

    pub(crate) fn resolve_revision(&self, revision: &str) -> Result<String> {
        let revision = revision.trim();
        if revision.is_empty() || revision.starts_with('-') {
            bail!("refusing to resolve `{revision}` as a revision");
        }
        if revision == "HEAD" {
            return Ok(revision.to_owned());
        }
        if let Some(reference) =
            self.rev_parse(["--symbolic-full-name", "--verify", "--quiet", revision])
            && (reference.starts_with("refs/heads/")
                || reference.starts_with("refs/remotes/")
                || reference.starts_with("refs/tags/"))
        {
            return Ok(reference);
        }
        self.rev_parse(["--verify", "--quiet", &format!("{revision}^{{commit}}")])
            .ok_or_else(|| anyhow!("`{revision}` does not name a commit in this repository"))
    }

    fn rev_parse<const N: usize>(&self, args: [&str; N]) -> Option<String> {
        let mut command = vec![OsString::from("rev-parse")];
        command.extend(args.iter().map(OsString::from));
        let output = self.run(command).ok()?;
        if !output.status.success() {
            return None;
        }
        let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        (!value.is_empty()).then_some(value)
    }

    pub(crate) fn history(&self, revision: &str, skip: usize, limit: usize) -> Result<Vec<Commit>> {
        if revision != "HEAD"
            && !revision.starts_with("refs/heads/")
            && !revision.starts_with("refs/remotes/")
            && !revision.starts_with("refs/tags/")
            && !is_full_oid(revision)
        {
            bail!("refusing to load history for an invalid branch reference");
        }
        let limit = if limit == 0 {
            DEFAULT_HISTORY_PAGE
        } else {
            limit
        };
        let args = vec![
            OsString::from("log"),
            OsString::from("--topo-order"),
            OsString::from("--decorate=short"),
            OsString::from("--no-color"),
            OsString::from(format!("--skip={skip}")),
            OsString::from(format!("--max-count={limit}")),
            OsString::from(format!("--format={LOG_FORMAT}")),
            OsString::from(revision),
            OsString::from("--"),
        ];
        let output = self.checked(args)?;
        Ok(parse_log(&output))
    }

    pub(crate) fn prepare_local_diff(
        &self,
        request: &LocalDiffRequest,
    ) -> Result<PreparedLocalDiff> {
        let index = self.local_diff_index(request)?;
        Ok(PreparedLocalDiff {
            repository: self.clone_for_worker(),
            request: request.clone(),
            index,
        })
    }

    #[expect(
        clippy::option_if_let_else,
        reason = "the branch is one arm of a longer chain that map_or_else cannot express"
    )]
    fn local_diff_index(&self, request: &LocalDiffRequest) -> Result<DiffIndex> {
        match request {
            LocalDiffRequest::Changes { changes, .. } => {
                let title = changes.first().map_or_else(
                    || "Working Tree".to_owned(),
                    |first| {
                        if changes.len() == 1 {
                            format!(
                                "{} — {} {}",
                                first.display_path(),
                                first.area.label(),
                                first.status.label()
                            )
                        } else {
                            format!("{}  {} files", first.area.label(), changes.len())
                        }
                    },
                );
                let mut files: Vec<_> = changes
                    .iter()
                    .map(|change| {
                        DiffFileIndexEntry::new(
                            change.path.clone(),
                            change.original_path.clone(),
                            change.status.label().to_ascii_lowercase(),
                        )
                    })
                    .collect();
                self.apply_worktree_counts(&mut files, changes);
                Ok(DiffIndex {
                    title,
                    files,
                    truncated: false,
                    commit_details: None,
                })
            }
            LocalDiffRequest::Commit { commit, .. } => {
                let args = if let Some(parent) = commit.parent_ids.first() {
                    diff_index_args(parent, &commit.id)
                } else {
                    vec![
                        OsString::from("diff-tree"),
                        OsString::from("--root"),
                        OsString::from("--no-commit-id"),
                        OsString::from("--name-status"),
                        OsString::from("-z"),
                        OsString::from("-r"),
                        OsString::from("--find-renames"),
                        OsString::from(&commit.id),
                        OsString::from("--"),
                    ]
                };
                let (files, truncated) = self.diff_index_files(args)?;
                Ok(DiffIndex {
                    title: format!("{} — {}", commit.short_id, commit.subject),
                    files,
                    truncated,
                    commit_details: Some(commit_details(commit)),
                })
            }
            LocalDiffRequest::Branch {
                branch, current, ..
            } => {
                validate_history_reference(&branch.reference)?;
                let (files, truncated) =
                    self.diff_index_files(diff_index_args(&branch.reference, "HEAD"))?;
                Ok(DiffIndex {
                    title: format!("{} → {} — branch comparison", branch.name, current),
                    files,
                    truncated,
                    commit_details: None,
                })
            }
            LocalDiffRequest::Stash { stash, .. } => {
                validate_stash_reference(&stash.reference)?;
                let (files, truncated) = self.diff_index_files(vec![
                    OsString::from("stash"),
                    OsString::from("show"),
                    OsString::from("--name-status"),
                    OsString::from("-z"),
                    OsString::from("--include-untracked"),
                    OsString::from(&stash.reference),
                    OsString::from("--"),
                ])?;
                Ok(DiffIndex {
                    title: format!("{} — {}", stash.reference, stash.message),
                    files,
                    truncated,
                    commit_details: None,
                })
            }
        }
    }

    /// Working-tree changes are already known from the status snapshot, so the
    /// index needs only their totals. One `--numstat` read per populated area
    /// keeps that to at most two extra Git calls regardless of file count.
    fn apply_worktree_counts(&self, files: &mut [DiffFileIndexEntry], changes: &[Change]) {
        let counts_for = |staged: bool| {
            let mut args = vec![OsString::from("diff"), OsString::from("--numstat")];
            if staged {
                args.push(OsString::from("--cached"));
            }
            args.extend([
                OsString::from("-z"),
                OsString::from("--find-renames"),
                OsString::from("--"),
            ]);
            self.numstat_counts(args)
        };
        let staged = if changes
            .iter()
            .any(|change| change.area == ChangeArea::Staged)
        {
            counts_for(true)
        } else {
            HashMap::new()
        };
        let unstaged = if changes
            .iter()
            .any(|change| change.area != ChangeArea::Staged)
        {
            counts_for(false)
        } else {
            HashMap::new()
        };
        for (file, change) in files.iter_mut().zip(changes) {
            let counts = if change.area == ChangeArea::Staged {
                &staged
            } else {
                &unstaged
            };
            file.counts = counts.get(&file.path).copied();
        }
    }

    /// Counts are a rendering enhancement, never a correctness requirement, so a
    /// failed or bounded read simply leaves the affected headers unresolved.
    fn numstat_counts(&self, args: Vec<OsString>) -> HashMap<PathBuf, DiffLineCounts> {
        self.checked_bounded(args, MAX_DIFF_INDEX_BYTES)
            .map(|(output, _)| parse_numstat(&output))
            .unwrap_or_default()
    }

    fn diff_index_files(&self, args: Vec<OsString>) -> Result<(Vec<DiffFileIndexEntry>, bool)> {
        let counts = numstat_args(&args)
            .map(|args| self.numstat_counts(args))
            .unwrap_or_default();
        let (mut output, command_truncated) = self.checked_bounded(args, MAX_DIFF_INDEX_BYTES)?;
        let mut truncated = command_truncated || truncate_diff_index(&mut output);
        if command_truncated && !output.ends_with(&[0]) {
            let boundary = output
                .iter()
                .rposition(|byte| *byte == 0)
                .map_or(0, |index| index + 1);
            output.truncate(boundary);
        }
        let records = output
            .split(|byte| *byte == 0)
            .filter(|record| !record.is_empty())
            .collect::<Vec<_>>();
        let mut files = Vec::new();
        let mut cursor = 0;
        while cursor < records.len() {
            if files.len() >= MAX_DIFF_INDEX_FILES {
                truncated = true;
                break;
            }
            let Some(status) = records.get(cursor).copied() else {
                break;
            };
            cursor += 1;
            let status_code = status.first().copied().unwrap_or_default();
            let rename_or_copy = matches!(status_code, b'R' | b'C');
            let Some(first_path) = records.get(cursor) else {
                truncated = true;
                break;
            };
            cursor += 1;
            let first_path = PathBuf::from(String::from_utf8_lossy(first_path).into_owned());
            let (old_path, path) = if rename_or_copy {
                let Some(new_path) = records.get(cursor) else {
                    truncated = true;
                    break;
                };
                cursor += 1;
                (
                    Some(first_path),
                    PathBuf::from(String::from_utf8_lossy(new_path).into_owned()),
                )
            } else {
                (None, first_path)
            };
            let counts = counts.get(&path).copied();
            files.push(DiffFileIndexEntry {
                path,
                old_path,
                status: diff_status_label(status_code).to_owned(),
                counts,
            });
        }
        Ok((files, truncated))
    }

    fn local_diff_file(
        &self,
        request: &LocalDiffRequest,
        index: &DiffIndex,
        path: &Path,
    ) -> Result<DiffDocument> {
        let file = index
            .files
            .iter()
            .find(|file| file.path == path)
            .with_context(|| format!("{} is not part of this diff", path.display()))?;
        match request {
            LocalDiffRequest::Changes {
                changes, expanded, ..
            } => {
                let change = changes
                    .iter()
                    .find(|change| change.path == path)
                    .with_context(|| format!("{} is no longer changed", path.display()))?;
                self.diff_for_change(change, *expanded)
            }
            LocalDiffRequest::Commit { commit, expanded } => {
                let mut document = if let Some(parent) = commit.parent_ids.first() {
                    self.revision_diff_file(parent, &commit.id, file, *expanded, &index.title)?
                } else {
                    self.root_commit_diff_file(commit, file, *expanded, &index.title)?
                };
                document.commit_details = Some(commit_details(commit));
                Ok(document)
            }
            LocalDiffRequest::Branch {
                branch, expanded, ..
            } => self.revision_diff_file(&branch.reference, "HEAD", file, *expanded, &index.title),
            LocalDiffRequest::Stash { stash, expanded } => {
                self.stash_diff_file(stash, file, *expanded, &index.title)
            }
        }
    }

    fn revision_diff_file(
        &self,
        base: &str,
        head: &str,
        file: &DiffFileIndexEntry,
        expanded: bool,
        title: &str,
    ) -> Result<DiffDocument> {
        let mut args = vec![
            OsString::from("diff"),
            OsString::from("--no-color"),
            OsString::from("--no-ext-diff"),
            OsString::from("--find-renames"),
            OsString::from("--patch"),
            OsString::from(if expanded {
                "--unified=1000000"
            } else {
                "--unified=3"
            }),
            OsString::from(base),
            OsString::from(head),
            OsString::from("--"),
        ];
        append_diff_file_paths(&mut args, file);
        self.diff_document_from_args(args, title, &file.path)
    }

    fn root_commit_diff_file(
        &self,
        commit: &Commit,
        file: &DiffFileIndexEntry,
        expanded: bool,
        title: &str,
    ) -> Result<DiffDocument> {
        let mut args = vec![
            OsString::from("show"),
            OsString::from("--format="),
            OsString::from("--no-color"),
            OsString::from("--no-ext-diff"),
            OsString::from("--find-renames"),
            OsString::from("--patch"),
            OsString::from(if expanded {
                "--unified=1000000"
            } else {
                "--unified=3"
            }),
            OsString::from(&commit.id),
            OsString::from("--"),
        ];
        append_diff_file_paths(&mut args, file);
        self.diff_document_from_args(args, title, &file.path)
    }

    fn stash_diff_file(
        &self,
        stash: &Stash,
        file: &DiffFileIndexEntry,
        expanded: bool,
        title: &str,
    ) -> Result<DiffDocument> {
        let context = if expanded {
            "--unified=1000000"
        } else {
            "--unified=3"
        };
        let mut tracked_args = vec![
            OsString::from("diff"),
            OsString::from("--no-color"),
            OsString::from("--no-ext-diff"),
            OsString::from("--find-renames"),
            OsString::from("--patch"),
            OsString::from(context),
            OsString::from(format!("{}^1", stash.reference)),
            OsString::from(&stash.reference),
            OsString::from("--"),
        ];
        append_diff_file_paths(&mut tracked_args, file);
        let (mut output, mut truncated) = self.checked_bounded(tracked_args, MAX_DIFF_BYTES)?;

        let untracked_commit = format!("{}^3", stash.reference);
        let untracked_exists = self
            .run([
                OsString::from("rev-parse"),
                OsString::from("--verify"),
                OsString::from("--quiet"),
                OsString::from(&untracked_commit),
            ])
            .is_ok_and(|result| result.status.success());
        if untracked_exists && !truncated {
            let mut untracked_args = vec![
                OsString::from("show"),
                OsString::from("--format="),
                OsString::from("--no-color"),
                OsString::from("--no-ext-diff"),
                OsString::from("--find-renames"),
                OsString::from("--patch"),
                OsString::from(context),
                OsString::from(untracked_commit),
                OsString::from("--"),
            ];
            append_diff_file_paths(&mut untracked_args, file);
            let (untracked, untracked_truncated) =
                self.checked_bounded(untracked_args, MAX_DIFF_BYTES.saturating_sub(output.len()))?;
            output.extend(untracked);
            truncated |= untracked_truncated;
        }

        if truncated {
            truncate_to_complete_line(&mut output);
        }
        Ok(parse_diff(&output, title, Some(&file.path), truncated))
    }

    fn diff_document_from_args<I, S>(
        &self,
        args: I,
        title: &str,
        path: &Path,
    ) -> Result<DiffDocument>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let (mut output, truncated) = self.checked_bounded(args, MAX_DIFF_BYTES)?;
        if truncated {
            truncate_to_complete_line(&mut output);
        }
        Ok(parse_diff(&output, title, Some(path), truncated))
    }

    pub(crate) fn diff_for_change(&self, change: &Change, expanded: bool) -> Result<DiffDocument> {
        let (output, truncated) = self.raw_diff_for_change(change, expanded)?;
        let title = format!(
            "{} — {} {}",
            change.display_path(),
            change.area.label(),
            change.status.label()
        );
        Ok(parse_diff(&output, title, Some(&change.path), truncated))
    }

    fn raw_diff_for_change(&self, change: &Change, expanded: bool) -> Result<(Vec<u8>, bool)> {
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
        let (mut output, truncated) = self.checked_bounded(args, MAX_DIFF_BYTES)?;
        if truncated {
            truncate_to_complete_line(&mut output);
        }
        Ok((output, truncated))
    }

    pub(crate) fn has_commit(&self, oid: &str) -> bool {
        is_full_oid(oid)
            && self
                .run([
                    OsString::from("cat-file"),
                    OsString::from("-e"),
                    OsString::from(format!("{oid}^{{commit}}")),
                ])
                .is_ok_and(|output| output.status.success())
    }

    pub(crate) fn branches(&self) -> Result<Vec<Branch>> {
        let output = self.checked([
            OsString::from("for-each-ref"),
            OsString::from("--sort=-committerdate"),
            OsString::from(
                "--format=%(refname:short)%1f%(HEAD)%1f%(upstream:short)%1f%(committerdate:iso-strict)%1f%(objectname:short)%1e",
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
            let [name, head, upstream, relative_date, short_id, ..] = fields.as_slice() else {
                continue;
            };
            let upstream = text(upstream);
            branches.push(Branch {
                name: text(name),
                current: *head == b"*",
                upstream: (!upstream.is_empty()).then_some(upstream),
                relative_date: text(relative_date),
                short_id: text(short_id),
            });
        }
        Ok(branches)
    }

    pub(crate) fn history_branches(&self) -> Result<Vec<HistoryBranch>> {
        let output = self.checked([
            OsString::from("for-each-ref"),
            OsString::from("--sort=-committerdate"),
            OsString::from(
                "--format=%(refname:short)%1f%(refname)%1f%(HEAD)%1f%(committerdate:iso-strict)%1f%(objectname:short)%1f%(symref)%1e",
            ),
            OsString::from("refs/heads"),
            OsString::from("refs/remotes"),
        ])?;

        let mut branches = Vec::new();
        for record in output.split(|byte| *byte == 0x1e) {
            let record = trim_ascii(record);
            if record.is_empty() {
                continue;
            }
            let fields: Vec<_> = record.split(|byte| *byte == 0x1f).collect();
            let [name, reference, head, relative_date, short_id, symref, ..] = fields.as_slice()
            else {
                continue;
            };
            if !trim_ascii(symref).is_empty() {
                continue;
            }
            let reference = text(reference);
            let remote = reference.starts_with("refs/remotes/");
            if !reference.starts_with("refs/heads/") && !remote {
                continue;
            }
            branches.push(HistoryBranch {
                name: text(name),
                reference,
                current: *head == b"*",
                remote,
                relative_date: text(relative_date),
                short_id: text(short_id),
            });
        }
        branches.sort_by_key(|branch| (!branch.current, branch.remote));
        Ok(branches)
    }

    pub(crate) fn stashes(&self) -> Result<Vec<Stash>> {
        let output = self.checked([
            OsString::from("stash"),
            OsString::from("list"),
            OsString::from("--format=%gd%x1f%gs%x1f%cI%x1f%h%x1e"),
        ])?;
        let mut stashes = Vec::new();
        for record in output.split(|byte| *byte == 0x1e) {
            let record = trim_ascii(record);
            if record.is_empty() {
                continue;
            }
            let fields: Vec<_> = record.split(|byte| *byte == 0x1f).collect();
            let [reference, subject, relative_date, short_id, ..] = fields.as_slice() else {
                continue;
            };
            let reference = text(reference);
            if !valid_stash_reference(&reference) {
                continue;
            }
            let subject = text(subject);
            let (branch, message) = parse_stash_subject(&subject);
            stashes.push(Stash {
                reference,
                message,
                branch,
                relative_date: text(relative_date),
                short_id: text(short_id),
            });
        }
        Ok(stashes)
    }

    pub(crate) fn worktrees(&self) -> Result<Vec<Worktree>> {
        self.worktrees_relative_to(&self.root)
    }

    pub(crate) fn worktrees_relative_to(&self, session_root: &Path) -> Result<Vec<Worktree>> {
        let output = self.checked([
            OsString::from("worktree"),
            OsString::from("list"),
            OsString::from("--porcelain"),
            OsString::from("-z"),
        ])?;
        Ok(parse_worktrees(&output, session_root))
    }

    pub(crate) fn git_common_dir(&self) -> Result<PathBuf> {
        let output = self.checked([
            OsString::from("rev-parse"),
            OsString::from("--git-common-dir"),
        ])?;
        let raw = text(trim_ascii(&output));
        if raw.is_empty() {
            bail!("Git returned an empty common directory");
        }
        let path = Path::new(&raw);
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        Ok(fs::canonicalize(&resolved).unwrap_or(resolved))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the draw pass reads better as one top-to-bottom pass"
    )]
    pub(crate) fn perform(&self, operation: &GitOperation) -> Result<String> {
        match operation {
            GitOperation::Stage(paths) => {
                drop(self.with_paths(["add"], paths)?);
                Ok(plural_message(
                    paths.len(),
                    "change staged",
                    "changes staged",
                ))
            }
            GitOperation::StageAll => {
                drop(self.checked(strings(["add", "-A"]))?);
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
            GitOperation::Remove(paths) => {
                let removed = self.remove(paths)?;
                Ok(plural_message(removed, "file removed", "files removed"))
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
                drop(self.checked(args)?);
                Ok(if *amend {
                    "Commit amended".to_owned()
                } else {
                    "Commit created".to_owned()
                })
            }
            GitOperation::Fetch => {
                drop(self.checked(strings(["fetch", "--all", "--prune"]))?);
                Ok("Fetch complete".to_owned())
            }
            GitOperation::Pull => {
                drop(self.checked(strings(["pull"]))?);
                Ok("Pull complete".to_owned())
            }
            GitOperation::Push => {
                self.push()?;
                Ok("Push complete".to_owned())
            }
            GitOperation::Sync => {
                drop(self.checked(strings(["pull"]))?);
                self.push()?;
                Ok("Synchronization complete".to_owned())
            }
            GitOperation::Checkout(branch) => {
                drop(self.checked(strings(["switch", "--", branch]))?);
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
                drop(self.checked(args)?);
                Ok(format!("Created and switched to {name}"))
            }
            GitOperation::RenameBranch { old, new } => {
                self.validate_branch_name(new)?;
                if old == new {
                    bail!("New branch name must be different from the current name");
                }
                drop(self.checked(strings(["branch", "--move", "--", old, new]))?);
                Ok(format!("Renamed local branch {old} to {new}"))
            }
            GitOperation::DeleteBranch(branch) => {
                drop(self.checked(strings(["branch", "--delete", "--", branch]))?);
                Ok(format!("Deleted {branch}"))
            }
            GitOperation::StashPush {
                message,
                include_untracked,
                staged,
                paths,
            } => {
                let mut args = vec![OsString::from("stash"), OsString::from("push")];
                if *include_untracked {
                    args.push(OsString::from("--include-untracked"));
                }
                if *staged {
                    args.push(OsString::from("--staged"));
                }
                if !message.trim().is_empty() {
                    args.push(OsString::from("--message"));
                    args.push(OsString::from(message.trim()));
                }
                if !paths.is_empty() {
                    args.push(OsString::from("--"));
                    for path in paths {
                        args.push(path.into());
                    }
                }
                drop(self.checked(args)?);
                Ok("Changes stashed".to_owned())
            }
            GitOperation::StashApply(reference) => {
                validate_stash_reference(reference)?;
                drop(self.checked(strings(["stash", "apply", "--index", reference]))?);
                Ok(format!("Applied {reference}"))
            }
            GitOperation::StashPop(reference) => {
                let mut args = vec![
                    OsString::from("stash"),
                    OsString::from("pop"),
                    OsString::from("--index"),
                ];
                if let Some(reference) = reference {
                    validate_stash_reference(reference)?;
                    args.push(OsString::from(reference));
                }
                drop(self.checked(args)?);
                Ok(reference.as_ref().map_or_else(
                    || "Popped latest stash".to_owned(),
                    |reference| format!("Popped {reference}"),
                ))
            }
            GitOperation::StashDrop(reference) => {
                validate_stash_reference(reference)?;
                drop(self.checked(strings(["stash", "drop", reference]))?);
                Ok(format!("Dropped {reference}"))
            }
            GitOperation::StashClear => {
                drop(self.checked(strings(["stash", "clear"]))?);
                Ok("Dropped all stashes".to_owned())
            }
            GitOperation::ResolveConflict { path, choice } => {
                let side = match choice {
                    ConflictChoice::Ours => "--ours",
                    ConflictChoice::Theirs => "--theirs",
                };
                drop(self.with_paths(["checkout", side], std::slice::from_ref(path))?);
                drop(self.with_paths(["add"], std::slice::from_ref(path))?);
                Ok(format!("Accepted {side} for {}", path.to_string_lossy()))
            }
            GitOperation::CherryPick(commit) => {
                drop(self.checked(strings(["cherry-pick", commit]))?);
                Ok(format!("Cherry-picked {}", short_id(commit)))
            }
            GitOperation::Revert(commit) => {
                drop(self.checked(strings(["revert", "--no-edit", commit]))?);
                Ok(format!("Reverted {}", short_id(commit)))
            }
        }
    }

    #[expect(
        clippy::similar_names,
        reason = "the names follow the Git vocabulary they model"
    )]
    fn untracked_patch(&self, change: &Change) -> Result<(Vec<u8>, bool)> {
        let path = safe_worktree_path(&self.root, &change.path)?;
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to read {}", change.display_path()))?;
        let display_path = change.display_path();
        let binary_patch = || {
            format!(
                "diff --git a/{display_path} b/{display_path}\nnew file mode 100644\nBinary files /dev/null and b/{display_path} differ\n"
            )
            .into_bytes()
        };
        if !metadata.is_file() {
            return Ok((binary_patch(), false));
        }

        let mut contents = Vec::with_capacity(64 * 1024);
        let _ = fs::File::open(&path)
            .with_context(|| format!("failed to read {}", change.display_path()))?
            .take(MAX_DIFF_BYTES as u64 + 1)
            .read_to_end(&mut contents)
            .with_context(|| format!("failed to read {}", change.display_path()))?;
        let input_truncated = contents.len() > MAX_DIFF_BYTES;
        contents.truncate(MAX_DIFF_BYTES);
        if contents.contains(&0) {
            return Ok((binary_patch(), input_truncated));
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
        let mut patch = patch.into_bytes();
        let patch_truncated = truncate(&mut patch, MAX_DIFF_BYTES);
        Ok((patch, input_truncated || patch_truncated))
    }

    fn delete_worktree_entry(&self, relative: &Path) -> Result<()> {
        let path = safe_worktree_path(&self.root, relative)?;
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect {}", relative.display()))?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    fn remove(&self, paths: &[PathBuf]) -> Result<usize> {
        let mut wanted: Vec<PathBuf> = Vec::new();
        for path in paths {
            if !wanted.contains(path) {
                wanted.push(path.clone());
            }
        }
        if wanted.is_empty() {
            return Ok(0);
        }
        let tracked = self.tracked_paths(&wanted)?;
        for path in wanted.iter().filter(|path| !tracked.contains(*path)) {
            self.delete_worktree_entry(path)?;
        }
        if !tracked.is_empty() {
            drop(self.with_paths(["rm", "--force", "-r"], &tracked)?);
        }
        Ok(wanted.len())
    }

    fn tracked_paths(&self, paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
        let mut args: Vec<OsString> = strings(["ls-files", "-z", "--"]).to_vec();
        args.extend(paths.iter().map(|path| path.as_os_str().to_owned()));
        let output = self.checked(args)?;
        let listed: Vec<PathBuf> = output
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
            .map(|entry| PathBuf::from(text(entry)))
            .collect();
        Ok(paths
            .iter()
            .filter(|path| {
                listed
                    .iter()
                    .any(|entry| entry == *path || entry.starts_with(path))
            })
            .cloned()
            .collect())
    }

    fn discard(&self, changes: &[Change]) -> Result<()> {
        let mut restore_worktree = Vec::new();
        let mut restore_both = Vec::new();
        for change in changes {
            if change.status == ChangeStatus::Untracked {
                self.delete_worktree_entry(&change.path)?;
            } else if change.area == ChangeArea::Staged {
                restore_both.push(change.path.clone());
            } else {
                restore_worktree.push(change.path.clone());
            }
        }

        if !restore_worktree.is_empty() {
            drop(self.with_paths(["restore", "--worktree"], &restore_worktree)?);
        }
        if !restore_both.is_empty() {
            drop(self.with_paths(
                ["restore", "--staged", "--worktree", "--source=HEAD"],
                &restore_both,
            )?);
        }
        Ok(())
    }

    fn unstage(&self, paths: &[PathBuf]) -> Result<()> {
        if self.has_head() {
            drop(self.with_paths(["restore", "--staged"], paths)?);
        } else {
            drop(self.with_paths(["rm", "--cached", "--ignore-unmatch"], paths)?);
        }
        Ok(())
    }

    fn unstage_all(&self) -> Result<()> {
        if self.has_head() {
            drop(self.checked(strings(["reset", "--mixed", "--quiet", "HEAD", "--"]))?);
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
            drop(self.checked(strings(["push"]))?);
            return Ok(());
        }

        let origin = self.run(strings(["remote", "get-url", "origin"]))?;
        if !origin.status.success() {
            bail!("Current branch has no upstream and no `origin` remote exists");
        }
        drop(self.checked(strings(["push", "--set-upstream", "origin", "HEAD"]))?);
        Ok(())
    }

    fn validate_branch_name(&self, name: &str) -> Result<()> {
        if name.trim().is_empty() {
            bail!("Branch name cannot be empty");
        }
        drop(self.checked(strings(["check-ref-format", "--branch", name]))?);
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

    fn checked_bounded<I, S>(&self, args: I, limit: usize) -> Result<(Vec<u8>, bool)>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new("git");
        let _ = command
            .arg("-C")
            .arg(&self.root)
            .args(["-c", "core.quotepath=false"])
            .args(args)
            .env("LC_ALL", "C")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_TERMINAL_PROMPT", "0");
        let output = run_bounded_command(&mut command, limit, MAX_GIT_ERROR_BYTES)
            .with_context(|| format!("failed to execute Git in {}", self.root.display()))?;
        if !output.status.success() && !output.stdout_truncated {
            bail!("{}", bounded_command_error("Git command failed", &output));
        }
        Ok((output.stdout, output.stdout_truncated))
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
        let _ = command
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

fn diff_index_args(base: &str, head: &str) -> Vec<OsString> {
    vec![
        OsString::from("diff"),
        OsString::from("--name-status"),
        OsString::from("-z"),
        OsString::from("--find-renames"),
        OsString::from(base),
        OsString::from(head),
        OsString::from("--"),
    ]
}

/// Reuse an index command's own revision range for its totals by swapping the
/// listing option. This keeps the two reads describing exactly the same diff.
fn numstat_args(args: &[OsString]) -> Option<Vec<OsString>> {
    let name_status = OsStr::new("--name-status");
    args.iter().any(|arg| arg == name_status).then(|| {
        args.iter()
            .map(|arg| {
                if arg == name_status {
                    OsString::from("--numstat")
                } else {
                    arg.clone()
                }
            })
            .collect()
    })
}

fn truncate_diff_index(output: &mut Vec<u8>) -> bool {
    if output.len() <= MAX_DIFF_INDEX_BYTES {
        return false;
    }
    let boundary = output
        .get(..MAX_DIFF_INDEX_BYTES)
        .unwrap_or_default()
        .iter()
        .rposition(|byte| *byte == 0)
        .map_or(0, |index| index + 1);
    output.truncate(boundary);
    true
}

const fn diff_status_label(status: u8) -> &'static str {
    match status {
        b'A' => "added",
        b'M' => "modified",
        b'D' => "deleted",
        b'R' => "renamed",
        b'C' => "copied",
        b'T' => "type changed",
        b'U' => "unmerged",
        _ => "changed",
    }
}

fn append_diff_file_paths(args: &mut Vec<OsString>, file: &DiffFileIndexEntry) {
    if let Some(old_path) = &file.old_path {
        args.push(old_path.as_os_str().to_owned());
    }
    args.push(file.path.as_os_str().to_owned());
}

fn commit_details(commit: &Commit) -> CommitDetails {
    CommitDetails {
        id: commit.id.clone(),
        subject: commit.subject.clone(),
        author: commit.author.clone(),
        author_email: commit.author_email.clone(),
        authored_at: commit.authored_at.clone(),
        committer: commit.committer.clone(),
        committer_email: commit.committer_email.clone(),
        committed_at: commit.committed_at.clone(),
    }
}

fn validate_history_reference(reference: &str) -> Result<()> {
    if reference.starts_with("refs/heads/") || reference.starts_with("refs/remotes/") {
        Ok(())
    } else {
        bail!("refusing to compare an invalid branch reference")
    }
}

fn valid_stash_reference(reference: &str) -> bool {
    reference
        .strip_prefix("stash@{")
        .and_then(|value| value.strip_suffix('}'))
        .is_some_and(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
}

fn validate_stash_reference(reference: &str) -> Result<()> {
    if valid_stash_reference(reference) {
        Ok(())
    } else {
        bail!("refusing to use an invalid stash reference")
    }
}

fn parse_stash_subject(subject: &str) -> (String, String) {
    let subject = subject.trim();
    for prefix in ["WIP on ", "On "] {
        if let Some(rest) = subject.strip_prefix(prefix)
            && let Some((branch, message)) = rest.split_once(": ")
        {
            return (branch.to_owned(), message.to_owned());
        }
    }
    (String::new(), subject.to_owned())
}

fn parse_worktrees(output: &[u8], session_root: &Path) -> Vec<Worktree> {
    let mut worktrees = Vec::new();
    let mut fields = Vec::new();
    for field in output.split(|byte| *byte == 0) {
        if field.is_empty() {
            if !fields.is_empty() {
                if let Some(worktree) = worktree_from_fields(&fields, session_root) {
                    worktrees.push(worktree);
                }
                fields.clear();
            }
            continue;
        }
        fields.push(field);
    }
    if !fields.is_empty()
        && let Some(worktree) = worktree_from_fields(&fields, session_root)
    {
        worktrees.push(worktree);
    }
    worktrees
}

fn worktree_from_fields(fields: &[&[u8]], session_root: &Path) -> Option<Worktree> {
    let mut path = None;
    let mut head = String::new();
    let mut branch = None;
    let mut bare = false;
    let mut detached = false;
    let mut locked = None;
    let mut prunable = None;
    for field in fields {
        if let Some(value) = field.strip_prefix(b"worktree ") {
            path = Some(parse_worktree_path(value));
        } else if let Some(value) = field.strip_prefix(b"HEAD ") {
            head = text(value);
        } else if let Some(value) = field.strip_prefix(b"branch ") {
            branch = Some(heads_branch_name(&text(value)));
        } else if *field == b"detached" {
            detached = true;
        } else if *field == b"bare" {
            bare = true;
        } else if let Some(value) = field.strip_prefix(b"locked") {
            locked = Some(text(value).trim().to_owned());
        } else if let Some(value) = field.strip_prefix(b"prunable") {
            prunable = Some(text(value).trim().to_owned());
        }
    }
    let path = path?;
    Some(Worktree {
        current: same_path(&path, session_root),
        path,
        head,
        branch,
        bare,
        detached,
        locked,
        prunable,
    })
}

fn heads_branch_name(reference: &str) -> String {
    reference
        .strip_prefix("refs/heads/")
        .unwrap_or(reference)
        .to_owned()
}

fn parse_worktree_path(value: &[u8]) -> PathBuf {
    let rendered = text(value);
    #[cfg(windows)]
    {
        PathBuf::from(rendered.replace('/', "\\"))
    }
    #[cfg(not(windows))]
    {
        PathBuf::from(rendered)
    }
}

pub(crate) fn same_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(first), Ok(second)) => first == second,
        _ => false,
    }
}

fn strings<const N: usize>(values: [&str; N]) -> [OsString; N] {
    values.map(OsString::from)
}

fn command_error(context: &str, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let details = if stderr.is_empty() { stdout } else { stderr };
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
    truncate_to_complete_line(bytes);
    true
}

fn truncate_to_complete_line(bytes: &mut Vec<u8>) {
    while bytes.last().is_some_and(|byte| *byte != b'\n') {
        let _ = bytes.pop();
    }
}

const fn trim_ascii(mut value: &[u8]) -> &[u8] {
    while let Some((first, rest)) = value.split_first() {
        if !first.is_ascii_whitespace() {
            break;
        }
        value = rest;
    }
    while let Some((last, rest)) = value.split_last() {
        if !last.is_ascii_whitespace() {
            break;
        }
        value = rest;
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

fn is_full_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn short_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests;
