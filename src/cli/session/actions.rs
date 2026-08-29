#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " Perform a workflow operation and report what it did. The operation"]
#[doc = " carries the runs it names, so the message describes exactly the set"]
#[doc = " the preview described."]
pub(super) fn operate_workflow(
    repository: &Repository,
    pull_request: &PullRequest,
    operation: &WorkflowOperation,
) -> Result<Outcome> {
    let label = operation.label().to_owned();
    let message = repository.perform_workflow_operation(pull_request, operation)?;
    Ok(Outcome::Operation {
        label,
        changes_history: false,
        message,
    })
}
