use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Context, Result};
use serde_json::{Value, json};

pub(crate) const KEY: &str = "sk-ant-test";

#[derive(Debug, Clone)]
pub(crate) struct Request {
    pub key: String,
    pub version: String,
    pub beta: String,
    pub body: Value,
}

pub(crate) struct FakeClaude {
    url: String,
    requests: Arc<Mutex<Vec<Request>>>,
}

impl FakeClaude {
    pub(crate) fn start() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").context("failed to bind a test port")?;
        let port = listener.local_addr()?.port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                drop(serve(stream, &recorded));
            }
        });
        Ok(Self {
            url: format!("http://127.0.0.1:{port}"),
            requests,
        })
    }

    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    pub(crate) fn requests(&self) -> Vec<Request> {
        self.requests
            .lock()
            .map(|requests| requests.clone())
            .unwrap_or_default()
    }
}

fn text(value: &Value, name: &str) -> String {
    value
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn serve(stream: TcpStream, requests: &Mutex<Vec<Request>>) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let (mut length, mut key, mut version, mut beta) =
        (0, String::new(), String::new(), String::new());
    let mut line = String::new();
    loop {
        line.clear();
        reader.read_line(&mut line)?;
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        let Some((name, value)) = header.split_once(':') else {
            continue;
        };
        let value = value.trim().to_owned();
        match name.to_ascii_lowercase().as_str() {
            "content-length" => length = value.parse().unwrap_or_default(),
            "x-api-key" => key = value,
            "anthropic-version" => version = value,
            "anthropic-beta" => beta = value,
            _ => {}
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let body: Value = serde_json::from_slice(&body)?;
    if let Ok(mut recorded) = requests.lock() {
        recorded.push(Request {
            key: key.clone(),
            version,
            beta,
            body: body.clone(),
        });
    }
    if key != KEY {
        return respond(
            stream,
            401,
            &json!({"type": "error", "error": {"type": "authentication_error", "message": "invalid x-api-key"}}),
        );
    }
    let model = text(&body, "model");
    let messages = body
        .get("messages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let has_tools = body.get("tools").is_some();
    if has_tools && messages.len() == 1 {
        return respond(
            stream,
            200,
            &json!({"model": model, "stop_reason": "tool_use", "content": [
                {"type": "text", "text": "Reading the history."},
                {"type": "tool_use", "id": "toolu_1", "name": "log", "input": {"revision": "", "skip": "", "limit": "1"}}
            ]}),
        );
    }
    let answer = if has_tools {
        let result = messages
            .last()
            .and_then(|message| message.get("content"))
            .and_then(Value::as_array)
            .and_then(|blocks| blocks.first())
            .map(|block| text(block, "content"))
            .unwrap_or_default();
        let subject = serde_json::from_str::<Value>(&result)
            .ok()
            .and_then(|commits| commits.get(0).map(|commit| text(commit, "subject")))
            .unwrap_or_default();
        format!("Verdict: ready\nEvidence: the newest commit is {subject}")
    } else {
        let prompt = messages
            .first()
            .map(|message| text(message, "content"))
            .unwrap_or_default();
        if prompt.contains("Staged patch") {
            "feat(widgets): add the widget fetcher".to_owned()
        } else {
            "No findings.".to_owned()
        }
    };
    respond(
        stream,
        200,
        &json!({"model": model, "stop_reason": "end_turn", "content": [{"type": "text", "text": answer}]}),
    )
}

fn respond(mut stream: TcpStream, status: u16, body: &Value) -> Result<()> {
    let body = body.to_string();
    let reason = if status == 200 { "OK" } else { "Unauthorized" };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    stream.flush()?;
    Ok(())
}
