use std::io::{BufRead, BufReader, Write};
use std::process::Stdio;

use serde_json::{Value, json};

use super::fake_claude::{FakeClaude, KEY};
use super::*;

fn with_claude(scratch: &Scratch, claude: &FakeClaude, args: &[&str]) -> Result<Run> {
    let mut command = scratch.quinjet_command(args);
    command
        .env("ANTHROPIC_BASE_URL", claude.url())
        .env("ANTHROPIC_API_KEY", KEY);
    Run::from(command.output().context("failed to run quinjet")?)
}

fn staged_repository() -> Result<Scratch> {
    let scratch = Scratch::repository()?;
    scratch.write("widgets.rs", "pub fn fetch() {}\n")?;
    scratch.git(&["add", "widgets.rs"])?;
    Ok(scratch)
}

#[test]
fn ai_commands_send_the_repository_context_to_claude() -> Result<()> {
    let scratch = staged_repository()?;
    let claude = FakeClaude::start()?;
    let drafted = with_claude(&scratch, &claude, &["ai", "commit-message"])?.success()?;
    ensure!(
        drafted.stdout == "feat(widgets): add the widget fetcher\n",
        "{}",
        drafted.stdout
    );
    let reviewed = with_claude(&scratch, &claude, &["ai", "review", "--json"])?
        .success()?
        .json()?;
    ensure!(reviewed["task"] == "review" && reviewed["text"] == "No findings.");
    ensure!(reviewed["model"] == "claude-opus-5-5");
    let requests = claude.requests();
    ensure!(requests.len() == 2, "{requests:?}");
    let first = &requests[0];
    ensure!(first.key == KEY);
    ensure!(first.version == "2026-10-21", "{}", first.version);
    ensure!(first.beta == "server-side-fallback-2026-07-01");
    ensure!(first.body["model"] == "claude-opus-5-5");
    ensure!(first.body["fallbacks"] == "default");
    ensure!(first.body["output_config"]["effort"] == "low");
    let prompt = first.body["messages"][0]["content"]
        .as_str()
        .unwrap_or_default();
    ensure!(prompt.contains("Branch: main"), "{prompt}");
    ensure!(prompt.contains("base\n"), "{prompt}");
    ensure!(prompt.contains("+pub fn fetch() {}"), "{prompt}");
    ensure!(requests[1].body["output_config"]["effort"] == "high");
    Ok(())
}

#[test]
fn investigations_loop_through_quinjet_read_tools() -> Result<()> {
    let scratch = Scratch::repository()?;
    let claude = FakeClaude::start()?;
    let run = with_claude(&scratch, &claude, &["ai", "investigate", "7"])?.success()?;
    ensure!(
        run.stdout == "Verdict: ready\nEvidence: the newest commit is base\n",
        "{}",
        run.stdout
    );
    let requests = claude.requests();
    ensure!(requests.len() == 2, "{requests:?}");
    let tools = requests[0].body["tools"]
        .as_array()
        .context("the loop sent no tools")?;
    ensure!(tools.len() == 23, "{}", tools.len());
    ensure!(
        !tools
            .iter()
            .any(|tool| tool["name"].as_str().unwrap_or_default().starts_with("ai"))
    );
    ensure!(requests[0].body["cache_control"]["type"] == "ephemeral");
    let result = &requests[1].body["messages"][2]["content"][0];
    ensure!(result["type"] == "tool_result" && result["tool_use_id"] == "toolu_1");
    ensure!(result["is_error"] == false);
    Ok(())
}

#[test]
fn a_saved_key_replaces_the_environment_and_can_be_forgotten() -> Result<()> {
    let scratch = staged_repository()?;
    let claude = FakeClaude::start()?;
    let mut missing = scratch.quinjet_command(&["ai", "commit-message"]);
    missing.env("ANTHROPIC_BASE_URL", claude.url());
    let refused = Run::from(missing.output()?)?;
    ensure!(refused.code == 1);
    ensure!(
        refused.stderr.contains("no Anthropic API key")
            && refused.stderr.contains("hint: paste a key"),
        "{}",
        refused.stderr
    );
    let mut save = scratch.quinjet_command(&["ai", "key"]);
    save.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = save.spawn()?;
    child
        .stdin
        .take()
        .context("no stdin")?
        .write_all(format!("{KEY}\n").as_bytes())?;
    let saved = Run::from(child.wait_with_output()?)?.success()?;
    ensure!(saved.stdout.starts_with("Saved the Anthropic API key to "));
    let mut stored = scratch.quinjet_command(&["ai", "commit-message"]);
    stored.env("ANTHROPIC_BASE_URL", claude.url());
    Run::from(stored.output()?)?.success()?;
    ensure!(
        claude
            .requests()
            .last()
            .is_some_and(|request| request.key == KEY)
    );
    let mut wrong = scratch.quinjet_command(&["ai", "commit-message"]);
    wrong
        .env("ANTHROPIC_BASE_URL", claude.url())
        .env("ANTHROPIC_API_KEY", "sk-wrong");
    let rejected = Run::from(wrong.output()?)?;
    ensure!(
        rejected.stderr.contains("(401): invalid x-api-key"),
        "{}",
        rejected.stderr
    );
    let cleared = scratch
        .quinjet(&["ai", "key", "--clear", "--json"])?
        .success()?;
    ensure!(cleared.json()?["cleared"] == true);
    Ok(())
}

#[test]
fn nothing_staged_is_reported_before_any_request() -> Result<()> {
    let scratch = Scratch::repository()?;
    let claude = FakeClaude::start()?;
    let run = with_claude(&scratch, &claude, &["ai", "commit-message"])?;
    ensure!(run.code == 1);
    ensure!(run.stderr.contains("nothing is staged"), "{}", run.stderr);
    ensure!(claude.requests().is_empty());
    Ok(())
}

fn exchange(lines: &[Value], scratch: &Scratch, args: &[&str]) -> Result<Vec<Value>> {
    let mut command = scratch.quinjet_command(args);
    command
        .env("QUINJET_GRAFTCODE_GATEWAY", scratch.path.join("missing-gg"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let mut input = child.stdin.take().context("no stdin")?;
    let mut output = BufReader::new(child.stdout.take().context("no stdout")?);
    let mut answers = Vec::new();
    for line in lines {
        writeln!(input, "{line}")?;
        input.flush()?;
        if line.get("id").is_some() {
            let mut answer = String::new();
            output.read_line(&mut answer)?;
            answers.push(serde_json::from_str(&answer)?);
        }
    }
    drop(input);
    let status = child.wait()?;
    ensure!(status.success(), "quinjet mcp exited with {status}");
    Ok(answers)
}

#[test]
fn mcp_answers_agents_on_standard_io() -> Result<()> {
    let scratch = Scratch::repository()?;
    let answers = exchange(
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "test", "version": "1"}}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "ping"}),
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "status", "arguments": {}}}),
            json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "nope", "arguments": {}}}),
            json!({"jsonrpc": "2.0", "id": 6, "method": "sampling/createMessage"}),
        ],
        &scratch,
        &["mcp"],
    )?;
    let initialized = &answers[0]["result"];
    ensure!(initialized["protocolVersion"] == "2025-03-26");
    ensure!(initialized["serverInfo"]["name"] == "quinjet");
    let tools = answers[1]["result"]["tools"]
        .as_array()
        .context("no tools")?;
    ensure!(tools.len() == 94, "{}", tools.len());
    let merge = tools
        .iter()
        .find(|tool| tool["name"] == "prMerge")
        .context("prMerge missing")?;
    ensure!(merge["annotations"]["readOnlyHint"] == false);
    ensure!(merge["annotations"]["destructiveHint"] == true);
    ensure!(merge["inputSchema"]["properties"]["yes"]["type"] == "boolean");
    let checks = tools
        .iter()
        .find(|tool| tool["name"] == "prChecks")
        .context("prChecks missing")?;
    ensure!(checks["inputSchema"]["required"] == json!(["number"]));
    ensure!(checks["inputSchema"]["properties"]["repository"].is_object());
    ensure!(
        checks["inputSchema"]["properties"]
            .get("accessToken")
            .is_none()
    );
    ensure!(answers[2]["result"] == json!({}));
    let status = &answers[3]["result"];
    ensure!(status["isError"] == true);
    ensure!(
        status["content"][0]["text"]
            .as_str()
            .is_some_and(|text| text.contains("no Graftcode gateway at")
                && text.contains("\nhint: run `quinjet graft install`")),
        "{status}"
    );
    ensure!(answers[4]["result"]["content"][0]["text"] == "Quinjet has no tool called `nope`");
    ensure!(answers[5]["error"]["code"] == -32601);
    Ok(())
}

#[test]
fn read_only_mcp_offers_and_runs_only_reads() -> Result<()> {
    let scratch = Scratch::repository()?;
    let answers = exchange(
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "commit", "arguments": {"message": "x"}}}),
        ],
        &scratch,
        &["mcp", "--read-only"],
    )?;
    let tools = answers[0]["result"]["tools"]
        .as_array()
        .context("no tools")?;
    ensure!(tools.len() == 27, "{}", tools.len());
    ensure!(
        tools
            .iter()
            .all(|tool| tool["annotations"]["readOnlyHint"] == true
                || tool["name"].as_str().unwrap_or_default().starts_with("ai"))
    );
    ensure!(answers[1]["result"]["content"][0]["text"] == "Quinjet has no tool called `commit`");
    Ok(())
}

#[test]
fn installed_agents_get_quinjet_added_once() -> Result<()> {
    let scratch = Scratch::directory()?;
    fs::create_dir_all(scratch.environment.join(".cursor"))?;
    let first = scratch
        .quinjet(&["mcp", "install", "--agent", "cursor", "--json"])?
        .success()?
        .json()?;
    ensure!(
        first[0]["agent"] == "cursor" && first[0]["outcome"] == "added",
        "{first}"
    );
    let config: Value = serde_json::from_str(&fs::read_to_string(
        scratch.environment.join(".cursor").join("mcp.json"),
    )?)?;
    ensure!(config["mcpServers"]["quinjet"]["args"] == json!(["mcp"]));
    ensure!(
        config["mcpServers"]["quinjet"]["command"]
            .as_str()
            .is_some_and(|command| command == "quinjet"
                || command.ends_with(if cfg!(windows) {
                    "quinjet.exe"
                } else {
                    "quinjet"
                })),
        "{config}"
    );
    let again = scratch
        .quinjet(&["mcp", "install", "--agent", "cursor", "--json"])?
        .success()?
        .json()?;
    ensure!(again[0]["outcome"] == "unchanged", "{again}");
    let printed = scratch
        .quinjet(&["mcp", "install", "--print", "--json"])?
        .success()?
        .json()?;
    ensure!(printed["mcpServers"]["quinjet"]["args"] == json!(["mcp"]));
    let read_only = scratch
        .quinjet(&["mcp", "install", "--print", "--read-only", "--json"])?
        .success()?
        .json()?;
    ensure!(read_only["args"] == json!(["mcp", "--read-only"]));
    Ok(())
}

#[test]
fn a_stopped_shared_gateway_says_how_it_starts() -> Result<()> {
    let scratch = Scratch::directory()?;
    let status = scratch.quinjet(&["graft", "status"])?.success()?;
    ensure!(
        status
            .stdout
            .starts_with("The shared Graftcode gateway is not running"),
        "{}",
        status.stdout
    );
    ensure!(
        scratch
            .quinjet(&["graft", "status", "--json"])?
            .success()?
            .json()?["running"]
            == false
    );
    ensure!(
        scratch
            .quinjet(&["graft", "stop", "--json"])?
            .success()?
            .json()?["stopped"]
            == false
    );
    Ok(())
}
