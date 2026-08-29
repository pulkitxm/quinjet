use crate::date_time::{format_local_timestamp, format_relative_timestamp};
use crate::git::diff::{DiffDocument, DiffLineKind};
use crate::git::github::{
    AnnotationCounts, AnnotationGrouping, CheckAnnotation, CheckRunLog, CheckStep,
    CodeScanningAlert, ContextSection, ConversationKind, DependencyDelta, DependencyVulnerability,
    FeedbackItem, GitHubRepository, MergeGate, MergeGateBranch, MergeGateChecks, MergeGateReview,
    PullRequest, PullRequestAnnotations, PullRequestArtifacts, PullRequestCheck,
    PullRequestCheckStatus, PullRequestCommits, PullRequestContext, PullRequestConversation,
    PullRequestDependencies, PullRequestDeployments, PullRequestDiffIndex, PullRequestFeedback,
    PullRequestFileStatus, PullRequestReviewSide, PullRequestReviewSnapshot,
    PullRequestReviewThreadSubject, PullRequestSecurity, PullRequestStackSnapshot,
    PullRequestSuggestions, PullRequestWorkflowRuns, ReviewFileProgress, ReviewNextStep,
    ReviewProgress, StackGate, SuggestionBlocker, unix_now,
};
use crate::git::history::Commit;
use crate::git::status::{ChangeArea, RepoStatus};
use crate::git::work::{WorkDiff, WorkPublishPlan, WorkSession, WorkSessions};
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

#[doc = " Enough of an object name to recognize, which is what a person reads"]
#[doc = " out of a listing. The whole name is always in the JSON."]
fn short_oid(oid: &str) -> String {
    oid.chars().take(12).collect()
}

mod actions;
mod annotations;
mod context;
mod feedback;
mod gate;
mod github;
mod progress;
mod repository;
mod work;

pub(crate) use actions::*;
pub(crate) use annotations::*;
pub(crate) use context::*;
pub(crate) use feedback::*;
pub(crate) use gate::*;
pub(crate) use github::*;
pub(crate) use progress::*;
pub(crate) use repository::*;
pub(crate) use work::*;

#[cfg(test)]
mod tests;
