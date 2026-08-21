#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrMenuItem {
    Merge(PullRequestMergeMethod),
    Stage,
    AutoMerge,
    DisableAutoMerge,
    Dequeue,
    AdminMerge,
    Review,
    Comments,
    Edit,
    UpdateBranch,
    Lock,
    Unlock,
    Subscribe,
    Unsubscribe,
    AllowMaintainerEdits,
    DisallowMaintainerEdits,
    Revert,
    Close,
    OpenInBrowser,
}

impl PrMenuItem {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Merge(method) => method.label(),
            Self::Stage => "Change review stage…",
            Self::AutoMerge => "Enable auto-merge…",
            Self::DisableAutoMerge => "Disable auto-merge",
            Self::Dequeue => "Remove from merge queue",
            Self::AdminMerge => "Merge as administrator…",
            Self::Review => "Submit review…",
            Self::Comments => "Manage comments…",
            Self::Edit => "Edit metadata…",
            Self::UpdateBranch => "Update branch…",
            Self::Lock => "Lock conversation…",
            Self::Unlock => "Unlock conversation",
            Self::Subscribe => "Subscribe",
            Self::Unsubscribe => "Unsubscribe",
            Self::AllowMaintainerEdits => "Allow maintainer edits",
            Self::DisallowMaintainerEdits => "Disallow maintainer edits",
            Self::Revert => "Create revert pull request…",
            Self::Close => "Close pull request",
            Self::OpenInBrowser => "Open in browser",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrPrimaryAction {
    Merge(PullRequestMergeMethod),
    Ready,
    Dequeue,
    DisableAutoMerge,
    Reopen,
    OpenInBrowser,
}

impl PrPrimaryAction {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Merge(method) => method.label(),
            Self::Ready => "Ready for review",
            Self::Dequeue => "Remove from merge queue",
            Self::DisableAutoMerge => "Disable auto-merge",
            Self::Reopen => "Reopen pull request",
            Self::OpenInBrowser => "Open in browser",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PullRequestEditField {
    Title,
    Body,
    Base,
    AddAssignee,
    RemoveAssignee,
    AddLabel,
    RemoveLabel,
    AddProject,
    RemoveProject,
    AddReviewer,
    RemoveReviewer,
    Milestone,
    RemoveMilestone,
}

impl PullRequestEditField {
    pub(crate) const ALL: [Self; 13] = [
        Self::Title,
        Self::Body,
        Self::Base,
        Self::AddAssignee,
        Self::RemoveAssignee,
        Self::AddLabel,
        Self::RemoveLabel,
        Self::AddProject,
        Self::RemoveProject,
        Self::AddReviewer,
        Self::RemoveReviewer,
        Self::Milestone,
        Self::RemoveMilestone,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Title => "Edit title",
            Self::Body => "Edit description",
            Self::Base => "Change base branch",
            Self::AddAssignee => "Add assignees",
            Self::RemoveAssignee => "Remove assignees",
            Self::AddLabel => "Add labels",
            Self::RemoveLabel => "Remove labels",
            Self::AddProject => "Add to projects",
            Self::RemoveProject => "Remove from projects",
            Self::AddReviewer => "Request reviewers",
            Self::RemoveReviewer => "Remove review requests",
            Self::Milestone => "Set milestone",
            Self::RemoveMilestone => "Remove milestone",
        }
    }

    pub(crate) fn edit(self, value: String) -> PullRequestEdit {
        match self {
            Self::Title => PullRequestEdit::Title(value),
            Self::Body => PullRequestEdit::Body(value),
            Self::Base => PullRequestEdit::Base(value),
            Self::AddAssignee => PullRequestEdit::AddAssignee(value),
            Self::RemoveAssignee => PullRequestEdit::RemoveAssignee(value),
            Self::AddLabel => PullRequestEdit::AddLabel(value),
            Self::RemoveLabel => PullRequestEdit::RemoveLabel(value),
            Self::AddProject => PullRequestEdit::AddProject(value),
            Self::RemoveProject => PullRequestEdit::RemoveProject(value),
            Self::AddReviewer => PullRequestEdit::AddReviewer(value),
            Self::RemoveReviewer => PullRequestEdit::RemoveReviewer(value),
            Self::Milestone => PullRequestEdit::SetMilestone(value),
            Self::RemoveMilestone => PullRequestEdit::RemoveMilestone,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrActionItem {
    AutoMerge(PullRequestMergeMethod),
    AdminMerge(PullRequestMergeMethod),
    Review(PullRequestReviewKind),
    Comment(PullRequestCommentMode),
    Edit(PullRequestEditField),
    UpdateBranch(PullRequestUpdateMethod),
    Lock(Option<PullRequestLockReason>),
    Revert(bool),
}

impl PrActionItem {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::AutoMerge(method) | Self::AdminMerge(method) => method.label(),
            Self::Review(kind) => kind.label(),
            Self::Comment(PullRequestCommentMode::Create) => "Add comment",
            Self::Comment(PullRequestCommentMode::EditLast) => "Edit my latest comment",
            Self::Comment(PullRequestCommentMode::DeleteLast) => "Delete my latest comment",
            Self::Edit(field) => field.label(),
            Self::UpdateBranch(PullRequestUpdateMethod::Merge) => "Merge base into head",
            Self::UpdateBranch(PullRequestUpdateMethod::Rebase) => "Rebase head onto base",
            Self::Lock(None) => "Lock without a reason",
            Self::Lock(Some(PullRequestLockReason::OffTopic)) => "Lock as off topic",
            Self::Lock(Some(PullRequestLockReason::Resolved)) => "Lock as resolved",
            Self::Lock(Some(PullRequestLockReason::Spam)) => "Lock as spam",
            Self::Lock(Some(PullRequestLockReason::TooHeated)) => "Lock as too heated",
            Self::Revert(false) => "Create ready revert pull request",
            Self::Revert(true) => "Create draft revert pull request",
        }
    }

    pub(crate) const fn needs_input(self) -> bool {
        matches!(
            self,
            Self::Review(PullRequestReviewKind::Comment | PullRequestReviewKind::RequestChanges)
                | Self::Comment(PullRequestCommentMode::Create | PullRequestCommentMode::EditLast)
                | Self::Edit(
                    PullRequestEditField::Title
                        | PullRequestEditField::Body
                        | PullRequestEditField::Base
                        | PullRequestEditField::AddAssignee
                        | PullRequestEditField::RemoveAssignee
                        | PullRequestEditField::AddLabel
                        | PullRequestEditField::RemoveLabel
                        | PullRequestEditField::AddProject
                        | PullRequestEditField::RemoveProject
                        | PullRequestEditField::AddReviewer
                        | PullRequestEditField::RemoveReviewer
                        | PullRequestEditField::Milestone
                )
        )
    }
}
