#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " The patch a pull request gained after a commit, rather than the whole"]
#[doc = " pull request. The base of the comparison is that commit exactly, so a"]
#[doc = " reviewer sees only what moved under them."]
pub(super) fn pull_request_delta(
    session: &mut Session,
    out: &Emitter,
    request: &PullRequest,
    since: &ReviewSinceRequest,
    path: Option<&Path>,
) -> Result<DiffDocument> {
    let commits = out
        .execute(
            session,
            Command::PullRequestCommits {
                pull_request: Box::new(request.clone()),
            },
        )?
        .commits()?;
    let record = crate::state::load_review_progress(&request.base_repository.url, request.number);
    let resolved = session
        .resolve_review_since(request, since, &record, &commits)
        .map_err(|error| Failure::new(EXIT_NOT_FOUND, format!("{error:#}")))?;
    if resolved.oid == request.head_oid {
        out.note(&format!(
            "note: nothing has changed since {}",
            resolved.source.label()
        ));
    }
    let index = out
        .execute(
            session,
            Command::PreparePullRequestSince {
                workspace: 0,
                pull_request: Box::new(request.clone()),
                since: resolved.oid.clone(),
            },
        )?
        .pull_request_index()?;
    prepared_pull_request_diff(
        session,
        out,
        &index,
        format!(
            "PR #{} since {} ({})",
            request.number,
            resolved.oid.chars().take(12).collect::<String>(),
            resolved.source.label()
        ),
        path,
    )
}
