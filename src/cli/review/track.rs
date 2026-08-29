#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " The verbs that read or record local review progress, kept apart from"]
#[doc = " the ones that write to GitHub."]
pub(super) fn track(session: &mut Session, out: &Emitter, command: PrReviewVerb) -> Result<u8> {
    match command {
        PrReviewVerb::Progress(args) => progress(session, out, &args),
        PrReviewVerb::Next(args) => next(session, out, &args),
        PrReviewVerb::Viewed(args) => viewed(session, out, args),
        PrReviewVerb::Visit(args) => {
            let pull_request = lookup(session, out, &args)?;
            let message = session
                .execute(Command::RecordReviewVisit {
                    pull_request: Box::new(pull_request),
                })?
                .operation()?
                .2;
            out.message(&message)?;
            Ok(0)
        }
        PrReviewVerb::Suggest(args) => suggest(session, out, args),
        other => Err(anyhow::anyhow!(
            "the review verb {other:?} is not a progress command"
        )),
    }
}

fn read_progress(
    session: &mut Session,
    out: &Emitter,
    args: &PrArgs,
    since: &PrSinceArgs,
) -> Result<ReviewProgress> {
    let pull_request = lookup(session, out, args)?;
    let index = prepare(session, out, &pull_request)?;
    out.execute(
        session,
        Command::PullRequestReviewProgress {
            pull_request: Box::new(pull_request),
            index: Box::new(index),
            since: since.request().unwrap_or(ReviewSinceRequest::LastReview),
        },
    )?
    .review_progress()
}

fn progress(session: &mut Session, out: &Emitter, args: &PrReviewProgressArgs) -> Result<u8> {
    let progress = read_progress(session, out, &args.pull_request, &args.since)?;
    out.emit(&progress, || render::review_progress(&progress, args.all))?;
    Ok(0)
}

fn next(session: &mut Session, out: &Emitter, args: &PrReviewNextArgs) -> Result<u8> {
    let progress = read_progress(session, out, &args.pull_request, &PrSinceArgs::default())?;
    match args.wanted.select(&progress) {
        None => out.emit(&NoNextStep { next: None }, || {
            "Nothing left to review\n".to_owned()
        })?,
        Some(step) => out.emit(&step, || render::review_next(&step))?,
    }
    Ok(0)
}

#[derive(Serialize)]
struct NoNextStep {
    next: Option<ReviewNextStep>,
}

impl PrReviewNextChoiceArgs {
    fn select(&self, progress: &ReviewProgress) -> Option<ReviewNextStep> {
        if self.threads {
            return progress.next_thread();
        }
        if self.files {
            return progress.next_file();
        }
        progress.next.clone()
    }
}

fn viewed(session: &mut Session, out: &Emitter, args: PrReviewViewedArgs) -> Result<u8> {
    let pull_request = lookup(session, out, &args.pull_request)?;
    if args.reset {
        let message = session
            .execute(Command::ForgetReviewProgress {
                pull_request: Box::new(pull_request),
            })?
            .operation()?
            .2;
        out.message(&message)?;
        return Ok(0);
    }
    let paths = if args.all {
        prepare(session, out, &pull_request)?
            .files
            .into_iter()
            .map(|file| file.path)
            .collect()
    } else {
        require_paths(args.paths, "reviews viewed")?
    };
    let message = session
        .execute(Command::MarkReviewFiles {
            pull_request: Box::new(pull_request),
            paths,
            viewed: !args.unviewed,
        })?
        .operation()?
        .2;
    out.message(&message)?;
    Ok(0)
}

#[doc = " Propose an exact replacement for a range of lines. The body is the"]
#[doc = " suggestion fence GitHub renders as an apply-able block, so this is the"]
#[doc = " same thing a reviewer writes by hand, without the fence."]
fn suggest(session: &mut Session, out: &Emitter, args: PrSuggestArgs) -> Result<u8> {
    let replacement = review_body(&args.text)?;
    let body = suggestion_body(&replacement, &args.note)
        .map_err(|error| Failure::new(EXIT_FAILURE, format!("{error:#}")))?;
    let start_line = args.start_line.filter(|start| *start != args.line);
    if start_line.is_some_and(|start| start > args.line) {
        return Err(Failure::new(EXIT_FAILURE, "--start-line must not be after --line").into());
    }
    mutate(
        session,
        out,
        &args.pull_request,
        PullRequestReviewOperation::AddThread {
            body,
            path: args.path,
            line: Some(args.line),
            side: Some(PullRequestReviewSide::Right),
            start_line,
            start_side: start_line.map(|_| PullRequestReviewSide::Right),
            subject: PullRequestReviewThreadSubject::Line,
        },
    )
}
