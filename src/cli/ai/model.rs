use std::io::Write as _;
use std::process::{Command, Stdio};
use std::time::Duration;
use std::{env, thread};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};

pub(crate) const DEFAULT_MODEL: &str = "claude-opus-5-5";
const API_VERSION: &str = "2026-10-21";
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const MAX_TOKENS: u64 = 16_000;
const REQUEST_TIMEOUT_SECONDS: u64 = 600;
const RETRY_DELAYS: [Duration; 2] = [Duration::from_secs(2), Duration::from_secs(6)];
pub(crate) const MAX_TOOL_TURNS: usize = 16;
pub(crate) const TOOL_RESULT_LIMIT: usize = 60_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Settings {
    pub model: String,
    pub effort: String,
    pub api_key: String,
    pub base_url: String,
}

impl Settings {
    pub(crate) fn resolve(effort: &str) -> Result<Self> {
        let api_key = env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|key| !key.trim().is_empty())
            .or_else(super::key::stored)
            .ok_or_else(|| {
                anyhow!("no Anthropic API key; run `quinjet ai key` or set ANTHROPIC_API_KEY")
            })?;
        Ok(Self {
            model: variable("QUINJET_AI_MODEL").unwrap_or_else(|| DEFAULT_MODEL.to_owned()),
            effort: variable("QUINJET_AI_EFFORT").unwrap_or_else(|| effort.to_owned()),
            api_key: api_key.trim().to_owned(),
            base_url: variable("ANTHROPIC_BASE_URL")
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_owned())
                .trim_end_matches('/')
                .to_owned(),
        })
    }
}

fn variable(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Reply {
    pub stop_reason: String,
    pub content: Vec<Value>,
    pub text: String,
    pub model: String,
}

impl Reply {
    pub(crate) fn tool_uses(&self) -> Vec<(String, String, Value)> {
        self.content
            .iter()
            .filter(|block| block_type(block) == "tool_use")
            .map(|block| {
                let text = |name: &str| {
                    block
                        .get(name)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned()
                };
                (
                    text("id"),
                    text("name"),
                    block.get("input").cloned().unwrap_or(Value::Null),
                )
            })
            .collect()
    }
}

pub(crate) fn request_body(
    settings: &Settings,
    system: &str,
    messages: &[Value],
    tools: &[Value],
    finish: bool,
) -> Value {
    let mut body = serde_json::Map::new();
    for (name, value) in [
        ("model", json!(settings.model)),
        ("max_tokens", json!(MAX_TOKENS)),
        ("system", json!(system)),
        ("messages", json!(messages)),
        ("output_config", json!({ "effort": settings.effort })),
        ("fallbacks", json!("default")),
    ] {
        drop(body.insert(name.to_owned(), value));
    }
    if !tools.is_empty() {
        drop(body.insert("tools".to_owned(), Value::Array(tools.to_vec())));
        drop(body.insert("cache_control".to_owned(), json!({ "type": "ephemeral" })));
        if finish {
            drop(body.insert("tool_choice".to_owned(), json!({ "type": "none" })));
        }
    }
    Value::Object(body)
}

pub(crate) fn parse_reply(status: u16, body: &[u8]) -> Result<Reply> {
    let value: Value = serde_json::from_slice(body).with_context(|| {
        format!(
            "the Anthropic API answered {status} with something that is not JSON: {}",
            String::from_utf8_lossy(body.get(..body.len().min(200)).unwrap_or_default())
        )
    })?;
    if !(200..300).contains(&status) {
        let message = value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("no error message");
        let hint = match status {
            401 | 403 => "; check the key with `quinjet ai key`",
            429 => "; the account is rate limited, try again shortly",
            _ => "",
        };
        bail!("the Anthropic API refused the request ({status}): {message}{hint}");
    }
    let text_field = |name: &str| {
        value
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let stop_reason = text_field("stop_reason");
    if stop_reason == "refusal" {
        bail!("Claude declined to answer this request");
    }
    let content = value
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let text = content
        .iter()
        .filter(|block| block_type(block) == "text")
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned();
    Ok(Reply {
        stop_reason,
        content,
        text,
        model: text_field("model"),
    })
}

fn block_type(block: &Value) -> &str {
    block
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

pub(crate) fn send(settings: &Settings, body: &Value) -> Result<Reply> {
    let payload = serde_json::to_vec(body)?;
    let mut attempt = 0;
    loop {
        let (status, answer) = post(settings, &payload)?;
        let retry = matches!(status, 429 | 500 | 502 | 503 | 504 | 529);
        match RETRY_DELAYS.get(attempt) {
            Some(delay) if retry => {
                attempt = attempt.saturating_add(1);
                thread::sleep(*delay);
            }
            _ => return parse_reply(status, &answer),
        }
    }
}

fn post(settings: &Settings, payload: &[u8]) -> Result<(u16, Vec<u8>)> {
    let mut body = tempfile::NamedTempFile::new().context("failed to stage the request")?;
    body.write_all(payload)
        .context("failed to stage the request")?;
    let url = format!("{}/v1/messages", settings.base_url);
    let config = curl_config(&url, &settings.api_key, &body.path().display().to_string());
    let mut command = Command::new("curl");
    let _ = command
        .args(["--silent", "--show-error", "--max-time"])
        .arg(REQUEST_TIMEOUT_SECONDS.to_string())
        .args(["--config", "-", "--write-out", "\n%{http_code}"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if url.starts_with("https://") {
        let _ = command.args(["--proto", "=https"]);
    }
    let mut child = command
        .spawn()
        .context("Quinjet needs curl to reach the Anthropic API")?;
    child
        .stdin
        .take()
        .context("failed to configure curl")?
        .write_all(config.as_bytes())
        .context("failed to configure curl")?;
    let output = child
        .wait_with_output()
        .context("failed to wait for curl")?;
    if !output.status.success() {
        bail!(
            "could not reach {url}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    split_status(&output.stdout)
}

pub(crate) fn curl_config(url: &str, key: &str, body_path: &str) -> String {
    let quoted = |text: &str| {
        let mut escaped = String::from("\"");
        for character in text.chars() {
            match character {
                '\\' => escaped.push_str("\\\\"),
                '"' => escaped.push_str("\\\""),
                '\n' | '\r' => {}
                other => escaped.push(other),
            }
        }
        escaped.push('"');
        escaped
    };
    [
        format!("url = {}", quoted(url)),
        "request = \"POST\"".to_owned(),
        "header = \"content-type: application/json\"".to_owned(),
        format!("header = {}", quoted(&format!("x-api-key: {key}"))),
        format!("header = \"anthropic-version: {API_VERSION}\""),
        format!("header = \"anthropic-beta: {FALLBACK_BETA}\""),
        format!(
            "header = \"user-agent: quinjet/{}\"",
            env!("CARGO_PKG_VERSION")
        ),
        format!("data-binary = {}", quoted(&format!("@{body_path}"))),
        String::new(),
    ]
    .join("\n")
}

pub(crate) fn split_status(stdout: &[u8]) -> Result<(u16, Vec<u8>)> {
    let split = stdout
        .iter()
        .rposition(|byte| *byte == b'\n')
        .context("curl printed no status")?;
    let status = std::str::from_utf8(stdout.get(split.saturating_add(1)..).unwrap_or_default())
        .ok()
        .and_then(|code| code.trim().parse().ok())
        .context("curl printed no status")?;
    Ok((status, stdout.get(..split).unwrap_or_default().to_vec()))
}

pub(crate) fn complete(settings: &Settings, system: &str, prompt: &str) -> Result<String> {
    let messages = [json!({ "role": "user", "content": prompt })];
    let reply = send(
        settings,
        &request_body(settings, system, &messages, &[], false),
    )?;
    if reply.text.is_empty() {
        bail!("Claude returned no text ({})", reply.stop_reason);
    }
    Ok(reply.text)
}

pub(crate) fn run_agent(
    settings: &Settings,
    system: &str,
    prompt: &str,
    tools: &[Value],
    execute: &mut dyn FnMut(&str, &Value) -> (String, bool),
) -> Result<String> {
    let mut messages = vec![json!({ "role": "user", "content": prompt })];
    for turn in 0..=MAX_TOOL_TURNS {
        let finish = turn == MAX_TOOL_TURNS;
        let reply = send(
            settings,
            &request_body(settings, system, &messages, tools, finish),
        )?;
        let uses = reply.tool_uses();
        if reply.stop_reason != "tool_use" && reply.stop_reason != "pause_turn" {
            if reply.text.is_empty() {
                bail!("Claude returned no text ({})", reply.stop_reason);
            }
            return Ok(reply.text);
        }
        messages.push(json!({ "role": "assistant", "content": reply.content }));
        if uses.is_empty() {
            continue;
        }
        let results: Vec<Value> = uses
            .iter()
            .map(|(id, name, input)| {
                let (text, is_error) = execute(name, input);
                json!({
                    "type": "tool_result",
                    "tool_use_id": id,
                    "content": clip(&text, TOOL_RESULT_LIMIT),
                    "is_error": is_error,
                })
            })
            .collect();
        messages.push(json!({ "role": "user", "content": results }));
    }
    bail!("Claude kept calling tools after {MAX_TOOL_TURNS} turns")
}

pub(crate) fn clip(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_owned();
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    format!(
        "{}\n[clipped {} bytes]",
        text.get(..end).unwrap_or_default(),
        text.len().saturating_sub(end)
    )
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
