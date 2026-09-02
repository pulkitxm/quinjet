#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " A change to what GitHub Actions is doing for a pull request. Each one"]
#[doc = " names the runs or jobs it will act on, so the preview a caller sees"]
#[doc = " without `--yes` is the exact set the confirmation performs."]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkflowOperation {
    #[doc = " Rerun only the failed jobs of each named run."]
    RerunFailedJobs { runs: Vec<WorkflowRun> },
    #[doc = " Rerun each named run from the start."]
    RerunRuns { runs: Vec<WorkflowRun> },
    #[doc = " Rerun one Actions job, named by the check that reported it."]
    RerunJob { check: String, job_id: u64 },
    #[doc = " Cancel each run that has not settled."]
    CancelRuns { runs: Vec<WorkflowRun> },
    #[doc = " Let one environment's held runs through, or reject them."]
    ReviewDeployments {
        environment: String,
        approve: bool,
        comment: String,
        pending: Vec<PendingDeployment>,
    },
}

impl WorkflowOperation {
    pub(crate) const fn label(&self) -> &'static str {
        match self {
            Self::RerunFailedJobs { .. } => "Rerunning failed jobs",
            Self::RerunRuns { .. } => "Rerunning workflow runs",
            Self::RerunJob { .. } => "Rerunning one job",
            Self::CancelRuns { .. } => "Cancelling workflow runs",
            Self::ReviewDeployments { approve: true, .. } => "Approving deployments",
            Self::ReviewDeployments { approve: false, .. } => "Rejecting deployments",
        }
    }

    #[doc = " Nothing to act on is not an error: a pull request with no failed run"]
    #[doc = " and nothing in flight is the state a caller wanted."]
    pub(crate) const fn is_empty(&self) -> bool {
        match self {
            Self::RerunFailedJobs { runs }
            | Self::RerunRuns { runs }
            | Self::CancelRuns { runs } => runs.is_empty(),
            Self::RerunJob { .. } => false,
            Self::ReviewDeployments { pending, .. } => pending.is_empty(),
        }
    }

    #[doc = " What `--yes` would do, named run by run, so a preview is a complete"]
    #[doc = " description rather than a summary."]
    pub(crate) fn preview_message(&self) -> String {
        if self.is_empty() {
            return format!("Nothing to act on: {}", self.nothing_reason());
        }
        let mut message = String::from("Would ");
        message.push_str(self.preview_verb());
        message.push(' ');
        message.push_str(&self.targets().join(", "));
        message.push_str(". Pass --yes to do it.");
        message
    }

    #[doc = " The verb a preview uses. Spelled out rather than derived from the"]
    #[doc = " label, so the two never drift into different sentences."]
    const fn preview_verb(&self) -> &'static str {
        match self {
            Self::RerunFailedJobs { .. } => "rerun the failed jobs of",
            Self::RerunRuns { .. } | Self::RerunJob { .. } => "rerun",
            Self::CancelRuns { .. } => "cancel",
            Self::ReviewDeployments { approve: true, .. } => "approve",
            Self::ReviewDeployments { approve: false, .. } => "reject",
        }
    }

    pub(crate) fn success_message(&self) -> String {
        let mut message = match self {
            Self::RerunFailedJobs { .. } => String::from("Reran the failed jobs of "),
            Self::RerunRuns { .. } | Self::RerunJob { .. } => String::from("Reran "),
            Self::CancelRuns { .. } => String::from("Cancelled "),
            Self::ReviewDeployments { approve: true, .. } => String::from("Approved "),
            Self::ReviewDeployments { approve: false, .. } => String::from("Rejected "),
        };
        message.push_str(&self.targets().join(", "));
        message
    }

    fn nothing_reason(&self) -> String {
        match self {
            Self::RerunFailedJobs { .. } | Self::RerunRuns { .. } => {
                String::from("no workflow run on this pull request has failed")
            }
            Self::RerunJob { .. } => String::from("there is no job to rerun"),
            Self::CancelRuns { .. } => {
                String::from("no workflow run on this pull request is still going")
            }
            Self::ReviewDeployments { environment, .. } => {
                format!("no run is waiting on `{environment}`")
            }
        }
    }

    #[doc = " The names a preview and a success message both read from, so the two"]
    #[doc = " can never describe different sets."]
    fn targets(&self) -> Vec<String> {
        match self {
            Self::RerunFailedJobs { runs }
            | Self::RerunRuns { runs }
            | Self::CancelRuns { runs } => runs.iter().map(run_label).collect(),
            Self::RerunJob { check, .. } => vec![format!("`{check}`")],
            Self::ReviewDeployments {
                environment,
                pending,
                ..
            } => pending
                .iter()
                .map(|deployment| format!("`{environment}` for run {}", deployment.run_id))
                .collect(),
        }
    }
}

fn run_label(run: &WorkflowRun) -> String {
    if run.name.is_empty() {
        return format!("run {}", run.id);
    }
    format!("`{}` (run {})", run.name, run.id)
}

impl Repository {
    #[doc = " Perform a workflow operation. Every branch is a POST that GitHub"]
    #[doc = " answers with no body, so the operation's own message is the answer."]
    pub(crate) fn perform_workflow_operation(
        &self,
        pull_request: &PullRequest,
        operation: &WorkflowOperation,
    ) -> Result<String> {
        if operation.is_empty() {
            return Ok(operation.preview_message());
        }
        let repository = &pull_request.base_repository;
        match operation {
            WorkflowOperation::RerunFailedJobs { runs } => {
                for run in runs {
                    self.actions_post(
                        repository,
                        &format!("actions/runs/{}/rerun-failed-jobs", run.id),
                        &[],
                        "unable to rerun the failed jobs",
                    )?;
                }
            }
            WorkflowOperation::RerunRuns { runs } => {
                for run in runs {
                    self.actions_post(
                        repository,
                        &format!("actions/runs/{}/rerun", run.id),
                        &[],
                        "unable to rerun the workflow run",
                    )?;
                }
            }
            WorkflowOperation::RerunJob { job_id, .. } => self.actions_post(
                repository,
                &format!("actions/jobs/{job_id}/rerun"),
                &[],
                "unable to rerun the job",
            )?,
            WorkflowOperation::CancelRuns { runs } => {
                for run in runs {
                    self.actions_post(
                        repository,
                        &format!("actions/runs/{}/cancel", run.id),
                        &[],
                        "unable to cancel the workflow run",
                    )?;
                }
            }
            WorkflowOperation::ReviewDeployments {
                approve,
                comment,
                pending,
                ..
            } => self.review_deployments(repository, *approve, comment, pending)?,
        }
        Ok(operation.success_message())
    }

    #[doc = " One request per run, carrying every environment of that run the"]
    #[doc = " caller named, because the endpoint takes a run and a list of"]
    #[doc = " environments together."]
    fn review_deployments(
        &self,
        repository: &GitHubRepository,
        approve: bool,
        comment: &str,
        pending: &[PendingDeployment],
    ) -> Result<()> {
        let mut runs: Vec<u64> = pending.iter().map(|deployment| deployment.run_id).collect();
        runs.sort_unstable();
        runs.dedup();
        for run in runs {
            let environments: Vec<u64> = pending
                .iter()
                .filter(|deployment| deployment.run_id == run)
                .map(|deployment| deployment.environment_id)
                .collect();
            let payload = serde_json::json!({
                "environment_ids": environments,
                "state": if approve { "approved" } else { "rejected" },
                "comment": comment,
            });
            let body = serde_json::to_vec(&payload)?;
            let output = self.run_gh_with_input(
                [
                    OsString::from("api"),
                    OsString::from("--method"),
                    OsString::from("POST"),
                    OsString::from(format!(
                        "repos/{}/actions/runs/{run}/pending_deployments",
                        repository.name_with_owner
                    )),
                    OsString::from("--input"),
                    OsString::from("-"),
                ],
                &body,
                super::super::MAX_GH_METADATA_BYTES,
            )?;
            if !output.status.success() {
                bail!(
                    "{}",
                    bounded_command_error("unable to review the pending deployment", &output)
                );
            }
        }
        Ok(())
    }
}
