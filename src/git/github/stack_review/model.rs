#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " An approval that no longer covers what the pull request now says,"]
#[doc = " because a commit arrived after it. Naming the reviewer matters: this"]
#[doc = " is a request to go back to a person, not a state to wait out."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StaleApproval {
    pub reviewer: String,
    #[doc = " The commit the approval was given on."]
    pub approved_oid: String,
    pub head_oid: String,
}

#[doc = " Why a member is not mergeable. `Own` means the member's own gate says"]
#[doc = " so; `Downstream` means the member is clear and is only waiting for"]
#[doc = " something lower in the stack."]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum StackBlockSource {
    None,
    Own,
    Downstream,
}

impl StackBlockSource {
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::None => "clear",
            Self::Own => "own",
            Self::Downstream => "downstream",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StackReviewMember {
    pub position: usize,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub selected: bool,
    pub verdict: MergeGateVerdict,
    #[doc = " Where the block comes from, which is the difference between work to"]
    #[doc = " do here and waiting for a layer below."]
    pub block_source: StackBlockSource,
    #[doc = " The member's own blockers, in the gate's order of actionability."]
    pub blockers: Vec<String>,
    pub head_oid: String,
    #[doc = " Additions, deletions and files measured against this member's own"]
    #[doc = " base, which is the parent's head rather than the trunk."]
    pub additions: usize,
    pub deletions: usize,
    pub changed_files: usize,
    pub stale_approvals: Vec<StaleApproval>,
    pub unresolved_threads: usize,
    pub failing_checks: Vec<String>,
    #[doc = " The paths this member changes on its own, bounded."]
    pub paths: Vec<PathBuf>,
    pub paths_truncated: bool,
}

impl StackReviewMember {
    pub(crate) const fn is_clear(&self) -> bool {
        matches!(
            self.verdict,
            MergeGateVerdict::Mergeable | MergeGateVerdict::Merged
        )
    }

    pub(crate) fn headline(&self) -> String {
        self.blockers
            .first()
            .cloned()
            .unwrap_or_else(|| self.verdict.word().to_owned())
    }
}

#[doc = " A path more than one member of the stack changes. Two members editing"]
#[doc = " the same file is where a rebase conflict comes from, and it is invisible"]
#[doc = " in any single pull request's diff."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DuplicatedPath {
    pub path: PathBuf,
    pub positions: Vec<usize>,
}

#[doc = " The failing check that comes first in merge order, which is the one"]
#[doc = " worth looking at: everything above it is waiting for it either way."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StackCheckFailure {
    pub position: usize,
    pub number: u64,
    pub check: String,
    pub required: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StackReview {
    pub schema_version: u8,
    pub number: u64,
    pub base_ref: String,
    pub size: usize,
    pub selected_position: usize,
    pub members: Vec<StackReviewMember>,
    #[doc = " The positions that can merge right now, lowest first, stopping at"]
    #[doc = " the first that cannot. Merging past a blocked layer is not safe."]
    pub merge_order: Vec<usize>,
    #[doc = " The lowest blocked position and everything above it, which is what"]
    #[doc = " that one member is holding up."]
    pub critical_path: Vec<usize>,
    pub critical_position: Option<usize>,
    #[doc = " Positions whose own gate is clear and which are waiting only on a"]
    #[doc = " layer below."]
    pub downstream_blocked: Vec<usize>,
    pub earliest_failing_check: Option<StackCheckFailure>,
    pub duplicated_paths: Vec<DuplicatedPath>,
    pub stale_approvals: usize,
    pub unresolved_threads: usize,
    pub truncated: bool,
    pub warnings: Vec<String>,
}

impl StackReview {
    pub(crate) const SCHEMA_VERSION: u8 = 1;

    pub(crate) fn critical_member(&self) -> Option<&StackReviewMember> {
        let position = self.critical_position?;
        self.members
            .iter()
            .find(|member| member.position == position)
    }

    #[doc = " Whether the whole stack could merge bottom to top right now."]
    pub(crate) const fn is_clear(&self) -> bool {
        self.critical_position.is_none() && !self.members.is_empty()
    }
}

#[doc = " One member's share of the stack-wide feedback queue."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StackFeedbackMember {
    pub position: usize,
    pub number: u64,
    pub title: String,
    pub selected: bool,
    pub items: Vec<FeedbackItem>,
    pub counts: FeedbackCounts,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StackFeedback {
    pub schema_version: u8,
    pub number: u64,
    pub size: usize,
    pub selected_position: usize,
    pub viewer: String,
    pub members: Vec<StackFeedbackMember>,
    pub counts: FeedbackCounts,
    #[doc = " The lowest position carrying something blocking, which is where the"]
    #[doc = " stack unblocks from."]
    pub next_position: Option<usize>,
    pub truncated: bool,
    pub warnings: Vec<String>,
}

impl StackFeedback {
    pub(crate) const SCHEMA_VERSION: u8 = 1;

    pub(crate) fn next_blocker(&self) -> Option<(usize, &FeedbackItem)> {
        let position = self.next_position?;
        let member = self
            .members
            .iter()
            .find(|member| member.position == position)?;
        member
            .items
            .iter()
            .find(|item| item.kind.is_blocking())
            .map(|item| (position, item))
    }
}
