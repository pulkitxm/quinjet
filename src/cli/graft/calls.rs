use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Map;

use super::{
    CallArgs, Command, EXIT_FAILURE, EXIT_NOT_FOUND, EXIT_UNAVAILABLE, Emitter, Failure, PrArgs,
    Session, ensure_log_available, lookup, render, render_graft, select_check, selected_gateways,
    session,
};
use crate::git::Repository;
use crate::git::github::PullRequestCheckStatus;
use crate::graft::call::{GatewayTools, GraftCall};
use crate::graft::context::{self, ContextKey};
use crate::graft::registry::Gateway;

pub(super) fn tools(out: &mut Emitter, repository: &Path, name: Option<&str>) -> Result<u8> {
    let gateways = selected_gateways(name)?;
    out.start_progress("Listing gateway tools")?;
    let listed = list_with_session(out, repository, gateways)?;
    out.emit(&listed, || render_graft::tools(&listed))?;
    let failed = listed.iter().any(|gateway| gateway.error.is_some());
    Ok(if failed { EXIT_UNAVAILABLE } else { 0 })
}

pub(super) fn list_with_session(
    out: &Emitter,
    repository: &Path,
    gateways: Vec<Gateway>,
) -> Result<Vec<GatewayTools>> {
    match Repository::discover(repository) {
        Ok(found) => out
            .execute(&mut Session::new(found), Command::GraftTools(gateways))?
            .graft_tools(),
        Err(_) => Ok(crate::graft::call::list_tools(&gateways)),
    }
}

pub(super) fn call(out: &mut Emitter, repository: &Path, args: CallArgs) -> Result<u8> {
    let gateway = selected_gateways(Some(&args.gateway))?
        .into_iter()
        .next()
        .context("the gateway list was empty")?;
    let mut arguments = Map::new();
    for assignment in &args.arguments {
        let (name, value) = context::parse_assignment(assignment)
            .map_err(|error| Failure::new(EXIT_FAILURE, format!("{error:#}")))?;
        drop(arguments.insert(name, value));
    }
    out.start_progress("Calling gateway tool")?;
    let mut session = session(repository)?;
    let check_log = match args.pull_request {
        Some(number)
            if !args.no_context
                && wants_check_log(out, &mut session, &gateway, &args, &arguments)? =>
        {
            Some(check_log(&mut session, out, number, args.check.as_deref())?)
        }
        _ => None,
    };
    let reply = out
        .execute(
            &mut session,
            Command::GraftCall(Box::new(GraftCall {
                gateway,
                tool: args.tool,
                arguments,
                fill_context: !args.no_context,
                pull_request: args.pull_request,
                check_log,
                timeout: Duration::from_secs(args.timeout.max(1)),
            })),
        )?
        .graft_reply()?;
    if reply.is_error && !out.json {
        return Err(Failure::new(
            EXIT_FAILURE,
            format!("{}.{} failed: {}", reply.gateway, reply.tool, reply.text),
        )
        .into());
    }
    out.emit(&reply, || render_graft::reply(&reply))?;
    Ok(if reply.is_error { EXIT_FAILURE } else { 0 })
}

fn wants_check_log(
    out: &Emitter,
    session: &mut Session,
    gateway: &Gateway,
    args: &CallArgs,
    arguments: &Map<String, serde_json::Value>,
) -> Result<bool> {
    let listed = out
        .execute(session, Command::GraftTools(vec![gateway.clone()]))?
        .graft_tools()?;
    Ok(listed
        .iter()
        .flat_map(|gateway| &gateway.tools)
        .filter(|tool| tool.name == args.tool || fold(&tool.name) == fold(&args.tool))
        .flat_map(|tool| &tool.params)
        .any(|param| {
            !arguments.contains_key(&param.name)
                && context::context_key(&param.name) == Some(ContextKey::CheckLog)
        }))
}

fn fold(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

pub(crate) fn check_log(
    session: &mut Session,
    out: &Emitter,
    number: u64,
    wanted: Option<&str>,
) -> Result<String> {
    let request = lookup(
        session,
        out,
        &PrArgs {
            number,
            repo: None,
            refresh: false,
        },
    )?;
    let listing = out
        .execute(
            session,
            Command::PullRequestChecks {
                commit: None,
                pull_request: Box::new(request.clone()),
                refresh: false,
            },
        )?
        .checks()?;
    let check = match wanted {
        Some(name) => select_check(&listing.checks, name)?,
        None => listing
            .checks
            .iter()
            .find(|check| check.status == PullRequestCheckStatus::Failed)
            .cloned()
            .ok_or_else(|| {
                Failure::new(
                    EXIT_NOT_FOUND,
                    format!("no check has failed on pull request #{number}"),
                )
                .hint("name one with --check")
            })?,
    };
    let log = out
        .execute(
            session,
            Command::CheckRunLog {
                pull_request: Box::new(request),
                check: Box::new(check.clone()),
            },
        )?
        .check_log()?;
    ensure_log_available(&log)?;
    Ok(render::check_log(&check, &log))
}
