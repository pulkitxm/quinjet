use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use serde_json::{Map, Value, json};

use super::http;

pub(crate) const PROTOCOL_VERSION: &str = "2025-06-18";
const SESSION_HEADER: &str = "mcp-session-id";
const RESPONSE_LIMIT: usize = 32 * 1024 * 1024;
const MAX_TOOL_PAGES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolParam {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Tool {
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub params: Vec<ToolParam>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolReply {
    pub text: String,
    pub is_error: bool,
    pub structured: Option<Value>,
}

pub(crate) struct McpSession {
    timeout: Duration,
    url: String,
    token: Option<String>,
    session: Option<String>,
    next_id: u64,
}

impl McpSession {
    pub(crate) fn connect(url: &str, token: Option<String>, timeout: Duration) -> Result<Self> {
        let mut session = Self {
            timeout,
            url: url.to_owned(),
            token,
            session: None,
            next_id: 0,
        };
        drop(session.request(
            "initialize",
            &json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "quinjet", "version": env!("CARGO_PKG_VERSION")},
            }),
        )?);
        drop(session.post(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))?);
        Ok(session)
    }

    pub(crate) fn list_tools(&mut self) -> Result<Vec<Tool>> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_TOOL_PAGES {
            let params = cursor
                .as_ref()
                .map_or_else(|| json!({}), |cursor| json!({"cursor": cursor}));
            let page = self.request("tools/list", &params)?;
            if let Some(listed) = page.get("tools").and_then(Value::as_array) {
                tools.extend(listed.iter().filter_map(parse_tool));
            }
            cursor = page
                .get("nextCursor")
                .and_then(Value::as_str)
                .filter(|next| !next.is_empty())
                .map(str::to_owned);
            if cursor.is_none() {
                break;
            }
        }
        Ok(tools)
    }

    pub(crate) fn call_tool(
        &mut self,
        name: &str,
        arguments: Map<String, Value>,
    ) -> Result<ToolReply> {
        let result = self.request(
            "tools/call",
            &json!({"name": name, "arguments": Value::Object(arguments)}),
        )?;
        Ok(parse_reply(&result))
    }

    fn request(&mut self, method: &str, params: &Value) -> Result<Value> {
        self.next_id = self.next_id.wrapping_add(1);
        let id = self.next_id;
        let body = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let text = self
            .post(&body)?
            .with_context(|| format!("the gateway sent no answer to {method}"))?;
        let message = find_response(&text, id)
            .with_context(|| format!("the gateway answered {method} with no JSON-RPC response"))?;
        if let Some(error) = message.get("error") {
            let code = error
                .get("code")
                .and_then(Value::as_i64)
                .unwrap_or_default();
            let detail = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            bail!("{method} failed with {code}: {detail}");
        }
        message
            .get("result")
            .cloned()
            .ok_or_else(|| anyhow!("the gateway answered {method} without a result"))
    }

    fn post(&mut self, body: &Value) -> Result<Option<String>> {
        let bearer = self.token.as_ref().map(|token| format!("Bearer {token}"));
        let mut headers = vec![
            ("Content-Type", "application/json"),
            ("Accept", "application/json, text/event-stream"),
            ("MCP-Protocol-Version", PROTOCOL_VERSION),
        ];
        if let Some(session) = &self.session {
            headers.push((SESSION_HEADER, session.as_str()));
        }
        if let Some(bearer) = &bearer {
            headers.push(("Authorization", bearer.as_str()));
        }
        let response = http::request(
            "POST",
            &self.url,
            &headers,
            serde_json::to_string(body)?.as_bytes(),
            self.timeout,
            RESPONSE_LIMIT,
        )
        .with_context(|| format!("could not reach the gateway at {}", self.url))?;
        if let Some(session) = response.header(SESSION_HEADER) {
            self.session = Some(session.to_owned());
        }
        let status = response.status;
        let text = String::from_utf8_lossy(&response.body).into_owned();
        if !(200..300).contains(&status) {
            let detail = text.trim();
            bail!(
                "the gateway at {} answered HTTP {status}{}{}",
                self.url,
                if detail.is_empty() { "" } else { ": " },
                detail.chars().take(400).collect::<String>()
            );
        }
        Ok((!text.trim().is_empty()).then_some(text))
    }
}

pub(crate) fn find_response(text: &str, id: u64) -> Option<Value> {
    let trimmed = text.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return serde_json::from_str::<Value>(trimmed)
            .ok()
            .and_then(|value| matching(value, id));
    }
    let mut data = String::new();
    for line in text.lines().chain(std::iter::once("")) {
        if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
        } else if line.trim().is_empty() && !data.is_empty() {
            let found = serde_json::from_str::<Value>(&data)
                .ok()
                .and_then(|value| matching(value, id));
            if found.is_some() {
                return found;
            }
            data.clear();
        }
    }
    None
}

fn matching(value: Value, id: u64) -> Option<Value> {
    match value {
        Value::Array(messages) => messages.into_iter().find_map(|value| matching(value, id)),
        Value::Object(_) if value.get("id").and_then(Value::as_u64) == Some(id) => Some(value),
        _ => None,
    }
}

pub(crate) fn parse_tool(value: &Value) -> Option<Tool> {
    let name = value.get("name")?.as_str()?.to_owned();
    let schema = value.get("inputSchema");
    let required: Vec<&str> = schema
        .and_then(|schema| schema.get("required"))
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let params = schema
        .and_then(|schema| schema.get("properties"))
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .iter()
                .map(|(name, property)| ToolParam {
                    name: name.clone(),
                    description: text_field(property, "description"),
                    required: required.contains(&name.as_str()),
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Tool {
        name,
        title: text_field(value, "title"),
        description: text_field(value, "description"),
        params,
    })
}

fn text_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

pub(crate) fn parse_reply(result: &Value) -> ToolReply {
    let structured = result.get("structuredContent").cloned();
    let content: Vec<String> = result
        .get("content")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("text").and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let text = match structured.as_ref().and_then(|value| value.get("value")) {
        Some(Value::String(text)) => text.clone(),
        Some(other) if content.is_empty() => other.to_string(),
        _ => content
            .iter()
            .map(|text| unquote(text))
            .collect::<Vec<_>>()
            .join("\n"),
    };
    let is_error = result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or_default();
    ToolReply {
        text: if is_error { clean_error(&text) } else { text },
        is_error,
        structured,
    }
}

fn is_gateway_frame(line: &str) -> bool {
    line.split_once("() in ").is_some_and(|(call, location)| {
        !call.trim_start_matches("async ").contains(' ')
            && location
                .rsplit_once(" line ")
                .is_some_and(|(_, number)| number.chars().all(|digit| digit.is_ascii_digit()))
    })
}

fn clean_error(text: &str) -> String {
    let mut lines: Vec<&str> = text
        .lines()
        .filter(|line| !is_gateway_frame(line))
        .collect();
    if let Some(first) = lines.first_mut() {
        while let Some(rest) = first.strip_prefix("Error: ") {
            *first = rest;
        }
    }
    lines.join("\n").trim().to_owned()
}

fn unquote(text: &str) -> String {
    serde_json::from_str::<String>(text).unwrap_or_else(|_| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_json_responses_are_matched_by_id() {
        let text = r#"{"jsonrpc":"2.0","id":3,"result":{"ok":true}}"#;
        assert_eq!(
            find_response(text, 3).and_then(|value| value.get("result").cloned()),
            Some(json!({"ok": true}))
        );
        assert!(find_response(text, 4).is_none());
    }

    #[test]
    fn event_stream_responses_skip_notifications() {
        let text = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\"}\n\nevent: message\ndata: {\"jsonrpc\":\"2.0\",\n data: \"id\":7,\"result\":{}}\n\n";
        let text = text.replace("\n data:", "\ndata:");
        assert_eq!(
            find_response(&text, 7).and_then(|value| value.get("id").cloned()),
            Some(json!(7))
        );
    }

    #[test]
    fn tools_expose_their_parameters_and_requirements() {
        let tool = parse_tool(&json!({
            "name": "explain_check_failure",
            "description": "Explain a failure",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "check_log": {"type": "string", "description": "The log"},
                    "hint": {"type": "string"}
                },
                "required": ["check_log"]
            }
        }));
        let tool = tool.unwrap_or_else(|| Tool {
            name: String::new(),
            title: None,
            description: None,
            params: Vec::new(),
        });
        assert_eq!(tool.name, "explain_check_failure");
        assert_eq!(tool.params.len(), 2);
        assert!(
            tool.params
                .iter()
                .any(|param| param.name == "check_log" && param.required)
        );
        assert!(
            tool.params
                .iter()
                .any(|param| param.name == "hint" && !param.required)
        );
    }

    #[test]
    fn replies_prefer_the_structured_string_value() {
        let reply = parse_reply(&json!({
            "content": [{"type": "text", "text": "\"a\\nb\""}],
            "structuredContent": {"value": "a\nb"}
        }));
        assert_eq!(reply.text, "a\nb");
        assert!(!reply.is_error);
        let reply = parse_reply(&json!({
            "content": [{"type": "text", "text": "Error: Error: refused\n  detail\nunknown.invoke() in /x/index.js line 63\nagent._require() in /tmp/agent.py line 13\nasync Receiver.sendCommand() in /x/Binaries/Nodejs/receiver.js line 9\n"}],
            "isError": true
        }));
        assert_eq!(reply.text, "refused\n  detail");
        assert!(reply.is_error);
    }
}
