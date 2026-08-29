#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " List the annotations a pull request's check runs placed on its lines,"]
#[doc = " with each one marked by whether the pull request's own patch shows the"]
#[doc = " line it points at."]
pub(super) fn annotations(
    session: &mut Session,
    out: &Emitter,
    args: &PrAnnotationsArgs,
) -> Result<u8> {
    let request = lookup(session, out, &args.pull_request)?;
    let listing = out
        .execute(
            session,
            Command::PullRequestAnnotations {
                pull_request: Box::new(request.clone()),
                refresh: args.pull_request.refresh,
            },
        )?
        .annotations()?;
    let placed = place(session, out, &request, listing)?;
    let filter = AnnotationFilter {
        severity: args.severity.map(Into::into),
        check: args.check.clone(),
        path: args.path.clone(),
        in_diff_only: args.in_diff,
    };
    let listing = filter.apply(placed);
    out.emit(&listing, || {
        render::annotations(&listing, args.group.into(), args.full)
    })?;
    Ok(if args.exit_code && listing.has_failures() {
        EXIT_FAILURE
    } else {
        0
    })
}

#[doc = " Decide each annotation's placement against the pull request's own patch."]
#[doc = " Only the annotated paths the pull request changes are loaded, in one"]
#[doc = " batch, so a wide pull request with three annotations reads three files."]
fn place(
    session: &mut Session,
    out: &Emitter,
    request: &PullRequest,
    mut listing: PullRequestAnnotations,
) -> Result<PullRequestAnnotations> {
    if listing.annotations.is_empty() {
        return Ok(listing);
    }
    let index = prepare(session, out, request)?;
    let paths = annotated_paths(&listing, &index);
    let mut visible = HashMap::new();
    for chunk in paths.chunks(16) {
        for (path, document) in out
            .execute(
                session,
                Command::PullRequestFileBatch {
                    workspace: 0,
                    paths: chunk.to_vec(),
                },
            )?
            .pull_request_diff_batch()?
        {
            drop(visible.insert(path, visible_lines(&document)));
        }
    }
    mark_diff_coverage(&mut listing, &index, &visible);
    Ok(listing)
}
