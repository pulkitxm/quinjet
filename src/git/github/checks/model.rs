#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PullRequestCheckStatus {
    Pending,
    Passed,
    Failed,
    Skipped,
    Cancelled,
    Unknown,
}

impl PullRequestCheckStatus {
    pub(crate) const fn is_running(self) -> bool {
        matches!(self, Self::Pending)
    }

    pub(super) fn from_conclusion(status: &str, conclusion: &str) -> Self {
        if !status.eq_ignore_ascii_case("completed") {
            return Self::Pending;
        }
        match conclusion.to_ascii_lowercase().as_str() {
            "success" => Self::Passed,
            "failure" | "timed_out" | "action_required" => Self::Failed,
            "skipped" | "neutral" => Self::Skipped,
            "cancelled" | "stale" => Self::Cancelled,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestCheck {
    pub name: String,
    pub workflow: String,
    pub state: String,
    pub status: PullRequestCheckStatus,
    pub description: String,
    pub link: String,
    pub started_at: String,
    pub completed_at: String,
}

impl PullRequestCheck {
    pub(crate) fn duration_label(&self) -> String {
        elapsed_label(&self.started_at, &self.completed_at)
    }

    #[doc = " GitHub Actions check links end in `/actions/runs/<run>/job/<job>`, which"]
    #[doc = " is the only place a check run exposes the job identity its logs need."]
    pub(crate) fn job_id(&self) -> Option<u64> {
        let (_, job) = self.link.rsplit_once("/job/")?;
        let job = job.split(['?', '#', '/']).next()?;
        job.parse().ok()
    }

    pub(crate) fn identity(&self) -> String {
        self.job_id().map_or_else(
            || {
                if self.link.is_empty() {
                    format!("{}\n{}\n{}", self.workflow, self.name, self.started_at)
                } else {
                    self.link.clone()
                }
            },
            |job| job.to_string(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CheckLogSeverity {
    Normal,
    Command,
    Notice,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckLogLine {
    pub timestamp: String,
    pub text: String,
    pub severity: CheckLogSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckStep {
    pub number: usize,
    pub name: String,
    pub status: PullRequestCheckStatus,
    pub conclusion: String,
    pub started_at: String,
    pub completed_at: String,
    pub lines: Vec<CheckLogLine>,
}

impl CheckStep {
    #[doc = " How long the step took, or how long it has been running so far when it"]
    #[doc = " has started but not finished."]
    pub(crate) fn duration_label(&self, now: i64) -> String {
        if self.completed_at.is_empty() {
            let Some(started) = timestamp_seconds(&self.started_at) else {
                return String::new();
            };
            return if now > started {
                format!("{}…", format_elapsed(now - started))
            } else {
                String::new()
            };
        }
        elapsed_label(&self.started_at, &self.completed_at)
    }
}

#[doc = " Seconds since the Unix epoch, for measuring against a GitHub timestamp."]
pub(crate) fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
        .unwrap_or_default()
}

#[doc = " A check list plus where it came from, so the view can say whether it is"]
#[doc = " showing a cached answer or one just read from GitHub."]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestChecks {
    pub checks: Vec<PullRequestCheck>,
    pub from_cache: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckRunLog {
    pub steps: Vec<CheckStep>,
    #[doc = " Output produced before the first step or after the last one, which is"]
    #[doc = " where a runner reports provisioning and teardown failures."]
    pub loose_lines: Vec<CheckLogLine>,
    pub truncated: bool,
    #[doc = " Set when there is nothing to show at all, with the reason to display."]
    pub unavailable: Option<String>,
    #[doc = " The runner has not written anything yet. This is only true for the first"]
    #[doc = " seconds of a job: GitHub serves a growing partial log from then on, so a"]
    #[doc = " running job tails rather than waiting for its own completion."]
    pub log_pending: bool,
}

impl CheckRunLog {
    pub(super) fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            unavailable: Some(reason.into()),
            ..Self::default()
        }
    }

    #[doc = " The step a runner is currently executing, if any."]
    pub(crate) fn running_step(&self) -> Option<&CheckStep> {
        self.steps
            .iter()
            .find(|step| step.status == PullRequestCheckStatus::Pending)
    }

    pub(crate) fn failed_step(&self) -> Option<&CheckStep> {
        self.steps
            .iter()
            .find(|step| step.status == PullRequestCheckStatus::Failed)
    }
}
