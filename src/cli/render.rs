use crate::date_time::{format_local_timestamp, format_relative_timestamp};
use crate::git::diff::{DiffDocument, DiffLineKind};
use crate::git::github::{
    CheckRunLog, CheckStep, ConversationKind, GitHubRepository, MergeGate, MergeGateBranch,
    MergeGateChecks, MergeGateReview, PullRequest, PullRequestCheck, PullRequestCheckStatus,
    PullRequestCommits, PullRequestConversation, PullRequestDiffIndex, PullRequestFileStatus,
    PullRequestReviewSide, PullRequestReviewSnapshot, PullRequestReviewThreadSubject,
    PullRequestStackSnapshot, ReviewFileProgress, ReviewNextStep, ReviewProgress, StackGate,
    unix_now,
};
use crate::git::history::Commit;
use crate::git::status::{ChangeArea, RepoStatus};
use crate::git::{Branch, HistoryBranch, ProjectGroup, Stash, Worktree};

#[derive(Default)]
struct Report(String);

impl Report {
    fn line(&mut self, text: &str) {
        self.0.push_str(text);
        self.0.push('\n');
    }

    fn blank(&mut self) {
        self.0.push('\n');
    }

    const fn empty(&self) -> bool {
        self.0.is_empty()
    }

    fn finish(self) -> String {
        self.0
    }
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let kept: String = text.chars().take(width.saturating_sub(1)).collect();
    format!("{kept}…")
}

mod gate;
mod github;
mod progress;
mod repository;

pub(crate) use gate::*;
pub(crate) use github::*;
pub(crate) use progress::*;
pub(crate) use repository::*;

#[cfg(test)]
mod tests;
