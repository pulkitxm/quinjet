#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WorkflowRunState {
    Queued,
    Running,
    Passed,
    Failed,
    Cancelled,
    Skipped,
    Unknown,
}

impl WorkflowRunState {
    pub(super) fn parse(status: &str, conclusion: &str) -> Self {
        match status.to_ascii_lowercase().as_str() {
            "queued" | "waiting" | "requested" | "pending" => return Self::Queued,
            "in_progress" => return Self::Running,
            _ => {}
        }
        match conclusion.to_ascii_lowercase().as_str() {
            "success" => Self::Passed,
            "failure" | "timed_out" | "startup_failure" | "action_required" => Self::Failed,
            "cancelled" => Self::Cancelled,
            "skipped" | "neutral" => Self::Skipped,
            _ => Self::Unknown,
        }
    }

    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Skipped => "skipped",
            Self::Unknown => "unknown",
        }
    }

    #[doc = " A run that has not settled is the only kind worth cancelling."]
    pub(crate) const fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }

    #[doc = " A run worth rerunning failed jobs for."]
    pub(crate) const fn is_failed(self) -> bool {
        matches!(self, Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkflowRun {
    pub id: u64,
    pub name: String,
    pub state: WorkflowRunState,
    pub status: String,
    pub conclusion: String,
    pub url: String,
    pub attempt: usize,
    pub event: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestWorkflowRuns {
    pub head_oid: String,
    pub runs: Vec<WorkflowRun>,
    pub truncated: bool,
    pub from_cache: bool,
}

impl PullRequestWorkflowRuns {
    pub(crate) fn active(&self) -> impl Iterator<Item = &WorkflowRun> {
        self.runs.iter().filter(|run| run.state.is_active())
    }

    pub(crate) fn failed(&self) -> impl Iterator<Item = &WorkflowRun> {
        self.runs.iter().filter(|run| run.state.is_failed())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkflowArtifact {
    pub id: u64,
    pub name: String,
    pub size_in_bytes: u64,
    pub expired: bool,
    pub expires_at: String,
    pub created_at: String,
    pub run_id: u64,
    pub workflow: String,
    pub download_url: String,
}

impl WorkflowArtifact {
    #[doc = " A human size, because an artifact listing is read to decide whether to"]
    #[doc = " spend the download."]
    #[expect(
        clippy::integer_division,
        reason = "a size shown to a reader is deliberately whole units"
    )]
    pub(crate) fn size_label(&self) -> String {
        let bytes = self.size_in_bytes;
        for (limit, suffix, divisor) in [
            (1024_u64, "B", 1_u64),
            (1024 * 1024, "KiB", 1024),
            (1024 * 1024 * 1024, "MiB", 1024 * 1024),
        ] {
            if bytes < limit {
                return format!("{} {suffix}", bytes / divisor);
            }
        }
        format!("{} GiB", bytes / (1024 * 1024 * 1024))
    }

    #[doc = " GitHub lets a workflow name an artifact almost anything, and that name"]
    #[doc = " is written by whoever can edit the workflow. It is never used as a"]
    #[doc = " path component without passing this first."]
    pub(crate) fn safe_file_name(&self) -> Result<String> {
        let name = self.name.trim();
        if name.is_empty() || name.len() > MAX_ARTIFACT_NAME_BYTES {
            bail!(
                "artifact {} has a name Quinjet will not write to disk",
                self.id
            );
        }
        if name == "." || name == ".." || name.starts_with('-') {
            bail!("artifact `{name}` has a name Quinjet will not write to disk");
        }
        if name.chars().any(|character| {
            character == '/' || character == '\\' || character == ':' || character.is_control()
        }) {
            bail!("artifact `{name}` has a name Quinjet will not write to disk");
        }
        Ok(format!("{name}.zip"))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestArtifacts {
    pub head_oid: String,
    pub artifacts: Vec<WorkflowArtifact>,
    pub truncated: bool,
    pub warnings: Vec<String>,
}

impl PullRequestArtifacts {
    #[doc = " Match by exact name first, then by a unique case-insensitive"]
    #[doc = " substring, the way a check is selected for its log."]
    pub(crate) fn select(&self, wanted: &str) -> Result<&WorkflowArtifact> {
        if let Some(artifact) = self
            .artifacts
            .iter()
            .find(|artifact| artifact.name == wanted)
        {
            return Ok(artifact);
        }
        let partial: Vec<&WorkflowArtifact> = self
            .artifacts
            .iter()
            .filter(|artifact| {
                artifact
                    .name
                    .to_lowercase()
                    .contains(&wanted.to_lowercase())
            })
            .collect();
        match partial.as_slice() {
            [only] => Ok(only),
            [] => bail!("no artifact on this pull request is called `{wanted}`"),
            _ => bail!("`{wanted}` matches more than one artifact"),
        }
    }

    pub(crate) fn names(&self) -> String {
        self.artifacts
            .iter()
            .map(|artifact| artifact.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[doc = " An environment holding a workflow run until somebody approves it."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PendingDeployment {
    pub run_id: u64,
    pub workflow: String,
    pub environment: String,
    pub environment_id: u64,
    pub wait_timer: usize,
    pub viewer_can_approve: bool,
    pub reviewers: Vec<String>,
}

#[doc = " A deployment GitHub already recorded for the head commit."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeploymentRecord {
    pub id: u64,
    pub environment: String,
    pub description: String,
    pub created_at: String,
    pub url: String,
    pub transient: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestDeployments {
    pub head_oid: String,
    pub pending: Vec<PendingDeployment>,
    pub deployments: Vec<DeploymentRecord>,
    pub warnings: Vec<String>,
}

impl PullRequestDeployments {
    #[doc = " Every pending environment whose name matches, because one environment"]
    #[doc = " can hold more than one run at a time and approving it means all of"]
    #[doc = " them."]
    pub(crate) fn pending_for(&self, environment: &str) -> Vec<&PendingDeployment> {
        self.pending
            .iter()
            .filter(|deployment| {
                deployment
                    .environment
                    .eq_ignore_ascii_case(environment.trim())
            })
            .collect()
    }

    pub(crate) fn environments(&self) -> String {
        let mut names: Vec<&str> = self
            .pending
            .iter()
            .map(|deployment| deployment.environment.as_str())
            .collect();
        names.sort_unstable();
        names.dedup();
        names.join(", ")
    }
}
