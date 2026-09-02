#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " One queue out of the conversation, the review threads and CI, so an"]
#[doc = " author does not have to read three views to find what is outstanding."]
pub(super) fn feedback(session: &mut Session, out: &Emitter, args: &PrFeedbackArgs) -> Result<u8> {
    let request = lookup(session, out, &args.pull_request)?;
    let gate = out
        .execute(
            session,
            Command::PullRequestGate {
                pull_request: Box::new(request.clone()),
                refresh: args.pull_request.refresh,
            },
        )?
        .gate()?;
    let review = out
        .execute(
            session,
            Command::PullRequestReview {
                pull_request: Box::new(request.clone()),
            },
        )?
        .review()?;
    let annotations = if args.no_checks {
        None
    } else {
        Some(Box::new(
            out.execute(
                session,
                Command::PullRequestAnnotations {
                    pull_request: Box::new(request.clone()),
                    refresh: args.pull_request.refresh,
                },
            )?
            .annotations()?,
        ))
    };
    let viewer = session
        .viewer_login(&request)
        .unwrap_or_else(|_| String::new());
    let queue = out
        .execute(
            session,
            Command::PullRequestFeedback {
                pull_request: Box::new(request),
                gate: Some(Box::new(gate)),
                review: Box::new(review),
                annotations,
                viewer,
            },
        )?
        .feedback()?;
    let filter = FeedbackFilter {
        blocking_only: args.unresolved,
        mine_only: args.mine,
    };
    let queue = filter.apply(queue);
    out.emit(&queue, || render::feedback(&queue, args.full))?;
    Ok(if args.exit_code && queue.counts.blocking > 0 {
        EXIT_FAILURE
    } else {
        0
    })
}

pub(super) fn suggestions(
    session: &mut Session,
    out: &Emitter,
    command: PrSuggestionsCommand,
) -> Result<u8> {
    let Some(PrSuggestionVerb::Apply(args)) = command.command else {
        let args = command.list.pull_request("pr suggestions")?;
        let listing = read_suggestions(session, out, &args)?.1;
        out.emit(&listing, || render::suggestions(&listing))?;
        return Ok(0);
    };
    apply(session, out, &args)
}

fn apply(session: &mut Session, out: &Emitter, args: &PrSuggestionApplyArgs) -> Result<u8> {
    let (request, listing) = read_suggestions(session, out, &args.pull_request)?;
    let chosen: Vec<Suggestion> = match &args.id {
        Some(id) => vec![
            listing
                .select(id)
                .map_err(|error| {
                    Failure::new(EXIT_NOT_FOUND, format!("{error:#}"))
                        .hint("run `quinjet pr suggestions <number>` for the ids it can apply")
                })?
                .clone(),
        ],
        None => listing
            .applicable_suggestions()
            .into_iter()
            .cloned()
            .collect(),
    };
    if chosen.is_empty() {
        out.message("Nothing to apply: no suggestion on this pull request can be applied")?;
        return Ok(0);
    }
    session
        .ensure_suggestion_checkout(&request)
        .map_err(|error| Failure::new(EXIT_FAILURE, format!("{error:#}")))?;
    let plan = out
        .execute(
            session,
            Command::PlanSuggestions {
                suggestions: chosen,
            },
        )?
        .suggestion_plan()?;
    if !args.yes {
        out.message(&preview(&plan))?;
        return Ok(0);
    }
    if plan.is_empty() {
        out.message(&preview(&plan))?;
        return Ok(0);
    }
    let message = session
        .execute(Command::ApplySuggestions {
            pull_request: Box::new(request),
            plan: Box::new(plan),
            message: args.message.clone(),
        })?
        .operation()?
        .2;
    out.message(&message)?;
    Ok(0)
}

#[doc = " Name every file the write would touch and every suggestion it would"]
#[doc = " skip, so the confirmation has nothing left to surprise a reader with."]
fn preview(plan: &SuggestionPlan) -> String {
    let mut text = if plan.is_empty() {
        String::from("Nothing to apply")
    } else {
        format!("Would apply {}", plan.summary())
    };
    for file in &plan.files {
        text.push_str("\n  ");
        text.push_str(&file.path.display().to_string());
        text.push_str("  +");
        text.push_str(&file.added.to_string());
        text.push_str(" -");
        text.push_str(&file.removed.to_string());
    }
    for skip in &plan.skipped {
        text.push_str("\n  skipped ");
        text.push_str(&skip.location);
        text.push_str(": ");
        text.push_str(&skip.reason);
    }
    if !plan.is_empty() {
        text.push_str("\nPass --yes to write them.");
    }
    text
}

fn read_suggestions(
    session: &mut Session,
    out: &Emitter,
    args: &PrArgs,
) -> Result<(PullRequest, PullRequestSuggestions)> {
    let request = lookup(session, out, args)?;
    let review = out
        .execute(
            session,
            Command::PullRequestReview {
                pull_request: Box::new(request.clone()),
            },
        )?
        .review()?;
    let listing = out
        .execute(
            session,
            Command::PullRequestSuggestions {
                pull_request: Box::new(request.clone()),
                review: Box::new(review),
            },
        )?
        .suggestions()?;
    Ok((request, listing))
}
