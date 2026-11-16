use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Context, Result};
use serde_json::{Value, json};

pub(crate) struct FakeGateway {
    url: String,
    calls: Arc<Mutex<Vec<Value>>>,
}

impl FakeGateway {
    pub(crate) fn start() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").context("failed to bind a test port")?;
        let port = listener.local_addr()?.port();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&calls);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                drop(serve(stream, &recorded));
            }
        });
        Ok(Self {
            url: format!("http://127.0.0.1:{port}"),
            calls,
        })
    }

    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    pub(crate) fn calls(&self) -> Vec<Value> {
        self.calls
            .lock()
            .map(|calls| calls.clone())
            .unwrap_or_default()
    }
}

fn serve(stream: TcpStream, calls: &Mutex<Vec<Value>>) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut length = 0;
    let mut session = false;
    let mut line = String::new();
    loop {
        line.clear();
        reader.read_line(&mut line)?;
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        let lower = header.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("content-length:") {
            length = value.trim().parse().unwrap_or_default();
        }
        if lower.starts_with("mcp-session-id: fake-session") {
            session = true;
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let message: Value = serde_json::from_slice(&body)?;
    let method = field(&message, "method")
        .as_str()
        .unwrap_or_default()
        .to_owned();
    if method != "initialize" && !session {
        return respond(stream, 400, "application/json", r#"{"error":"no session"}"#);
    }
    let id = field(&message, "id").clone();
    match method.as_str() {
        "initialize" => respond_json(
            stream,
            &json!({"jsonrpc": "2.0", "id": id, "result": {
                "protocolVersion": "2025-06-18",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "fake-gateway", "version": "1"}
            }}),
        ),
        "notifications/initialized" => respond(stream, 202, "application/json", ""),
        "tools/list" => respond_json(
            stream,
            &json!({"jsonrpc": "2.0", "id": id, "result": {"tools": tools()}}),
        ),
        "tools/call" => {
            if let Ok(mut recorded) = calls.lock() {
                recorded.push(field(&message, "params").clone());
            }
            call(stream, &id, field(&message, "params"))
        }
        _ => respond_json(
            stream,
            &json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "unknown"}}),
        ),
    }
}

fn tools() -> Value {
    json!([
        {
            "name": "draft_commit_message",
            "description": "Draft a commit message for the staged change",
            "inputSchema": {"type": "object", "properties": {
                "staged_diff": {"type": "string", "description": "The staged patch"},
                "branch": {"type": "string"},
                "recent_commits": {"type": "string"},
                "tone": {"type": "string"}
            }, "required": ["staged_diff", "branch", "recent_commits", "tone"]}
        },
        {
            "name": "explain_check_failure",
            "description": "Explain a failing check\nSecond line",
            "inputSchema": {"type": "object", "properties": {
                "check_log": {"type": "string"},
                "pull_request": {"type": "integer"}
            }, "required": ["check_log", "pull_request"]}
        },
        {"name": "refuse", "inputSchema": {"type": "object", "properties": {}}},
        {"name": "stream", "inputSchema": {"type": "object", "properties": {}}}
    ])
}

fn call(stream: TcpStream, id: &Value, params: &Value) -> Result<()> {
    let name = field(params, "name").as_str().unwrap_or_default();
    let arguments = &field(params, "arguments");
    match name {
        "refuse" => respond_json(
            stream,
            &json!({"jsonrpc": "2.0", "id": id, "result": {
                "content": [{"type": "text", "text": "Error: Error: the model is offline\nunknown.refuse() in /srv/agent.py line 9"}],
                "isError": true
            }}),
        ),
        "stream" => {
            let event = json!({"jsonrpc": "2.0", "id": id, "result": {
                "content": [{"type": "text", "text": "streamed answer"}]
            }});
            let body = format!(
                "event: message\ndata: {}\n\nevent: message\ndata: {event}\n\n",
                json!({"jsonrpc": "2.0", "method": "notifications/progress", "params": {}})
            );
            respond(stream, 200, "text/event-stream", &body)
        }
        _ => {
            let summary = format!(
                "{name}: {}",
                serde_json::to_string(arguments).unwrap_or_default()
            );
            respond_json(
                stream,
                &json!({"jsonrpc": "2.0", "id": id, "result": {
                    "content": [{"type": "text", "text": serde_json::to_string(&summary).unwrap_or_default()}],
                    "structuredContent": {"value": summary}
                }}),
            )
        }
    }
}

fn respond_json(stream: TcpStream, value: &Value) -> Result<()> {
    respond(stream, 200, "application/json", &value.to_string())
}

fn respond(mut stream: TcpStream, status: u16, content_type: &str, body: &str) -> Result<()> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        _ => "Bad Request",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nMcp-Session-Id: fake-session\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    stream.flush()?;
    Ok(())
}

fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    value.get(name).unwrap_or(&Value::Null)
}
