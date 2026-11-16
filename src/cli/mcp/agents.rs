use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use serde::Serialize;
use serde_json::{Map, Value, json};

use super::Emitter;

const SERVER_NAME: &str = "quinjet";

#[derive(Debug, Args)]
pub(crate) struct InstallArgs {
    #[doc = " Only connect these agents"]
    #[arg(long = "agent", value_enum, value_name = "AGENT")]
    agents: Vec<Agent>,
    #[doc = " Print the configuration instead of writing it"]
    #[arg(long)]
    print: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Agent {
    ClaudeCode,
    Codex,
    Cursor,
    VsCode,
    Windsurf,
    GeminiCli,
}

impl Agent {
    pub(crate) const ALL: [Self; 6] = [
        Self::ClaudeCode,
        Self::Codex,
        Self::Cursor,
        Self::VsCode,
        Self::Windsurf,
        Self::GeminiCli,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
            Self::Cursor => "Cursor",
            Self::VsCode => "VS Code",
            Self::Windsurf => "Windsurf",
            Self::GeminiCli => "Gemini CLI",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Outcome {
    Added,
    Updated,
    Unchanged,
    NotInstalled,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Connection {
    pub agent: Agent,
    pub outcome: Outcome,
    pub location: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Launcher {
    pub command: String,
    pub args: Vec<String>,
}

pub(super) fn run(out: &Emitter, args: &InstallArgs) -> Result<u8> {
    let launcher = launcher()?;
    let home = home_dir().context("could not find the home directory")?;
    let agents: Vec<Agent> = if args.agents.is_empty() {
        Agent::ALL.to_vec()
    } else {
        args.agents.clone()
    };
    if args.print {
        let document = json!({
            "command": launcher.command,
            "args": launcher.args,
            "mcpServers": { SERVER_NAME: json_entry(Agent::Cursor, &launcher) },
        });
        out.emit(&document, || {
            format!(
                "{}\n",
                serde_json::to_string_pretty(&document).unwrap_or_default()
            )
        })?;
        return Ok(0);
    }
    let connections = connect(&home, &launcher, &agents, !args.agents.is_empty());
    out.emit(&connections, || render(&connections))?;
    Ok(u8::from(
        connections
            .iter()
            .any(|connection| connection.outcome == Outcome::Failed),
    ))
}

pub(crate) fn connect_all() -> Result<Vec<Connection>> {
    let launcher = launcher()?;
    let home = home_dir().context("could not find the home directory")?;
    Ok(connect(&home, &launcher, &Agent::ALL, false))
}

fn connect(home: &Path, launcher: &Launcher, agents: &[Agent], named: bool) -> Vec<Connection> {
    agents
        .iter()
        .map(|agent| connect_one(home, launcher, *agent, named))
        .filter(|connection| named || connection.outcome != Outcome::NotInstalled)
        .collect()
}

fn connect_one(home: &Path, launcher: &Launcher, agent: Agent, named: bool) -> Connection {
    let result = match agent {
        Agent::ClaudeCode => claude_code(launcher),
        Agent::Codex => {
            let folder = home.join(".codex");
            installed(&folder, "codex", named).map(|()| {
                let path = folder.join("config.toml");
                (toml_file(&path, launcher), path.display().to_string())
            })
        }
        json_agent => json_location(home, json_agent)
            .filter(|(folder, _)| folder.is_dir() || named)
            .map(|(_, path)| {
                (
                    json_file(&path, json_agent, launcher),
                    path.display().to_string(),
                )
            }),
    };
    match result {
        None => Connection {
            agent,
            outcome: Outcome::NotInstalled,
            location: String::new(),
            detail: None,
        },
        Some((Ok(outcome), location)) => Connection {
            agent,
            outcome,
            location,
            detail: None,
        },
        Some((Err(error), location)) => Connection {
            agent,
            outcome: Outcome::Failed,
            location,
            detail: Some(format!("{error:#}")),
        },
    }
}

fn installed(folder: &Path, program: &str, named: bool) -> Option<()> {
    (named || folder.is_dir() || on_path(program).is_some()).then_some(())
}

fn json_location(home: &Path, agent: Agent) -> Option<(PathBuf, PathBuf)> {
    let folder = match agent {
        Agent::Cursor => home.join(".cursor"),
        Agent::Windsurf => home.join(".codeium").join("windsurf"),
        Agent::GeminiCli => home.join(".gemini"),
        Agent::VsCode => vs_code_user(home)?,
        Agent::ClaudeCode | Agent::Codex => return None,
    };
    let file = match agent {
        Agent::Windsurf => "mcp_config.json",
        Agent::GeminiCli => "settings.json",
        _ => "mcp.json",
    };
    Some((folder.clone(), folder.join(file)))
}

fn vs_code_user(home: &Path) -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        return Some(
            home.join("Library")
                .join("Application Support")
                .join("Code")
                .join("User"),
        );
    }
    if cfg!(windows) {
        return std::env::var_os("APPDATA")
            .map(|path| PathBuf::from(path).join("Code").join("User"));
    }
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|path| !path.is_empty())
        .map_or_else(|| home.join(".config"), PathBuf::from);
    Some(config.join("Code").join("User"))
}

pub(crate) fn json_entry(agent: Agent, launcher: &Launcher) -> Value {
    if agent == Agent::VsCode {
        json!({ "type": "stdio", "command": launcher.command, "args": launcher.args })
    } else {
        json!({ "command": launcher.command, "args": launcher.args })
    }
}

pub(crate) fn json_file(path: &Path, agent: Agent, launcher: &Launcher) -> Result<Outcome> {
    let mut document = match fs::read_to_string(path) {
        Ok(text) if !text.trim().is_empty() => {
            serde_json::from_str::<Value>(&text).with_context(|| {
                format!(
                    "{} is not plain JSON; add the `{SERVER_NAME}` server by hand",
                    path.display()
                )
            })?
        }
        _ => Value::Object(Map::new()),
    };
    let section = if agent == Agent::VsCode {
        "servers"
    } else {
        "mcpServers"
    };
    let root = document
        .as_object_mut()
        .with_context(|| format!("{} does not hold a JSON object", path.display()))?;
    let servers = root
        .entry(section)
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .with_context(|| format!("`{section}` in {} is not an object", path.display()))?;
    let entry = json_entry(agent, launcher);
    let outcome = match servers.get(SERVER_NAME) {
        Some(existing) if *existing == entry => return Ok(Outcome::Unchanged),
        Some(_) => Outcome::Updated,
        None => Outcome::Added,
    };
    drop(servers.insert(SERVER_NAME.to_owned(), entry));
    write(path, &(serde_json::to_string_pretty(&document)? + "\n"))?;
    Ok(outcome)
}

pub(crate) fn toml_file(path: &Path, launcher: &Launcher) -> Result<Outcome> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    let header = format!("[mcp_servers.{SERVER_NAME}]");
    let quoted = |text: &str| format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""));
    let section = format!(
        "{header}\ncommand = {}\nargs = [{}]\n",
        quoted(&launcher.command),
        launcher
            .args
            .iter()
            .map(|arg| quoted(arg))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let lines: Vec<&str> = existing.lines().collect();
    let (text, outcome) = if let Some(start) = lines.iter().position(|line| line.trim() == header) {
        {
            let end = lines
                .iter()
                .enumerate()
                .skip(start.saturating_add(1))
                .find(|(_, line)| line.trim_start().starts_with('['))
                .map_or(lines.len(), |(index, _)| index);
            let current = lines.get(start..end).unwrap_or_default().join("\n");
            if current.trim_end() == section.trim_end() {
                return Ok(Outcome::Unchanged);
            }
            let mut text = lines.get(..start).unwrap_or_default().join("\n");
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&section);
            let rest = lines.get(end..).unwrap_or_default().join("\n");
            if !rest.is_empty() {
                text.push('\n');
                text.push_str(&rest);
                text.push('\n');
            }
            (text, Outcome::Updated)
        }
    } else {
        {
            let mut text = existing.clone();
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&section);
            (text, Outcome::Added)
        }
    };
    write(path, &text)?;
    Ok(outcome)
}

fn claude_code(launcher: &Launcher) -> Option<(Result<Outcome>, String)> {
    let claude = on_path("claude")?;
    let location = "claude mcp (user scope)".to_owned();
    let run = |args: &[&str]| {
        std::process::Command::new(&claude)
            .args(args)
            .stdin(Stdio::null())
            .output()
    };
    let existing = run(&["mcp", "get", SERVER_NAME]).ok();
    let present = existing
        .as_ref()
        .is_some_and(|output| output.status.success());
    if present
        && existing.as_ref().is_some_and(|output| {
            String::from_utf8_lossy(&output.stdout).contains(&launcher.command)
        })
    {
        return Some((Ok(Outcome::Unchanged), location));
    }
    if present {
        drop(run(&["mcp", "remove", "--scope", "user", SERVER_NAME]));
    }
    let mut add = vec![
        "mcp",
        "add",
        "--scope",
        "user",
        SERVER_NAME,
        "--",
        &launcher.command,
    ];
    add.extend(launcher.args.iter().map(String::as_str));
    let result = run(&add)
        .context("failed to run claude")
        .and_then(|output| {
            if output.status.success() {
                Ok(if present {
                    Outcome::Updated
                } else {
                    Outcome::Added
                })
            } else {
                Err(anyhow::anyhow!(
                    "claude mcp add failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ))
            }
        });
    Some((result, location))
}

fn write(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, text).with_context(|| format!("failed to write {}", path.display()))
}

pub(crate) fn launcher() -> Result<Launcher> {
    // nosemgrep: rust.lang.security.current-exe.current-exe
    let running = std::env::current_exe().context("could not find the running Quinjet")?;
    let canonical = fs::canonicalize(&running).unwrap_or_else(|_| running.clone());
    let command = on_path("quinjet")
        .filter(|found| fs::canonicalize(found).is_ok_and(|found| found == canonical))
        .map_or_else(|| running.display().to_string(), |_| "quinjet".to_owned());
    Ok(Launcher {
        command,
        args: vec!["mcp".to_owned()],
    })
}

fn on_path(program: &str) -> Option<PathBuf> {
    let names: Vec<String> = if cfg!(windows) {
        vec![
            format!("{program}.exe"),
            format!("{program}.cmd"),
            program.to_owned(),
        ]
    } else {
        vec![program.to_owned()]
    };
    std::env::var_os("PATH")
        .iter()
        .flat_map(std::env::split_paths)
        .flat_map(|directory| names.iter().map(move |name| directory.join(name)))
        .find(|candidate| candidate.is_file())
}

fn home_dir() -> Option<PathBuf> {
    ["HOME", "USERPROFILE"]
        .iter()
        .filter_map(std::env::var_os)
        .find(|path| !path.is_empty())
        .map(PathBuf::from)
}

pub(crate) fn render(connections: &[Connection]) -> String {
    if connections.is_empty() {
        return "No supported AI agents were found. Run `quinjet mcp install --print` for the configuration to add by hand.\n".to_owned();
    }
    let mut lines = Vec::new();
    for connection in connections {
        let status = match connection.outcome {
            Outcome::Added => "connected",
            Outcome::Updated => "updated",
            Outcome::Unchanged => "already connected",
            Outcome::NotInstalled => "not installed",
            Outcome::Failed => "failed",
        };
        let line = format!("{:<12} {status:<17}", connection.agent.label());
        lines.push(if connection.location.is_empty() {
            line.trim_end().to_owned()
        } else {
            format!("{line}  {}", tilde(&connection.location))
        });
        if let Some(detail) = &connection.detail {
            lines.push(format!("             {detail}"));
        }
    }
    lines.join("\n") + "\n"
}

fn tilde(location: &str) -> String {
    home_dir()
        .and_then(|home| {
            Path::new(location)
                .strip_prefix(&home)
                .ok()
                .map(|rest| format!("~/{}", rest.display()))
        })
        .unwrap_or_else(|| location.to_owned())
}

#[cfg(test)]
#[path = "agents_tests.rs"]
mod tests;
