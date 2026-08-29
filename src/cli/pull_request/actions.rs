#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn runs(session: &mut Session, out: &Emitter, args: &PrArgs) -> Result<u8> {
    let request = lookup(session, out, args)?;
    let listing = workflow_runs(session, out, &request, args.refresh)?;
    out.emit(&listing, || render::workflow_runs(&listing))?;
    Ok(0)
}

#[doc = " Rerun failed jobs, whole runs, or the one job a named check reported."]
pub(super) fn rerun(session: &mut Session, out: &Emitter, args: &PrRerunArgs) -> Result<u8> {
    let request = lookup(session, out, &args.pull_request)?;
    let operation = if let Some(name) = &args.check {
        let listing = out
            .execute(
                session,
                Command::PullRequestChecks {
                    pull_request: Box::new(request.clone()),
                    refresh: args.pull_request.refresh,
                },
            )?
            .checks()?;
        let check = select_check(&listing.checks, name)?;
        let job_id = Repository::check_job_id(&check)
            .map_err(|error| Failure::new(EXIT_UNAVAILABLE, format!("{error:#}")))?;
        WorkflowOperation::RerunJob {
            check: check.name,
            job_id,
        }
    } else {
        let runs = workflow_runs(session, out, &request, args.pull_request.refresh)?;
        let failed: Vec<_> = runs.failed().cloned().collect();
        if args.all {
            WorkflowOperation::RerunRuns { runs: failed }
        } else {
            WorkflowOperation::RerunFailedJobs { runs: failed }
        }
    };
    operate_workflow(session, out, &request, operation, args.yes)
}

pub(super) fn cancel(session: &mut Session, out: &Emitter, args: &PrCancelArgs) -> Result<u8> {
    let request = lookup(session, out, &args.pull_request)?;
    let runs = workflow_runs(session, out, &request, true)?;
    let operation = WorkflowOperation::CancelRuns {
        runs: runs.active().cloned().collect(),
    };
    operate_workflow(session, out, &request, operation, args.yes)
}

pub(super) fn artifacts(
    session: &mut Session,
    out: &Emitter,
    command: PrArtifactsCommand,
) -> Result<u8> {
    let Some(PrArtifactVerb::Download(args)) = command.command else {
        let args = command.list.pull_request("pr artifacts")?;
        let listing = read_artifacts(session, out, &args)?.1;
        out.emit(&listing, || render::artifacts(&listing))?;
        return Ok(0);
    };
    download(session, out, &args)
}

fn download(session: &mut Session, out: &Emitter, args: &PrArtifactDownloadArgs) -> Result<u8> {
    let (request, listing) = read_artifacts(session, out, &args.pull_request)?;
    let artifact = listing.select(&args.name).map_err(|error| {
        Failure::new(EXIT_NOT_FOUND, format!("{error:#}")).hint(format!(
            "the artifacts are: {}",
            if listing.artifacts.is_empty() {
                "none".to_owned()
            } else {
                listing.names()
            }
        ))
    })?;
    let path = out
        .execute(
            session,
            Command::DownloadArtifact {
                pull_request: Box::new(request),
                artifact: Box::new(artifact.clone()),
                directory: args.directory.clone(),
            },
        )?
        .downloaded_artifact()?;
    out.message(&format!("Saved {}", path.display()))?;
    Ok(0)
}

pub(super) fn deployments(
    session: &mut Session,
    out: &Emitter,
    command: PrDeploymentsCommand,
) -> Result<u8> {
    let (approve, args) = match command.command {
        None => {
            let args = command.list.pull_request("pr deployments")?;
            let request = lookup(session, out, &args)?;
            let listing = read_deployments(session, out, &request, args.refresh)?;
            out.emit(&listing, || render::deployments(&listing))?;
            return Ok(0);
        }
        Some(PrDeploymentVerb::Approve(args)) => (true, args),
        Some(PrDeploymentVerb::Reject(args)) => (false, args),
    };
    let request = lookup(session, out, &args.pull_request)?;
    let listing = read_deployments(session, out, &request, true)?;
    let pending: Vec<_> = listing
        .pending_for(&args.environment)
        .into_iter()
        .cloned()
        .collect();
    if !pending.is_empty()
        && let Some(blocked) = pending.iter().find(|entry| !entry.viewer_can_approve)
    {
        return Err(Failure::new(
            EXIT_UNAVAILABLE,
            format!(
                "GitHub does not let you review `{}` on run {}",
                blocked.environment, blocked.run_id
            ),
        )
        .into());
    }
    let operation = WorkflowOperation::ReviewDeployments {
        environment: args.environment.clone(),
        approve,
        comment: args.comment.clone(),
        pending,
    };
    if operation.is_empty() && !listing.pending.is_empty() {
        out.note(&format!(
            "hint: the waiting environments are: {}",
            listing.environments()
        ));
    }
    operate_workflow(session, out, &request, operation, args.yes)
}

fn read_artifacts(
    session: &mut Session,
    out: &Emitter,
    args: &PrArgs,
) -> Result<(PullRequest, PullRequestArtifacts)> {
    let request = lookup(session, out, args)?;
    let runs = workflow_runs(session, out, &request, args.refresh)?;
    let listing = out
        .execute(
            session,
            Command::PullRequestArtifacts {
                pull_request: Box::new(request.clone()),
                runs: Box::new(runs),
            },
        )?
        .artifacts()?;
    Ok((request, listing))
}

fn read_deployments(
    session: &mut Session,
    out: &Emitter,
    request: &PullRequest,
    refresh: bool,
) -> Result<PullRequestDeployments> {
    let runs = workflow_runs(session, out, request, refresh)?;
    out.execute(
        session,
        Command::PullRequestDeployments {
            pull_request: Box::new(request.clone()),
            runs: Box::new(runs),
        },
    )?
    .deployments()
}

fn workflow_runs(
    session: &mut Session,
    out: &Emitter,
    request: &PullRequest,
    refresh: bool,
) -> Result<PullRequestWorkflowRuns> {
    out.execute(
        session,
        Command::PullRequestWorkflowRuns {
            pull_request: Box::new(request.clone()),
            refresh,
        },
    )?
    .workflow_runs()
}

#[doc = " Every workflow mutation is preview-first: without `--yes` it names the"]
#[doc = " exact runs it would act on and changes nothing."]
fn operate_workflow(
    session: &mut Session,
    out: &Emitter,
    request: &PullRequest,
    operation: WorkflowOperation,
    yes: bool,
) -> Result<u8> {
    if !yes || operation.is_empty() {
        out.message(&operation.preview_message())?;
        return Ok(0);
    }
    let message = session
        .execute(Command::OperateWorkflow {
            pull_request: Box::new(request.clone()),
            operation: Box::new(operation),
        })?
        .operation()?
        .2;
    out.message(&message)?;
    Ok(0)
}
