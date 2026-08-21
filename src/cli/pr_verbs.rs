#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Subcommand)]
pub(super) enum PrVerb {
    /// Print a pull request's metadata and description
    View(PrWatchArgs),
    /// List the files a pull request changes
    Files(PrArgs),
    /// Print a pull request's patch
    Diff(PrDiffArgs),
    /// Print a pull request's timeline and review comments
    Conversation(PrWatchArgs),
    /// List a pull request's checks
    Checks(PrChecksArgs),
    /// Print one check run's steps and log
    Logs(PrLogsArgs),
    /// Open a pull request in a browser
    Open(PrOpenArgs),
    /// Merge a pull request
    Merge(PrMergeArgs),
    /// Merge despite branch protections
    AdminMerge(PrMergeArgs),
    /// Merge automatically after requirements pass
    AutoMerge(PrMergeArgs),
    /// Disable automatic merging
    DisableAutoMerge(PrMutateArgs),
    /// Remove a pull request from its merge queue
    Dequeue(PrMutateArgs),
    /// Mark a draft pull request ready for review
    Ready(PrMutateArgs),
    /// Convert an open pull request to a draft
    Draft(PrMutateArgs),
    /// Submit an approval, comment, or change request
    Review(PrReviewArgs),
    /// Add a conversation comment
    Comment(PrTextMutateArgs),
    /// Edit your latest conversation comment
    EditLastComment(PrTextMutateArgs),
    /// Delete your latest conversation comment
    DeleteLastComment(PrMutateArgs),
    /// Edit pull-request metadata
    Edit(PrEditArgs),
    /// Bring the head branch up to date with its base
    UpdateBranch(PrUpdateBranchArgs),
    /// Lock the conversation
    Lock(PrLockArgs),
    /// Unlock the conversation
    Unlock(PrMutateArgs),
    /// Subscribe to notifications
    Subscribe(PrMutateArgs),
    /// Unsubscribe from notifications
    Unsubscribe(PrMutateArgs),
    /// Allow maintainers to edit a fork's head branch
    AllowMaintainerEdits(PrMutateArgs),
    /// Prevent maintainers from editing a fork's head branch
    DisallowMaintainerEdits(PrMutateArgs),
    /// Create a pull request that reverts a merged pull request
    Revert(PrRevertArgs),
    /// Close a pull request
    Close(PrMutateArgs),
    /// Reopen a closed pull request
    Reopen(PrMutateArgs),
    /// Read or write pull-request review threads
    Reviews {
        #[command(subcommand)]
        command: PrReviewVerb,
    },
}
