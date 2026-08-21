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

#[doc = " A local or remote-tracking branch that can be inspected without changing HEAD."]
#[doc = " `reference` is always a full ref emitted by Git and is used only as a revision."]
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
    github_cli: Option<PathBuf>,
}

mod local_diff;
mod operations;
mod reads;
mod repository;
pub(crate) mod support;

#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use support::*;

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
pub(crate) mod tests;
