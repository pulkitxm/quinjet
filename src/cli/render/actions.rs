#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(crate) fn workflow_runs(listing: &PullRequestWorkflowRuns) -> String {
    if listing.runs.is_empty() {
        return "No workflow runs reported\n".to_owned();
    }
    let mut out = Report::default();
    for run in &listing.runs {
        out.line(&format!(
            "{:<10} {:<10} {:<40} run {}",
            run.state.word(),
            run.event,
            truncate(&run.name, 40),
            run.id
        ));
    }
    if listing.truncated {
        out.line("\n[the workflow-run list reached Quinjet's size cap]");
    }
    out.finish()
}

pub(crate) fn artifacts(listing: &PullRequestArtifacts) -> String {
    let mut out = Report::default();
    if listing.artifacts.is_empty() {
        out.line("No artifacts reported");
    }
    for artifact in &listing.artifacts {
        let state = if artifact.expired { "expired" } else { "ready" };
        out.line(&format!(
            "{:<8} {:>10}  {:<40} {}",
            state,
            artifact.size_label(),
            truncate(&artifact.name, 40),
            artifact.workflow
        ));
    }
    if listing.truncated {
        out.line("\n[the artifact list reached Quinjet's size cap]");
    }
    for warning in &listing.warnings {
        out.line(&format!("note  {warning}"));
    }
    out.finish()
}

pub(crate) fn deployments(listing: &PullRequestDeployments) -> String {
    let mut out = Report::default();
    if listing.pending.is_empty() && listing.deployments.is_empty() {
        out.line("No deployments reported");
    }
    if !listing.pending.is_empty() {
        out.line("Waiting for approval");
        for deployment in &listing.pending {
            let reviewers = if deployment.reviewers.is_empty() {
                String::new()
            } else {
                format!("  reviewers {}", deployment.reviewers.join(", "))
            };
            let allowed = if deployment.viewer_can_approve {
                ""
            } else {
                "  (you cannot review this)"
            };
            out.line(&format!(
                "  {:<24} run {}  {}{reviewers}{allowed}",
                deployment.environment, deployment.run_id, deployment.workflow
            ));
        }
    }
    if !listing.deployments.is_empty() {
        if !listing.pending.is_empty() {
            out.blank();
        }
        out.line("Deployed");
        for deployment in &listing.deployments {
            let transient = if deployment.transient {
                "  transient"
            } else {
                ""
            };
            out.line(&format!(
                "  {:<24} {}{transient}",
                deployment.environment,
                format_local_timestamp(&deployment.created_at)
            ));
        }
    }
    for warning in &listing.warnings {
        out.line(&format!("note  {warning}"));
    }
    out.finish()
}
