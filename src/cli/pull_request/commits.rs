use super::{Command, Emitter, PrArgs, Result, Session, lookup, render};

pub(super) fn commits(session: &mut Session, out: &Emitter, args: &PrArgs) -> Result<u8> {
    let pull_request = lookup(session, out, args)?;
    let commits = out
        .execute(
            session,
            Command::PullRequestCommits {
                pull_request: Box::new(pull_request),
            },
        )?
        .commits()?;
    out.emit(&commits, || render::pull_request_commits(&commits))?;
    Ok(0)
}
