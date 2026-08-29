#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " The one answer `quinjet pr gate` exists to give."]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum MergeGateVerdict {
    Mergeable,
    Blocked,
    Merged,
    Closed,
    Unknown,
}

impl MergeGateVerdict {
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Mergeable => "mergeable",
            Self::Blocked => "blocked",
            Self::Merged => "merged",
            Self::Closed => "closed",
            Self::Unknown => "unknown",
        }
    }

    #[doc = " 0 when nothing stands in the way, 1 when something does, and"]
    #[doc = " `EXIT_UNAVAILABLE` when GitHub has not decided yet. A script can branch"]
    #[doc = " on the code alone; the JSON explains it."]
    pub(crate) const fn exit_code(self) -> u8 {
        match self {
            Self::Mergeable | Self::Merged => 0,
            Self::Blocked | Self::Closed => 1,
            Self::Unknown => 4,
        }
    }

    pub(crate) const fn is_settled(self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

#[doc = " Which requirement a blocker belongs to. The kinds are ordered by how"]
#[doc = " actionable they are, so the rendering and the JSON agree on priority"]
#[doc = " without a separate sort key."]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum MergeGateBlockerKind {
    State,
    Conflict,
    Ci,
    Review,
    Approval,
    Threads,
    Branch,
    Deployment,
    Policy,
    Queue,
}

impl MergeGateBlockerKind {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::State => "state",
            Self::Conflict => "conflict",
            Self::Ci => "CI",
            Self::Review => "review",
            Self::Approval => "approval",
            Self::Threads => "threads",
            Self::Branch => "branch",
            Self::Deployment => "deployment",
            Self::Policy => "policy",
            Self::Queue => "queue",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MergeGateBlocker {
    pub kind: MergeGateBlockerKind,
    pub summary: String,
    #[doc = " Named specifics, such as the failing check runs behind a CI blocker."]
    pub details: Vec<String>,
}

impl MergeGateBlocker {
    pub(super) fn new(kind: MergeGateBlockerKind, summary: impl Into<String>) -> Self {
        Self {
            kind,
            summary: summary.into(),
            details: Vec::new(),
        }
    }

    pub(super) fn with_details(mut self, details: Vec<String>) -> Self {
        self.details = details;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum GateCheckState {
    Passed,
    Failed,
    Pending,
    Skipped,
    Unknown,
}

impl GateCheckState {
    pub(super) fn from_rollup(status: &str, conclusion: &str) -> Self {
        if !status.eq_ignore_ascii_case("COMPLETED") {
            return Self::Pending;
        }
        match conclusion.to_ascii_uppercase().as_str() {
            "SUCCESS" => Self::Passed,
            "FAILURE" | "TIMED_OUT" | "STARTUP_FAILURE" | "ACTION_REQUIRED" | "CANCELLED"
            | "STALE" => Self::Failed,
            "SKIPPED" | "NEUTRAL" => Self::Skipped,
            _ => Self::Unknown,
        }
    }

    pub(super) fn from_status_context(state: &str) -> Self {
        match state.to_ascii_uppercase().as_str() {
            "SUCCESS" => Self::Passed,
            "FAILURE" | "ERROR" => Self::Failed,
            "PENDING" | "EXPECTED" => Self::Pending,
            _ => Self::Unknown,
        }
    }

    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Pending => "pending",
            Self::Skipped => "skipped",
            Self::Unknown => "unknown",
        }
    }
}

#[doc = " One entry of the head commit's status-check rollup, flattened across"]
#[doc = " Actions check runs and legacy commit statuses, and tagged with whether"]
#[doc = " the base branch's rules actually require it."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GateCheck {
    pub name: String,
    pub workflow: String,
    pub state: GateCheckState,
    pub required: bool,
    pub url: String,
    #[doc = " Set when the check is a deployment gate waiting for a human."]
    pub awaiting_approval: bool,
}

impl GateCheck {
    pub(crate) fn display_name(&self) -> String {
        if self.workflow.is_empty() || self.workflow == self.name {
            self.name.clone()
        } else {
            format!("{} / {}", self.workflow, self.name)
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MergeGateChecks {
    pub checks: Vec<GateCheck>,
    pub required_total: usize,
    pub required_passed: usize,
    pub required_failed: usize,
    pub required_pending: usize,
    pub optional_failed: usize,
    #[doc = " Contexts the base branch requires that the head commit never reported."]
    pub missing_required: Vec<String>,
    pub truncated: bool,
}

impl MergeGateChecks {
    pub(crate) fn failing(&self) -> impl Iterator<Item = &GateCheck> {
        self.checks
            .iter()
            .filter(|check| check.state == GateCheckState::Failed)
    }

    pub(crate) fn pending_required(&self) -> impl Iterator<Item = &GateCheck> {
        self.checks
            .iter()
            .filter(|check| check.required && check.state == GateCheckState::Pending)
    }
}

#[doc = " One reviewer's latest opinion, and whether it still applies to the head"]
#[doc = " commit that is up for merge."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GateReview {
    pub author: String,
    pub state: String,
    pub commit_oid: String,
    #[doc = " The review approved or rejected an older commit than the current head."]
    pub stale: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MergeGateReview {
    pub decision: String,
    pub reviews: Vec<GateReview>,
    pub approvals: usize,
    pub current_approvals: usize,
    pub stale_approvals: usize,
    pub changes_requested_by: Vec<String>,
    pub requested_reviewers: Vec<String>,
    pub required_approvals: usize,
    pub requires_code_owner_review: bool,
    pub requires_conversation_resolution: bool,
    pub unresolved_threads: usize,
    pub outdated_unresolved_threads: usize,
    pub threads_truncated: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MergeGateBranch {
    pub base_ref: String,
    pub base_oid: String,
    pub head_oid: String,
    pub merge_state: String,
    pub mergeable: String,
    #[doc = " How many commits the base branch has that the head does not. `None`"]
    #[doc = " when GitHub could not answer the comparison."]
    pub behind_by: Option<usize>,
    pub requires_linear_history: bool,
    pub requires_signatures: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MergeGateQueue {
    pub state: String,
    pub position: usize,
    pub enqueued: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MergeGateAutoMerge {
    pub enabled: bool,
    pub method: String,
    pub enabled_by: String,
}

#[doc = " Everything `pr gate` combines, plus the verdict it reaches. The document"]
#[doc = " is the stable contract: shells read `verdict`, tools read `blockers`, and"]
#[doc = " the sections carry the numbers behind both."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MergeGate {
    pub schema_version: u8,
    pub repository: String,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub state: String,
    pub is_draft: bool,
    pub verdict: MergeGateVerdict,
    pub blockers: Vec<MergeGateBlocker>,
    pub checks: MergeGateChecks,
    pub review: MergeGateReview,
    pub branch: MergeGateBranch,
    pub queue: Option<MergeGateQueue>,
    pub auto_merge: MergeGateAutoMerge,
    #[doc = " Rules Quinjet could not read, so a caller knows the verdict is partial"]
    #[doc = " rather than permissive."]
    pub warnings: Vec<String>,
    pub from_cache: bool,
}

impl MergeGate {
    pub(crate) const SCHEMA_VERSION: u8 = 1;

    #[doc = " A one-line reason, for a stack row or a status bar that has no space"]
    #[doc = " for the full list."]
    pub(crate) fn headline(&self) -> String {
        self.blockers.first().map_or_else(
            || self.verdict.word().to_owned(),
            |blocker| format!("{}: {}", blocker.kind.label(), blocker.summary),
        )
    }
}

#[doc = " One stack member's verdict, in merge order."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StackGateMember {
    pub position: usize,
    pub number: u64,
    pub title: String,
    pub selected: bool,
    pub gate: MergeGate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StackGate {
    pub schema_version: u8,
    pub number: u64,
    pub base_ref: String,
    pub size: usize,
    pub selected_position: usize,
    pub members: Vec<StackGateMember>,
    pub verdict: MergeGateVerdict,
    #[doc = " The positions that can merge right now, lowest first, stopping at the"]
    #[doc = " first member that cannot: merging past a blocked layer is not safe."]
    pub mergeable_prefix: Vec<usize>,
    #[doc = " The lowest blocked position, which is the one worth working on."]
    pub critical_position: Option<usize>,
    pub truncated: bool,
    pub warnings: Vec<String>,
}

impl StackGate {
    pub(crate) fn critical_member(&self) -> Option<&StackGateMember> {
        let position = self.critical_position?;
        self.members
            .iter()
            .find(|member| member.position == position)
    }
}
