#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl Repository {
    #[doc = " What is deployed from this pull request's head, and what is waiting"]
    #[doc = " for a human to let it through. The two come from different endpoints"]
    #[doc = " because a pending approval belongs to a workflow run rather than to a"]
    #[doc = " deployment that has not happened yet."]
    pub(crate) fn pull_request_deployments(
        &self,
        pull_request: &PullRequest,
        runs: &PullRequestWorkflowRuns,
    ) -> PullRequestDeployments {
        let repository = &pull_request.base_repository;
        let mut listing = PullRequestDeployments {
            head_oid: pull_request.head_oid.clone(),
            ..PullRequestDeployments::default()
        };
        match self.head_deployments(repository, &pull_request.head_oid) {
            Err(error) => listing
                .warnings
                .push(format!("unable to list deployments: {error:#}")),
            Ok(deployments) => listing.deployments = deployments,
        }
        for run in runs.runs.iter().filter(|run| run.state.is_active()) {
            match self.pending_deployments(repository, run) {
                Err(error) => listing.warnings.push(format!(
                    "unable to read pending deployments for {}: {error:#}",
                    run.name
                )),
                Ok(pending) => listing.pending.extend(pending),
            }
        }
        listing.pending.sort_by(|left, right| {
            left.environment
                .cmp(&right.environment)
                .then_with(|| left.run_id.cmp(&right.run_id))
        });
        listing.deployments.sort_by(|left, right| {
            left.environment
                .cmp(&right.environment)
                .then_with(|| right.id.cmp(&left.id))
        });
        listing
    }

    fn head_deployments(
        &self,
        repository: &GitHubRepository,
        head_oid: &str,
    ) -> Result<Vec<DeploymentRecord>> {
        let output = self.run_gh([
            OsString::from("api"),
            OsString::from(format!(
                "repos/{}/deployments?sha={head_oid}&per_page=100",
                repository.name_with_owner
            )),
            OsString::from("--jq"),
            OsString::from(DEPLOYMENT_TSV_JQ),
        ])?;
        if !output.status.success() {
            bail!(
                "{}",
                bounded_command_error("unable to list deployments", &output)
            );
        }
        parse_deployments(&output.stdout)
    }

    #[doc = " Pending approvals are never cached: the answer is the input to a"]
    #[doc = " mutation, and acting on a stale one would approve the wrong run."]
    fn pending_deployments(
        &self,
        repository: &GitHubRepository,
        run: &WorkflowRun,
    ) -> Result<Vec<PendingDeployment>> {
        let output = self.run_gh([
            OsString::from("api"),
            OsString::from(format!(
                "repos/{}/actions/runs/{}/pending_deployments",
                repository.name_with_owner, run.id
            )),
            OsString::from("--jq"),
            OsString::from(PENDING_TSV_JQ),
        ])?;
        if !output.status.success() {
            bail!(
                "{}",
                bounded_command_error("unable to read pending deployments", &output)
            );
        }
        parse_pending(&output.stdout, run)
    }
}

fn parse_deployments(output: &[u8]) -> Result<Vec<DeploymentRecord>> {
    let mut deployments = Vec::new();
    for (index, record) in output.split(|byte| *byte == b'\n').enumerate() {
        if record.is_empty() {
            continue;
        }
        let [id, environment, description, created_at, url, transient] =
            parse_tsv_record::<DEPLOYMENT_TSV_FIELDS>(record)
                .with_context(|| format!("invalid deployment record {}", index + 1))?;
        deployments.push(DeploymentRecord {
            id: id.parse().unwrap_or_default(),
            environment,
            description,
            created_at,
            url,
            transient: transient == "true",
        });
    }
    Ok(deployments)
}

fn parse_pending(output: &[u8], run: &WorkflowRun) -> Result<Vec<PendingDeployment>> {
    let mut pending = Vec::new();
    for (index, record) in output.split(|byte| *byte == b'\n').enumerate() {
        if record.is_empty() {
            continue;
        }
        let [
            environment_id,
            environment,
            wait_timer,
            can_approve,
            reviewers,
        ] = parse_tsv_record::<PENDING_TSV_FIELDS>(record)
            .with_context(|| format!("invalid pending-deployment record {}", index + 1))?;
        pending.push(PendingDeployment {
            run_id: run.id,
            workflow: run.name.clone(),
            environment,
            environment_id: environment_id.parse().unwrap_or_default(),
            wait_timer: wait_timer.parse().unwrap_or_default(),
            viewer_can_approve: can_approve == "true",
            reviewers: reviewers
                .split(", ")
                .map(str::trim)
                .filter(|reviewer| !reviewer.is_empty())
                .map(str::to_owned)
                .collect(),
        });
    }
    Ok(pending)
}
