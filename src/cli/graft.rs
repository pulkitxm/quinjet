use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, CommandFactory, Subcommand, ValueHint};
use serde::Serialize;

use super::{
    Cli, Command, EXIT_FAILURE, EXIT_NOT_FOUND, EXIT_UNAVAILABLE, Emitter, Failure, PrArgs,
    Session, ensure_log_available, lookup, render, select_check,
};
use crate::git::Repository;
use crate::graft::call::{CALL_TIMEOUT, GatewayTools, GraftReply};
use crate::graft::gateway::{self, DEFAULT_PORT, INSTALL_HINT, ServePlan};
use crate::graft::receiver::{Access, Receiver, ToolSummary};
use crate::graft::registry::{self, Gateway};

mod calls;
mod install;
mod render_graft;
mod serve;
mod service;

pub(super) use calls::check_log;
pub(crate) use service::{ServiceDocument, ensure_shared_gateway};

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
    #[doc = " Start the shared gateway that `quinjet mcp` and the terminal interface use"]
    Start(GatewayArgs),
    #[doc = " Show whether the shared gateway is running"]
    Status,
    #[doc = " Stop the shared gateway"]
    Stop,
    #[doc = " Download the Graftcode gateway into Quinjet's cache"]
    Install,
    #[command(hide = true)]
    Daemon(GatewayArgs),
}

#[derive(Debug, Args)]
pub(super) struct GatewayArgs {
    #[doc = " Path to the Graftcode gateway executable"]
    #[arg(long, value_name = "PATH", value_hint = ValueHint::ExecutablePath)]
    gateway: Option<PathBuf>,
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
        GraftVerb::Tools(args) => calls::tools(out, repository, args.name.as_deref()),
        GraftVerb::Call(args) => calls::call(out, repository, args),
        GraftVerb::Start(args) => service::start(out, args.gateway.as_deref()),
        GraftVerb::Status => service::status(out),
        GraftVerb::Stop => service::stop(out),
        GraftVerb::Install => service::install_command(out),
        GraftVerb::Daemon(args) => service::daemon(args.gateway.as_deref()),
    }
}

pub(crate) fn receiver(writes: bool) -> Receiver {
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

pub(super) fn selected_gateways(name: Option<&str>) -> Result<Vec<Gateway>> {
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

pub(super) fn session(repository: &Path) -> Result<Session> {
    Ok(Session::new(Repository::discover(repository)?))
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
    let gateway = match gateway::locate_gateway(args.gateway.as_deref()) {
        Ok(path) => path,
        Err(_) if !gateway::gateway_configured(args.gateway.as_deref()) => {
            install::install(&mut |line: &str| super::note(line))?
        }
        Err(error) => {
            return Err(Failure::new(EXIT_FAILURE, format!("{error:#}"))
                .hint(INSTALL_HINT)
                .into());
        }
    };
    Ok(ServePlan {
        gateway,
        module_dir: gateway::module_dir(args.allow_writes)?,
        repository: Some(repository.root().to_path_buf()),
        // nosemgrep: rust.lang.security.current-exe.current-exe
        quinjet: std::env::current_exe()
            .context("could not find the running Quinjet executable")?,
        port: args.port,
        publish: args.publish,
        token: None,
        activity: None,
        log: gateway::log_file(&gateway::module_dir(args.allow_writes)?),
    })
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
