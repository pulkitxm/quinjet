use super::stack_verbs::{StackGateArgs, StackRangeArgs};
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
