use anyhow::{Context, bail};

use super::{Command, Emitter, PrCommitsArgs, Result, Session, lookup, render};
use crate::git::github::PullRequest;

pub(super) fn commits(session: &mut Session, out: &Emitter, args: &PrCommitsArgs) -> Result<u8> {
    let pull_request = lookup(session, out, &args.pull_request)?;
    let commits = out
        .execute(
            session,
            Command::PullRequestCommits {
                introduced_only: args.introduced_only,
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
        bail!("Use at least seven hexadecimal characters from a commit hash");
    }
    let commits = out
        .execute(
            session,
            Command::PullRequestCommits {
                introduced_only: false,
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
        .context("The hash does not match a commit in the PR head history")?;
    if matches.next().is_some() {
        bail!("The commit hash is ambiguous; use a longer hash");
    }
    Ok(Some(commit.oid.clone()))
}
