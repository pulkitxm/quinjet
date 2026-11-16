use std::io::{self, BufRead as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueHint};
use serde_json::{Map, Value, json};

use super::{Emitter, graft};
use crate::git::Repository;
use crate::graft::call::CALL_TIMEOUT;
use crate::graft::client::McpSession;
use crate::graft::receiver::{REPO_PARAM, Receiver, TOKEN_PARAM};
use crate::graft::schema::{REPOSITORY_PROPERTY, mcp_tool};
use crate::graft::service::{Lease, ServicePaths, ServiceState};

mod agents;

pub(crate) use agents::{connect_all as connect_agents, render as render_connections};

const PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];
const INSTRUCTIONS: &str = "Quinjet reads the Git repository and the GitHub pull requests of the folder you are working in. \
Every tool answers with the same JSON as the matching `quinjet <command> --json`. \
Pass `repository` to use another checkout. \
Tools whose names start with `ai` ask Quinjet's own Claude model and need an Anthropic API key, set with `quinjet ai key`.";

#[derive(Debug, Args)]
pub(super) struct McpArgs {
    #[command(subcommand)]
    command: Option<McpVerb>,
    #[doc = " Path to the Graftcode gateway executable"]
    #[arg(long, value_name = "PATH", value_hint = ValueHint::ExecutablePath)]
    gateway: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum McpVerb {
    #[doc = " Add `quinjet mcp` to the AI agents installed on this machine"]
    Install(agents::InstallArgs),
}

pub(super) fn run(out: &Emitter, repository: &Path, args: McpArgs) -> Result<u8> {
    match args.command {
        Some(McpVerb::Install(install)) => agents::run(out, &install),
        None => serve(repository, args.gateway),
    }
}

struct Bridge {
    folder: PathBuf,
    gateway: Option<PathBuf>,
    receiver: Receiver,
    service: Mutex<Option<ServiceState>>,
    lease: Mutex<Option<Lease>>,
    stdout: Mutex<io::Stdout>,
}

fn serve(repository: &Path, gateway: Option<PathBuf>) -> Result<u8> {
    let folder = std::path::absolute(repository).unwrap_or_else(|_| repository.to_path_buf());
    let bridge = Arc::new(Bridge {
        folder,
        gateway,
        receiver: graft::receiver(false),
        service: Mutex::new(None),
        lease: Mutex::new(None),
        stdout: Mutex::new(io::stdout()),
    });
    {
        let bridge = Arc::clone(&bridge);
        drop(
            thread::Builder::new()
                .name("quinjet-mcp-warmup".to_owned())
                .spawn(move || drop(bridge.service())),
        );
    }
    for line in io::stdin().lock().lines() {
        let line = line.context("failed to read from the MCP client")?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            bridge.send(&json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": { "code": -32700, "message": "the message was not JSON" },
            }))?;
            continue;
        };
        let (Some(id), Some(method)) = (
            message.get("id").cloned(),
            message.get("method").and_then(Value::as_str),
        ) else {
            continue;
        };
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        if method == "tools/call" {
            let bridge = Arc::clone(&bridge);
            drop(
                thread::Builder::new()
                    .name("quinjet-mcp-call".to_owned())
                    .spawn(move || {
                        let result = bridge.call(&params);
                        drop(bridge.send(&json!({ "jsonrpc": "2.0", "id": id, "result": result })));
                    })
                    .context("failed to start an MCP call")?,
            );
            continue;
        }
        let reply = bridge.answer(method, &params).map_or_else(
            || {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": format!("Quinjet does not handle {method}") },
                })
            },
            |result| json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        );
        bridge.send(&reply)?;
    }
    Ok(0)
}

impl Bridge {
    fn send(&self, message: &Value) -> Result<()> {
        let line = serde_json::to_string(message)?;
        let mut stdout = self.stdout.lock().unwrap_or_else(PoisonError::into_inner);
        writeln!(stdout, "{line}")?;
        stdout.flush()?;
        drop(stdout);
        Ok(())
    }

    fn answer(&self, method: &str, params: &Value) -> Option<Value> {
        Some(match method {
            "initialize" => {
                let requested = params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let version = PROTOCOL_VERSIONS
                    .iter()
                    .find(|version| **version == requested)
                    .or_else(|| PROTOCOL_VERSIONS.first())
                    .copied()
                    .unwrap_or_default();
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "quinjet", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": INSTRUCTIONS,
                })
            }
            "ping" => json!({}),
            "tools/list" => {
                json!({ "tools": self.receiver.methods.iter().map(mcp_tool).collect::<Vec<_>>() })
            }
            "resources/list" => json!({ "resources": [] }),
            "resources/templates/list" => json!({ "resourceTemplates": [] }),
            "prompts/list" => json!({ "prompts": [] }),
            _ => return None,
        })
    }

    fn service(&self) -> Result<ServiceState> {
        let cached = self
            .service
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(state) = cached.filter(ServiceState::healthy) {
            return Ok(state);
        }
        let state = graft::ensure_shared_gateway(self.gateway.as_deref(), true, &mut |line| {
            super::note(line);
        })?;
        let mut lease = self.lease.lock().unwrap_or_else(PoisonError::into_inner);
        if lease.is_none() {
            *lease = ServicePaths::locate().and_then(|paths| paths.lease()).ok();
        }
        drop(lease);
        *self.service.lock().unwrap_or_else(PoisonError::into_inner) = Some(state.clone());
        Ok(state)
    }

    fn call(&self, params: &Value) -> Value {
        match self.forward(params) {
            Ok(result) => result,
            Err(error) => json!({
                "content": [{ "type": "text", "text": format!("{error:#}") }],
                "isError": true,
            }),
        }
    }

    fn forward(&self, params: &Value) -> Result<Value> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let method = self
            .receiver
            .find(name)
            .with_context(|| format!("Quinjet has no tool called `{name}`"))?;
        let given = params
            .get("arguments")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let folder = given
            .get(REPOSITORY_PROPERTY)
            .and_then(Value::as_str)
            .filter(|path| !path.trim().is_empty())
            .map_or_else(|| self.folder.clone(), PathBuf::from);
        let root = Repository::discover(&folder)
            .with_context(|| {
                format!(
                    "{} is not inside a Git repository; pass `{REPOSITORY_PROPERTY}`",
                    folder.display()
                )
            })?
            .root()
            .to_path_buf();
        let mut attempt = 0;
        loop {
            let state = self.service()?;
            let mut arguments = Map::new();
            for param in &method.params {
                let value = given
                    .get(&param.name)
                    .cloned()
                    .unwrap_or_else(|| Value::String(String::new()));
                drop(arguments.insert(param.name.clone(), value));
            }
            drop(arguments.insert(
                REPO_PARAM.to_owned(),
                Value::String(root.display().to_string()),
            ));
            drop(arguments.insert(TOKEN_PARAM.to_owned(), Value::String(state.token.clone())));
            match McpSession::connect(&state.mcp_url(), None, CALL_TIMEOUT)
                .and_then(|mut session| session.call_tool(&method.name, arguments))
            {
                Ok(reply) => {
                    return Ok(json!({
                        "content": [{ "type": "text", "text": reply.text }],
                        "isError": reply.is_error,
                    }));
                }
                Err(_) if attempt == 0 && !state.healthy() => {
                    attempt += 1;
                    *self.service.lock().unwrap_or_else(PoisonError::into_inner) = None;
                }
                Err(error) => return Err(error),
            }
        }
    }
}
