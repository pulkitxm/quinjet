#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RemoteUrl {
    pub(super) remote: String,
    pub(super) url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CacheDisposition {
    Network,
    Fresh,
    Stale,
}

#[doc = " How long an entry stays usable. `Immutable` is for content whose identity is"]
#[doc = " already in its key: a finished run's log, or a patch between two fixed"]
#[doc = " commits. Such an entry can never become wrong, only evicted."]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CacheLife {
    Immutable,
    Ttl(Duration),
}

impl CacheLife {
    pub(super) fn accepts(self, age: Duration) -> bool {
        match self {
            Self::Immutable => true,
            Self::Ttl(ttl) => age <= ttl,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PullRequestProgress {
    LoadingMetadata,
    PreparingRepository,
    FetchingBase,
    FetchingHead,
    FindingMergeBase,
    EnumeratingFiles,
}

impl PullRequestProgress {
    pub(crate) const fn percent(self) -> u16 {
        match self {
            Self::LoadingMetadata => 10,
            Self::PreparingRepository => 20,
            Self::FetchingBase => 35,
            Self::FetchingHead => 50,
            Self::FindingMergeBase => 65,
            Self::EnumeratingFiles => 90,
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::LoadingMetadata => "Fetching pull-request metadata",
            Self::PreparingRepository => "Preparing an isolated diff workspace",
            Self::FetchingBase => "Fetching the destination commit",
            Self::FetchingHead => "Fetching the source commit",
            Self::FindingMergeBase => "Finding the merge base",
            Self::EnumeratingFiles => "Enumerating changed files",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum PullRequestMergeMethod {
    Merge,
    #[default]
    Squash,
    Rebase,
}

impl PullRequestMergeMethod {
    pub(crate) const ALL: [Self; 3] = [Self::Merge, Self::Squash, Self::Rebase];

    pub(crate) const fn flag(self) -> &'static str {
        match self {
            Self::Merge => "--merge",
            Self::Squash => "--squash",
            Self::Rebase => "--rebase",
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Merge => "Create a merge commit",
            Self::Squash => "Squash and merge",
            Self::Rebase => "Rebase and merge",
        }
    }

    pub(crate) const fn preview_verb(self) -> &'static str {
        match self {
            Self::Merge => "create a merge commit for",
            Self::Squash => "squash and merge",
            Self::Rebase => "rebase and merge",
        }
    }

    pub(crate) const fn done_verb(self) -> &'static str {
        match self {
            Self::Merge => "Merged",
            Self::Squash => "Squashed and merged",
            Self::Rebase => "Rebased and merged",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum PullRequestMergeMode {
    #[default]
    Direct,
    Auto,
    Admin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PullRequestReviewKind {
    Approve,
    Comment,
    RequestChanges,
}

impl PullRequestReviewKind {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Approve => "Approve pull request",
            Self::Comment => "Submit review comment",
            Self::RequestChanges => "Request changes",
        }
    }

    pub(super) const fn flag(self) -> &'static str {
        match self {
            Self::Approve => "--approve",
            Self::Comment => "--comment",
            Self::RequestChanges => "--request-changes",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PullRequestCommentMode {
    Create,
    EditLast,
    DeleteLast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PullRequestUpdateMethod {
    Merge,
    Rebase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PullRequestLockReason {
    OffTopic,
    Resolved,
    Spam,
    TooHeated,
}

impl PullRequestLockReason {
    pub(super) const fn flag(self) -> &'static str {
        match self {
            Self::OffTopic => "off_topic",
            Self::Resolved => "resolved",
            Self::Spam => "spam",
            Self::TooHeated => "too_heated",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PullRequestEdit {
    Title(String),
    Body(String),
    Base(String),
    AddAssignee(String),
    RemoveAssignee(String),
    AddLabel(String),
    RemoveLabel(String),
    AddProject(String),
    RemoveProject(String),
    AddReviewer(String),
    RemoveReviewer(String),
    SetMilestone(String),
    RemoveMilestone,
}

impl PullRequestEdit {
    pub(crate) const fn label(&self) -> &'static str {
        match self {
            Self::Title(_) => "Edit pull-request title",
            Self::Body(_) => "Edit pull-request description",
            Self::Base(_) => "Change base branch",
            Self::AddAssignee(_) => "Add assignees",
            Self::RemoveAssignee(_) => "Remove assignees",
            Self::AddLabel(_) => "Add labels",
            Self::RemoveLabel(_) => "Remove labels",
            Self::AddProject(_) => "Add to projects",
            Self::RemoveProject(_) => "Remove from projects",
            Self::AddReviewer(_) => "Request reviewers",
            Self::RemoveReviewer(_) => "Remove review requests",
            Self::SetMilestone(_) => "Set milestone",
            Self::RemoveMilestone => "Remove milestone",
        }
    }

    pub(super) fn append_args(&self, args: &mut Vec<OsString>) {
        let (flag, value) = match self {
            Self::Title(value) => ("--title", Some(value)),
            Self::Body(value) => ("--body", Some(value)),
            Self::Base(value) => ("--base", Some(value)),
            Self::AddAssignee(value) => ("--add-assignee", Some(value)),
            Self::RemoveAssignee(value) => ("--remove-assignee", Some(value)),
            Self::AddLabel(value) => ("--add-label", Some(value)),
            Self::RemoveLabel(value) => ("--remove-label", Some(value)),
            Self::AddProject(value) => ("--add-project", Some(value)),
            Self::RemoveProject(value) => ("--remove-project", Some(value)),
            Self::AddReviewer(value) => ("--add-reviewer", Some(value)),
            Self::RemoveReviewer(value) => ("--remove-reviewer", Some(value)),
            Self::SetMilestone(value) => ("--milestone", Some(value)),
            Self::RemoveMilestone => ("--remove-milestone", None),
        };
        args.push(OsString::from(flag));
        if let Some(value) = value {
            args.push(OsString::from(value));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PullRequestOperation {
    Merge {
        method: PullRequestMergeMethod,
        mode: PullRequestMergeMode,
        delete_branch: bool,
    },
    SetDraft(bool),
    Review {
        kind: PullRequestReviewKind,
        body: String,
    },
    Comment {
        mode: PullRequestCommentMode,
        body: String,
    },
    Edit(PullRequestEdit),
    UpdateBranch(PullRequestUpdateMethod),
    DisableAutoMerge,
    Dequeue,
    Lock(Option<PullRequestLockReason>),
    Unlock,
    Subscribe(bool),
    SetMaintainerEdits(bool),
    Revert {
        draft: bool,
        title: String,
        body: String,
    },
    Close,
    Reopen,
}

impl PullRequestOperation {
    pub(crate) const fn label(&self) -> &'static str {
        match self {
            Self::Merge {
                method,
                mode: PullRequestMergeMode::Direct,
                ..
            } => method.label(),
            Self::Merge {
                mode: PullRequestMergeMode::Auto,
                ..
            } => "Enable auto-merge",
            Self::Merge {
                mode: PullRequestMergeMode::Admin,
                ..
            } => "Merge with administrator privileges",
            Self::SetDraft(true) => "Convert to draft",
            Self::SetDraft(false) => "Mark ready for review",
            Self::Review { kind, .. } => kind.label(),
            Self::Comment {
                mode: PullRequestCommentMode::Create,
                ..
            } => "Comment on pull request",
            Self::Comment {
                mode: PullRequestCommentMode::EditLast,
                ..
            } => "Edit last comment",
            Self::Comment {
                mode: PullRequestCommentMode::DeleteLast,
                ..
            } => "Delete last comment",
            Self::Edit(edit) => edit.label(),
            Self::UpdateBranch(PullRequestUpdateMethod::Merge) => "Update branch with merge",
            Self::UpdateBranch(PullRequestUpdateMethod::Rebase) => "Update branch with rebase",
            Self::DisableAutoMerge => "Disable auto-merge",
            Self::Dequeue => "Remove from merge queue",
            Self::Lock(_) => "Lock conversation",
            Self::Unlock => "Unlock conversation",
            Self::Subscribe(true) => "Subscribe to pull request",
            Self::Subscribe(false) => "Unsubscribe from pull request",
            Self::SetMaintainerEdits(true) => "Allow maintainer edits",
            Self::SetMaintainerEdits(false) => "Disallow maintainer edits",
            Self::Revert { .. } => "Create revert pull request",
            Self::Close => "Close pull request",
            Self::Reopen => "Reopen pull request",
        }
    }

    pub(crate) fn confirm_title(&self) -> String {
        let mut title = self.label().to_owned();
        title.push('?');
        title
    }

    pub(crate) fn confirm_message(&self, pull_request: &PullRequest) -> String {
        let mut message = match self {
            Self::Merge { method, mode, .. } => {
                let mut text = String::from("Really ");
                if *mode == PullRequestMergeMode::Auto {
                    text.push_str("enable auto-merge to ");
                } else if *mode == PullRequestMergeMode::Admin {
                    text.push_str("use administrator privileges to ");
                }
                text.push_str(method.preview_verb());
                text
            }
            Self::SetDraft(true) => String::from("Really convert to draft"),
            Self::SetDraft(false) => String::from("Really mark ready for review"),
            Self::Review { kind, .. } => format!("Really {}", kind.label().to_lowercase()),
            Self::Comment { .. }
            | Self::Edit(_)
            | Self::UpdateBranch(_)
            | Self::DisableAutoMerge
            | Self::Dequeue
            | Self::Lock(_)
            | Self::Unlock
            | Self::Subscribe(_)
            | Self::SetMaintainerEdits(_)
            | Self::Revert { .. } => format!("Really {}", self.label().to_lowercase()),
            Self::Close => String::from("Really close"),
            Self::Reopen => String::from("Really reopen"),
        };
        message.push_str(" #");
        message.push_str(&pull_request.number.to_string());
        message.push_str(" (");
        message.push_str(&pull_request.title);
        message.push_str(")?");
        message
    }

    pub(crate) fn success_message(&self, pull_request: &PullRequest) -> String {
        let mut message = match self {
            Self::Merge {
                method,
                mode: PullRequestMergeMode::Direct,
                ..
            } => method.done_verb().to_owned(),
            Self::Merge {
                mode: PullRequestMergeMode::Auto,
                ..
            } => String::from("Enabled auto-merge for"),
            Self::Merge {
                mode: PullRequestMergeMode::Admin,
                ..
            } => String::from("Administrator-merged"),
            Self::SetDraft(true) => String::from("Converted to draft"),
            Self::SetDraft(false) => String::from("Marked ready for review"),
            Self::Review { kind, .. } => match kind {
                PullRequestReviewKind::Approve => String::from("Approved"),
                PullRequestReviewKind::Comment => String::from("Reviewed"),
                PullRequestReviewKind::RequestChanges => String::from("Requested changes on"),
            },
            Self::Comment {
                mode: PullRequestCommentMode::Create,
                ..
            } => String::from("Commented on"),
            Self::Comment {
                mode: PullRequestCommentMode::EditLast,
                ..
            } => String::from("Edited the last comment on"),
            Self::Comment {
                mode: PullRequestCommentMode::DeleteLast,
                ..
            } => String::from("Deleted the last comment on"),
            Self::Edit(_) => String::from("Updated"),
            Self::UpdateBranch(_) => String::from("Updated the branch for"),
            Self::DisableAutoMerge => String::from("Disabled auto-merge for"),
            Self::Dequeue => String::from("Removed from the merge queue"),
            Self::Lock(_) => String::from("Locked the conversation on"),
            Self::Unlock => String::from("Unlocked the conversation on"),
            Self::Subscribe(true) => String::from("Subscribed to"),
            Self::Subscribe(false) => String::from("Unsubscribed from"),
            Self::SetMaintainerEdits(true) => String::from("Allowed maintainer edits on"),
            Self::SetMaintainerEdits(false) => String::from("Disallowed maintainer edits on"),
            Self::Revert { .. } => String::from("Created a revert for"),
            Self::Close => String::from("Closed"),
            Self::Reopen => String::from("Reopened"),
        };
        message.push_str(" #");
        message.push_str(&pull_request.number.to_string());
        message
    }
}
