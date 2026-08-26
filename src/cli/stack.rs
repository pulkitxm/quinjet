use super::stack_verbs::StackRangeArgs;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn stack(session: &mut Session, out: &Emitter, command: StackVerb) -> Result<u8> {
    match command {
        StackVerb::View(args) => {
            let snapshot = lookup_stack(session, out, &args)?;
            out.emit(&snapshot, || render::pull_request_stack(&snapshot))?;
            Ok(0)
        }
        StackVerb::Files(args) => {
            let (stack, from, to) = stack_range(session, out, &args)?;
            let index = prepare_stack(session, out, stack, from, to)?;
            out.emit(&index, || render::pull_request_files(&index))?;
            Ok(0)
        }
        StackVerb::Diff(args) => {
            let (stack, from, to) = stack_range(session, out, &args.range)?;
            let title = format!("Stack #{} positions {from} through {to}", stack.number);
            let index = prepare_stack(session, out, stack, from, to)?;
            let document =
                prepared_pull_request_diff(session, out, &index, title, args.path.as_deref())?;
            out.emit(&document, || render::diff(&document))?;
            Ok(0)
        }
    }
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
    let snapshot = lookup_stack(session, out, &args.pull_request)?;
    let stack = snapshot.stack.ok_or_else(|| {
        Failure::new(
            EXIT_NOT_FOUND,
            format!(
                "pull request #{} is not part of a stack",
                args.pull_request.number
            ),
        )
    })?;
    let from = args.from.unwrap_or(1);
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
