use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Args, CommandFactory, Subcommand, ValueHint};
use serde::Serialize;
use serde_json::Map;

use super::{
    Cli, Command, EXIT_FAILURE, EXIT_NOT_FOUND, EXIT_UNAVAILABLE, Emitter, Failure, PrArgs,
    Session, ensure_log_available, lookup, render, select_check,
};
use crate::git::Repository;
use crate::git::github::PullRequestCheckStatus;
use crate::graft::call::{CALL_TIMEOUT, GatewayTools, GraftCall, GraftReply};
use crate::graft::context::{self, ContextKey};
use crate::graft::gateway::{self, DEFAULT_PORT, GatewayProcess, INSTALL_HINT, ServePlan};
use crate::graft::receiver::{Access, Receiver, ToolSummary};
use crate::graft::registry::{self, Gateway};

mod render_graft;
mod serve;

#[derive(Debug, Args)]
pub(super) struct GraftArgs {
    #[command(subcommand)]
    pub(super) command: GraftVerb,
}

#[derive(Debug, Subcommand)]
pub(super) enum GraftVerb {
    #[doc = " Host this repository's Quinjet commands on a Graftcode gateway"]
    Serve(ServeArgs),
    #[doc = " Write the Graftcode receiver module without starting a gateway"]
    Module(ModuleArgs),
    #[doc = " Register a Graftcode gateway under a short name"]
    Add(AddArgs),
    #[doc = " List registered Graftcode gateways"]
    List,
    #[doc = " Forget a registered Graftcode gateway"]
    #[command(visible_alias = "rm")]
    Remove(NameArgs),
    #[doc = " List the tools registered gateways expose"]
    Tools(ToolsArgs),
    #[doc = " Call a gateway tool, filling the repository context it asks for"]
    Call(CallArgs),
}

#[derive(Debug, Args)]
pub(super) struct ServeArgs {
    #[doc = " WebSocket and MCP port; Graftcode uses the next port for its HTTP server"]
    #[arg(long, default_value_t = DEFAULT_PORT, value_hint = ValueHint::Other)]
    port: u16,
    #[doc = " Also expose commands that change the repository or pull requests"]
    #[arg(long)]
    allow_writes: bool,
    #[doc = " Path to the Graftcode gateway executable"]
    #[arg(long, value_name = "PATH", value_hint = ValueHint::ExecutablePath)]
    gateway: Option<PathBuf>,
    #[doc = " Publish method signatures to grft.dev so Vision can install typed Grafts"]
    #[arg(long)]
    publish: bool,
    #[doc = " Register the running gateway under this name"]
    #[arg(long, value_name = "NAME", value_hint = ValueHint::Other)]
    register: Option<String>,
    #[doc = " Seconds to wait for the gateway to start"]
    #[arg(long, value_name = "SECONDS", default_value_t = 180, value_hint = ValueHint::Other)]
    startup_timeout: u64,
}

#[derive(Debug, Args)]
pub(super) struct ModuleArgs {
    #[doc = " Directory to write package.json and index.js into"]
    #[arg(long, value_name = "DIR", value_hint = ValueHint::DirPath)]
    out: PathBuf,
    #[doc = " Also expose commands that change the repository or pull requests"]
    #[arg(long)]
    allow_writes: bool,
}

#[derive(Debug, Args)]
pub(super) struct AddArgs {
    #[doc = " Short name to call the gateway by"]
    #[arg(value_name = "NAME", value_hint = ValueHint::Other)]
    name: String,
    #[doc = " Gateway address, such as `http://localhost:7390`"]
    #[arg(value_name = "URL", value_hint = ValueHint::Url)]
    url: String,
    #[doc = " Environment variable holding a bearer token for the gateway"]
    #[arg(long, value_name = "VARIABLE", value_hint = ValueHint::Other)]
    token_env: Option<String>,
}

#[derive(Debug, Args)]
pub(super) struct NameArgs {
    #[doc = " Registered gateway name"]
    #[arg(value_name = "NAME", value_hint = ValueHint::Other)]
    name: String,
}

#[derive(Debug, Args)]
pub(super) struct ToolsArgs {
    #[doc = " Only this registered gateway"]
    #[arg(value_name = "NAME", value_hint = ValueHint::Other)]
    name: Option<String>,
}

#[derive(Debug, Args)]
pub(super) struct CallArgs {
    #[doc = " Registered gateway name"]
    #[arg(value_name = "GATEWAY", value_hint = ValueHint::Other)]
    gateway: String,
    #[doc = " Tool to call"]
    #[arg(value_name = "TOOL", value_hint = ValueHint::Other)]
    tool: String,
    #[doc = " Tool argument as name=value; JSON numbers and booleans keep their type"]
    #[arg(short, long = "arg", value_name = "NAME=VALUE", value_hint = ValueHint::Other)]
    arguments: Vec<String>,
    #[doc = " Pull request that fills `pull_request` and `check_log` parameters"]
    #[arg(long = "pr", value_name = "NUMBER", value_hint = ValueHint::Other)]
    pull_request: Option<u64>,
    #[doc = " Check whose log fills `check_log`; defaults to the first failing check"]
    #[arg(long, value_name = "NAME", requires = "pull_request", value_hint = ValueHint::Other)]
    check: Option<String>,
    #[doc = " Send only the arguments given, without repository context"]
    #[arg(long)]
    no_context: bool,
    #[doc = " Seconds to wait for the tool to answer"]
    #[arg(long, value_name = "SECONDS", default_value_t = CALL_TIMEOUT.as_secs(), value_hint = ValueHint::Other)]
    timeout: u64,
}

pub(super) fn run(out: &mut Emitter, repository: &Path, command: GraftVerb) -> Result<u8> {
    match command {
        GraftVerb::Serve(args) => serve::serve(out, repository, &args),
        GraftVerb::Module(args) => module(out, &args),
        GraftVerb::Add(args) => add(out, args),
        GraftVerb::List => list(out),
        GraftVerb::Remove(args) => remove(out, &args.name),
        GraftVerb::Tools(args) => tools(out, repository, args.name.as_deref()),
        GraftVerb::Call(args) => call(out, repository, args),
    }
}

pub(super) fn receiver(writes: bool) -> Receiver {
    Receiver::from_command(&Cli::command(), writes)
}

fn module(out: &Emitter, args: &ModuleArgs) -> Result<u8> {
    let receiver = receiver(args.allow_writes);
    gateway::write_module(&args.out, &receiver)?;
    let document = ModuleDocument {
        directory: args.out.display().to_string(),
        tools: receiver.tools(),
    };
    out.emit(&document, || render_graft::module(&document))?;
    Ok(0)
}

fn add(out: &Emitter, args: AddArgs) -> Result<u8> {
    registry::validate_name(&args.name)?;
    let gateway = Gateway {
        name: args.name,
        url: registry::endpoint(&args.url)?,
        token_env: args.token_env,
    };
    let path = registry::registry_path()?;
    let mut gateways = registry::load(&path)?;
    let added = registry::upsert(&mut gateways, gateway.clone());
    registry::save(&path, &gateways)?;
    out.emit(&gateway, || {
        format!(
            "{} gateway `{}` at {}\n",
            if added { "Registered" } else { "Updated" },
            gateway.name,
            gateway.url
        )
    })?;
    Ok(0)
}

fn list(out: &Emitter) -> Result<u8> {
    let gateways = registry::load(&registry::registry_path()?)?;
    out.emit(&gateways, || render_graft::gateways(&gateways))?;
    Ok(0)
}

fn remove(out: &Emitter, name: &str) -> Result<u8> {
    let path = registry::registry_path()?;
    let mut gateways = registry::load(&path)?;
    let removed = registry::find(&gateways, name)
        .map_err(|error| Failure::new(EXIT_NOT_FOUND, format!("{error:#}")))?
        .clone();
    gateways.retain(|gateway| gateway.name != name);
    registry::save(&path, &gateways)?;
    out.emit(&removed, || format!("Forgot gateway `{}`\n", removed.name))?;
    Ok(0)
}

fn selected_gateways(name: Option<&str>) -> Result<Vec<Gateway>> {
    let gateways = registry::load(&registry::registry_path()?)?;
    match name {
        Some(name) => Ok(vec![
            registry::find(&gateways, name)
                .map_err(|error| Failure::new(EXIT_NOT_FOUND, format!("{error:#}")))?
                .clone(),
        ]),
        None if gateways.is_empty() => Err(Failure::new(
            EXIT_NOT_FOUND,
            "no Graftcode gateways are registered",
        )
        .hint("register one with `quinjet graft add <name> <url>`")
        .into()),
        None => Ok(gateways),
    }
}

fn session(repository: &Path) -> Result<Session> {
    Ok(Session::new(Repository::discover(repository)?))
}

fn tools(out: &mut Emitter, repository: &Path, name: Option<&str>) -> Result<u8> {
    let gateways = selected_gateways(name)?;
    out.start_progress("Listing gateway tools")?;
    let listed = list_with_session(out, repository, gateways)?;
    out.emit(&listed, || render_graft::tools(&listed))?;
    let failed = listed.iter().any(|gateway| gateway.error.is_some());
    Ok(if failed { EXIT_UNAVAILABLE } else { 0 })
}

fn list_with_session(
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

fn call(out: &mut Emitter, repository: &Path, args: CallArgs) -> Result<u8> {
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

fn check_log(
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ModuleDocument {
    directory: String,
    tools: Vec<ToolSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ServeDocument {
    repository: String,
    gateway: String,
    mcp: String,
    vision: String,
    websocket: String,
    access: Access,
    published: bool,
    registered: Option<String>,
    tools: Vec<ToolSummary>,
    log: String,
}

pub(super) fn plan(args: &ServeArgs, repository: &Repository) -> Result<ServePlan> {
    let gateway = gateway::locate_gateway(args.gateway.as_deref())
        .map_err(|error| Failure::new(EXIT_FAILURE, format!("{error:#}")).hint(INSTALL_HINT))?;
    Ok(ServePlan {
        gateway,
        module_dir: gateway::module_dir(args.allow_writes)?,
        repository: repository.root().to_path_buf(),
        quinjet: std::env::current_exe()
            .context("could not find the running Quinjet executable")?,
        port: args.port,
        publish: args.publish,
    })
}

pub(super) fn spawn(plan: &ServePlan) -> Result<GatewayProcess> {
    let child = plan
        .command()?
        .spawn()
        .with_context(|| format!("failed to start {}", plan.gateway.display()))?;
    Ok(GatewayProcess(child))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graft::receiver::{
        HOST_ONLY_GROUPS, HOST_ONLY_PATHS, READ_PATHS, access, leaf_paths,
    };

    #[test]
    fn every_listed_path_names_a_real_command() {
        let mut root = Cli::command();
        root.build();
        let paths: Vec<String> = leaf_paths(&root)
            .into_iter()
            .map(|(path, _)| path.join(" "))
            .collect();
        for listed in READ_PATHS.iter().chain(HOST_ONLY_PATHS) {
            assert!(
                paths.iter().any(|path| path == listed),
                "{listed} is not a command"
            );
        }
        for group in HOST_ONLY_GROUPS {
            assert!(
                paths.iter().any(|path| path.starts_with(group)),
                "{group} has no commands"
            );
        }
    }

    #[test]
    fn read_tools_never_take_a_confirmation() {
        for method in receiver(false).methods {
            assert_eq!(access(&method.path.join(" ")), Some(Access::Read));
            assert!(
                method.params.iter().all(|param| param.name != "yes"),
                "{} is listed as a read but confirms",
                method.name
            );
        }
    }

    #[test]
    fn method_names_are_unique() {
        let receiver = receiver(true);
        let mut names: Vec<&str> = receiver
            .methods
            .iter()
            .map(|method| method.name.as_str())
            .collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count);
    }
}
