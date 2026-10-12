use anyhow::{Context, bail};

use super::{Command, Emitter, PrArgs, Result, Session, lookup, render};
use crate::git::github::PullRequest;

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

pub(super) fn selected_commit(
    session: &mut Session,
    out: &Emitter,
    request: &PullRequest,
    hash: Option<&str>,
) -> Result<Option<String>> {
    let Some(hash) = hash else {
        return Ok(None);
    };
    if hash.len() < 7 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("Use at least seven hexadecimal characters from a PR commit hash");
    }
    let commits = out
        .execute(
            session,
            Command::PullRequestCommits {
                pull_request: Box::new(request.clone()),
            },
        )?
        .commits()?;
    let mut matches = commits
        .commits
        .iter()
        .filter(|commit| commit.oid.starts_with(&hash.to_ascii_lowercase()));
    let commit = matches
        .next()
        .context("The hash does not match a loaded PR commit")?;
    if matches.next().is_some() {
        bail!("The commit hash is ambiguous; use a longer hash");
    }
    Ok(Some(commit.oid.clone()))
}
