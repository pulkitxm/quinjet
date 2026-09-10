#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Subcommand)]
pub(super) enum PrVerb {
    #[doc = " Print a pull request's metadata and description"]
    View(PrWatchArgs),
    #[doc = " List the files a pull request changes"]
    Files(PrArgs),
    #[doc = " List a pull request's commits"]
    Commits(PrArgs),
    #[doc = " Print a pull request's patch"]
    Diff(PrDiffArgs),
    #[doc = " Print a pull request's timeline and review comments"]
    Conversation(PrWatchArgs),
    #[doc = " List a pull request's checks"]
    Checks(PrChecksArgs),
    #[doc = " Explain whether a pull request can merge"]
    Gate(PrGateArgs),
    #[doc = " Print one check run's steps and log"]
    Logs(PrLogsArgs),
    #[doc = " Open a pull request in a browser"]
    Open(PrOpenArgs),
    #[doc = " Merge a pull request"]
    Merge(PrMergeArgs),
    #[doc = " Merge despite branch protections"]
    AdminMerge(PrMergeArgs),
    #[doc = " Merge automatically after requirements pass"]
    AutoMerge(PrMergeArgs),
    #[doc = " Disable automatic merging"]
    DisableAutoMerge(PrMutateArgs),
    #[doc = " Remove a pull request from its merge queue"]
    Dequeue(PrMutateArgs),
    #[doc = " Mark a draft pull request ready for review"]
    Ready(PrMutateArgs),
    #[doc = " Convert an open pull request to a draft"]
    Draft(PrMutateArgs),
    #[doc = " Submit an approval, comment, or change request"]
    Review(PrReviewArgs),
    #[doc = " Add a conversation comment"]
    Comment(PrTextMutateArgs),
    #[doc = " Edit your latest conversation comment"]
    EditLastComment(PrTextMutateArgs),
    #[doc = " Delete your latest conversation comment"]
    DeleteLastComment(PrMutateArgs),
    #[doc = " Edit pull-request metadata"]
    Edit(PrEditArgs),
    #[doc = " Bring the head branch up to date with its base"]
    UpdateBranch(PrUpdateBranchArgs),
    #[doc = " Lock the conversation"]
    Lock(PrLockArgs),
    #[doc = " Unlock the conversation"]
    Unlock(PrMutateArgs),
    #[doc = " Subscribe to notifications"]
    Subscribe(PrMutateArgs),
    #[doc = " Unsubscribe from notifications"]
    Unsubscribe(PrMutateArgs),
    #[doc = " Allow maintainers to edit a fork's head branch"]
    AllowMaintainerEdits(PrMutateArgs),
    #[doc = " Prevent maintainers from editing a fork's head branch"]
    DisallowMaintainerEdits(PrMutateArgs),
    #[doc = " Create a pull request that reverts a merged pull request"]
    Revert(PrRevertArgs),
    #[doc = " Close a pull request"]
    Close(PrMutateArgs),
    #[doc = " Reopen a closed pull request"]
    Reopen(PrMutateArgs),
    #[doc = " Read or write pull-request review threads"]
    Reviews {
        #[command(subcommand)]
        command: PrReviewVerb,
    },
}
