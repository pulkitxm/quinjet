use super::{GatewayTools, GraftReply, ModuleDocument, ServeDocument, ServiceDocument};
use crate::graft::context::{ContextKey, context_key};
use crate::graft::receiver::Access;
use crate::graft::registry::Gateway;

fn joined(lines: &[String]) -> String {
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

pub(super) fn module(document: &ModuleDocument) -> String {
    let writes = document
        .tools
        .iter()
        .filter(|tool| tool.access == Access::Write)
        .count();
    joined(&[
        format!("Wrote the Quinjet receiver to {}", document.directory),
        format!(
            "  {} methods, {writes} of them change the repository",
            document.tools.len()
        ),
        format!(
            "  host it with: gg --runtime nodejs --modules {}/index.js",
            document.directory
        ),
    ])
}

pub(super) fn serving(document: &ServeDocument) -> String {
    let access = if document.access == Access::Write {
        "reads and writes; writes still preview until yes is true"
    } else {
        "read-only; pass --allow-writes for mutations"
    };
    let mut lines = vec![
        format!("Quinjet is grafted for {}", document.repository),
        String::new(),
        format!("  MCP        {}", document.mcp),
        format!("  Vision     {}", document.vision),
        format!("  WebSocket  {}", document.websocket),
        format!("  Tools      {} ({access})", document.tools.len()),
    ];
    if let Some(name) = &document.registered {
        lines.push(format!("  Registered `{name}` for quinjet graft call"));
    }
    if document.published {
        lines.push("  Published  method signatures went to grft.dev for typed Grafts".to_owned());
    }
    lines.extend([
        format!("  Log        {}", document.log),
        String::new(),
        "Add it to an MCP client:".to_owned(),
        format!(
            "  {{\"mcpServers\": {{\"quinjet\": {{\"url\": \"{}\"}}}}}}",
            document.mcp
        ),
        String::new(),
        "The gateway listens on every interface. Press Ctrl-C to stop it.".to_owned(),
    ]);
    joined(&lines)
}

pub(super) fn service(document: &ServiceDocument) -> String {
    let (Some(mcp), Some(vision), Some(websocket)) = (
        document.mcp.as_deref(),
        document.vision.as_deref(),
        document.websocket.as_deref(),
    ) else {
        return joined(&[
            "The shared Graftcode gateway is not running".to_owned(),
            "  It starts by itself when an agent runs `quinjet mcp` or Quinjet opens".to_owned(),
        ]);
    };
    joined(&[
        "The shared Graftcode gateway is running".to_owned(),
        String::new(),
        format!("  MCP        {mcp} (through `quinjet mcp`)"),
        format!("  Vision     {vision}"),
        format!("  WebSocket  {websocket}"),
        format!("  Tools      {}", document.tools),
        format!("  Clients    {}", document.clients),
        format!(
            "  Log        {}",
            document.log.as_deref().unwrap_or_default()
        ),
    ])
}

pub(super) fn gateways(gateways: &[Gateway]) -> String {
    if gateways.is_empty() {
        return "No gateways are registered. Add one with `quinjet graft add <name> <url>`.\n"
            .to_owned();
    }
    let width = gateways
        .iter()
        .map(|gateway| gateway.name.len())
        .max()
        .unwrap_or_default();
    let lines: Vec<String> = gateways
        .iter()
        .map(|gateway| {
            let token = gateway
                .token_env
                .as_ref()
                .map(|variable| format!("  (token from ${variable})"))
                .unwrap_or_default();
            format!("{:width$}  {}{token}", gateway.name, gateway.url)
        })
        .collect();
    joined(&lines)
}

pub(super) fn tools(listed: &[GatewayTools]) -> String {
    let mut lines = Vec::new();
    for gateway in listed {
        lines.push(format!("{}  {}", gateway.gateway, gateway.url));
        if let Some(error) = &gateway.error {
            lines.push(format!("  unavailable: {error}"));
        }
        for tool in &gateway.tools {
            let params: Vec<String> = tool
                .params
                .iter()
                .map(|param| {
                    let marker = if context_key(&param.name).is_some() {
                        "*"
                    } else if param.required {
                        ""
                    } else {
                        "?"
                    };
                    format!("{}{marker}", param.name)
                })
                .collect();
            let description = tool
                .description
                .as_deref()
                .map(|text| format!("  {}", text.lines().next().unwrap_or_default().trim()))
                .unwrap_or_default();
            lines.push(format!(
                "  {}({}){description}",
                tool.name,
                params.join(", ")
            ));
        }
        lines.push(String::new());
    }
    lines.push("* Quinjet fills this parameter from the repository. ? optional.".to_owned());
    lines.push("Context parameters:".to_owned());
    lines.extend(
        ContextKey::ALL
            .iter()
            .map(|key| format!("  {:15} {}", key.canonical(), key.description())),
    );
    joined(&lines)
}

pub(super) fn reply(reply: &GraftReply) -> String {
    let mut text = reply.text.clone();
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}
