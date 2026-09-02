use super::stack_verbs::{StackFeedbackArgs, StackGateArgs, StackRangeArgs, StackReviewArgs};
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn stack(session: &mut Session, out: &Emitter, command: StackVerb) -> Result<u8> {
    match command {
        StackVerb::View(args) => {
            let snapshot = lookup_stack(session, out, &args)?;
            out.emit(&snapshot, || render::pull_request_stack(&snapshot))?;
        }
        StackVerb::Files(args) => {
            let (stack, from, to) = stack_range(session, out, &args)?;
            let index = prepare_stack(session, out, stack, from, to)?;
            out.emit(&index, || render::pull_request_files(&index))?;
        }
        StackVerb::Gate(args) => return stack_gate(session, out, &args),
        StackVerb::Review(args) => return stack_review(session, out, &args),
        StackVerb::Feedback(args) => return stack_feedback(session, out, &args),
        StackVerb::Diff(args) => {
            let (stack, from, to) = stack_range(session, out, &args.range)?;
            let title = format!("Stack #{} positions {from} through {to}", stack.number);
            let index = prepare_stack(session, out, stack, from, to)?;
            let document =
                prepared_pull_request_diff(session, out, &index, title, args.path.as_deref())?;
            out.emit(&document, || render::diff(&document))?;
        }
        command => {
            let (operation, yes) = command
                .into_operation()
                .ok_or_else(|| anyhow::anyhow!("stack command does not define an operation"))?;
            if yes {
                return operate(session, out, GitOperation::Stack(Box::new(operation)));
            }
            out.message(&operation.preview_message())?;
        }
    }
    Ok(0)
}

fn stack_gate(session: &mut Session, out: &Emitter, args: &StackGateArgs) -> Result<u8> {
    let stack = require_stack(session, out, &args.pull_request)?;
    let gate = out
        .execute(
            session,
            Command::PullRequestStackGate {
                stack: Box::new(stack),
                refresh: args.pull_request.refresh,
            },
        )?
        .stack_gate()?;
    out.emit(&gate, || render::stack_gate(&gate))?;
    Ok(if args.no_exit_code {
        0
    } else {
        gate.verdict.exit_code()
    })
}

#[doc = " The whole stack read at once. A single pull request cannot say what"]
#[doc = " can merge now, which one member everything else is waiting on, or"]
#[doc = " where two members touch the same file."]
fn stack_review(session: &mut Session, out: &Emitter, args: &StackReviewArgs) -> Result<u8> {
    let stack = require_stack(session, out, &args.pull_request)?;
    let review = out
        .execute(
            session,
            Command::PullRequestStackReview {
                stack: Box::new(stack),
                incremental: args.incremental,
                refresh: args.pull_request.refresh,
            },
        )?
        .stack_review()?;
    out.emit(&review, || render::stack_review(&review))?;
    Ok(if args.exit_code && !review.is_clear() {
        EXIT_FAILURE
    } else {
        0
    })
}

#[doc = " One queue across the stack, bottom to top. Answering a thread on the"]
#[doc = " bottom member is what lets anything above it move."]
fn stack_feedback(session: &mut Session, out: &Emitter, args: &StackFeedbackArgs) -> Result<u8> {
    let stack = require_stack(session, out, &args.pull_request)?;
    let queue = out
        .execute(
            session,
            Command::PullRequestStackFeedback {
                stack: Box::new(stack),
                refresh: args.pull_request.refresh,
            },
        )?
        .stack_feedback()?;
    let filter = FeedbackFilter {
        blocking_only: args.unresolved,
        mine_only: args.mine,
    };
    let queue = filter.apply_stack(queue);
    out.emit(&queue, || render::stack_feedback(&queue))?;
    Ok(if args.exit_code && queue.counts.blocking > 0 {
        EXIT_FAILURE
    } else {
        0
    })
}

fn require_stack(session: &mut Session, out: &Emitter, args: &PrArgs) -> Result<PullRequestStack> {
    lookup_stack(session, out, args)?.stack.ok_or_else(|| {
        Failure::new(
            EXIT_NOT_FOUND,
            format!("pull request #{} is not part of a stack", args.number),
        )
        .into()
    })
}

fn lookup_stack(
    session: &mut Session,
    out: &Emitter,
    args: &PrArgs,
) -> Result<PullRequestStackSnapshot> {
    let pull_request = lookup(session, out, args)?;
    let snapshot = out
        .execute(
            session,
            Command::PullRequestStack {
                pull_request: Box::new(pull_request),
                refresh: args.refresh,
            },
        )?
        .pull_request_stack()?;
    for warning in &snapshot.warnings {
        out.note(&format!("warning: {warning}"));
    }
    Ok(snapshot)
}

fn stack_range(
    session: &mut Session,
    out: &Emitter,
    args: &StackRangeArgs,
) -> Result<(PullRequestStack, usize, usize)> {
    let stack = require_stack(session, out, &args.pull_request)?;
    let from = args.from.unwrap_or(stack.selected_position);
    let to = args.to.unwrap_or(stack.selected_position);
    drop(stack.comparison(from, to)?);
    Ok((stack, from, to))
}

fn prepare_stack(
    session: &mut Session,
    out: &Emitter,
    stack: PullRequestStack,
    from: usize,
    to: usize,
) -> Result<PullRequestDiffIndex> {
    out.execute(
        session,
        Command::PreparePullRequestStack {
            workspace: 0,
            stack: Box::new(stack),
            from,
            to,
        },
    )?
    .pull_request_index()
}
