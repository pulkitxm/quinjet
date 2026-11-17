use super::fake_gateway::FakeGateway;
use super::*;

fn node_available() -> bool {
    ProcessCommand::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn registered(scratch: &Scratch, gateway: &FakeGateway) -> Result<()> {
    scratch
        .quinjet(&["graft", "add", "agent", gateway.url()])?
        .success()?;
    Ok(())
}

#[test]
fn the_receiver_exposes_reads_by_default_and_writes_on_request() -> Result<()> {
    let scratch = Scratch::directory()?;
    let read_dir = scratch.path.join("read");
    let read_path = read_dir.to_string_lossy().into_owned();
    let read = scratch
        .quinjet(&["graft", "module", "--out", &read_path, "--json"])?
        .success()?
        .json()?;
    let tools = read["tools"].as_array().context("tools were not listed")?;
    ensure!(
        tools.len() == 27,
        "unexpected read tool count {}",
        tools.len()
    );
    ensure!(tools.iter().all(|tool| tool["access"] == "read"));
    ensure!(
        tools
            .iter()
            .filter(|tool| tool["name"]
                .as_str()
                .is_some_and(|name| name.starts_with("ai")))
            .count()
            == 4
    );
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    for expected in ["status", "diff", "log", "prChecks", "prLogs", "stackView"] {
        ensure!(names.contains(&expected), "missing {expected} in {names:?}");
    }
    let source = fs::read_to_string(read_dir.join("index.js"))?;
    ensure!(source.contains("class Quinjet {"));
    ensure!(
        source.contains(
            "static async prChecks(number, commit, repo, refresh, repoPath, accessToken) {"
        )
    );
    ensure!(source.contains("@param {string} number Pull-request number. Required"));
    ensure!(!source.contains("static async commit("));
    ensure!(!source.contains("aiKey"));
    let package: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(read_dir.join("package.json"))?)?;
    ensure!(package["main"] == "index.js");

    let write_dir = scratch.path.join("write");
    let write_path = write_dir.to_string_lossy().into_owned();
    let write = scratch
        .quinjet(&[
            "graft",
            "module",
            "--out",
            &write_path,
            "--allow-writes",
            "--json",
        ])?
        .success()?
        .json()?;
    let tools = write["tools"].as_array().context("tools were not listed")?;
    ensure!(
        tools.len() > 80,
        "unexpected write tool count {}",
        tools.len()
    );
    ensure!(
        tools
            .iter()
            .any(|tool| tool["name"] == "commit" && tool["access"] == "write")
    );
    ensure!(!tools.iter().any(|tool| tool["name"] == "prOpen"));
    ensure!(!tools.iter().any(|tool| tool["name"] == "graftServe"));
    ensure!(!tools.iter().any(|tool| tool["name"] == "mcpInstall"));
    Ok(())
}

#[test]
fn generated_methods_run_quinjet_against_the_bound_repository() -> Result<()> {
    if !node_available() {
        return Ok(());
    }
    let scratch = Scratch::repository()?;
    scratch.write("README.md", "two\n")?;
    scratch.git(&["commit", "--all", "--message=second change"])?;
    let module = scratch.path.join("module");
    let module_path = module.to_string_lossy().into_owned();
    scratch
        .quinjet(&["graft", "module", "--out", &module_path, "--allow-writes"])?
        .success()?;
    let script = "const { Quinjet } = require(process.argv[1]); \
        (async () => { \
        const commits = JSON.parse(await Quinjet.log(\"\", \"\", \"1\")); \
        const status = JSON.parse(await Quinjet.status()); \
        let refused = ''; \
        try { await Quinjet.show('no-such-revision', false); } catch (error) { refused = error.message; } \
        await Quinjet.branchCreate('topic', ''); \
        const renamed = JSON.parse(await Quinjet.branchRename('topic', 'renamed')); \
        let locked = ''; \
        process.env.QUINJET_GRAFT_TOKEN = 'secret'; \
        try { await Quinjet.status('', 'wrong'); } catch (error) { locked = error.message; } \
        const opened = JSON.parse(await Quinjet.status('', 'secret')); \
        process.stdout.write(JSON.stringify({ subject: commits[0].subject, count: commits.length, head: status.branch.head, refused, renamed: renamed.message, locked, opened: opened.branch.head })); \
        })();";
    let mut command = ProcessCommand::new("node");
    command
        .arg("-e")
        .arg(script)
        .arg(&module_path)
        .env("QUINJET_GRAFT_BIN", env!("CARGO_BIN_EXE_quinjet"))
        .env("QUINJET_GRAFT_REPO", &scratch.path);
    isolate_git(&mut command);
    isolate_quinjet(&mut command, &scratch.environment);
    let run = Run::from(command.output().context("failed to run node")?)?.success()?;
    let answer = run.json()?;
    ensure!(answer["subject"] == "second change", "{answer}");
    ensure!(answer["count"] == 1, "{answer}");
    ensure!(answer["head"] == "main", "{answer}");
    let refused = answer["refused"].as_str().unwrap_or_default();
    ensure!(refused.contains("quinjet show exited 3"), "{refused}");
    let locked = answer["locked"].as_str().unwrap_or_default();
    ensure!(locked.contains("connect through `quinjet mcp`"), "{locked}");
    ensure!(answer["renamed"].is_string(), "{answer}");
    ensure!(answer["opened"] == "renamed", "{answer}");
    Ok(())
}

#[test]
fn gateways_register_list_and_forget() -> Result<()> {
    let scratch = Scratch::directory()?;
    let empty = scratch.quinjet(&["graft", "list"])?.success()?;
    ensure!(empty.stdout.contains("No gateways are registered"));
    let added = scratch
        .quinjet(&[
            "graft",
            "add",
            "agent",
            "http://localhost:7390/GV",
            "--token-env",
            "AGENT_TOKEN",
            "--json",
        ])?
        .success()?
        .json()?;
    ensure!(added["url"] == "http://localhost:7390/mcp", "{added}");
    let listed = scratch
        .quinjet(&["graft", "list", "--json"])?
        .success()?
        .json()?;
    ensure!(listed[0]["name"] == "agent" && listed[0]["tokenEnv"] == "AGENT_TOKEN");
    let invalid = scratch.quinjet(&["graft", "add", "agent", "ws://localhost:7390"])?;
    ensure!(invalid.code == 1, "{}", invalid.stderr);
    let renamed = scratch.quinjet(&["graft", "add", "bad name", "http://localhost:1"])?;
    ensure!(renamed.code == 1 && renamed.stderr.contains("gateway names use"));
    scratch.quinjet(&["graft", "remove", "agent"])?.success()?;
    let missing = scratch.quinjet(&["graft", "rm", "agent"])?;
    ensure!(missing.code == 3, "{}", missing.stderr);
    Ok(())
}

#[test]
fn tools_mark_the_parameters_quinjet_fills() -> Result<()> {
    let scratch = Scratch::directory()?;
    let gateway = FakeGateway::start()?;
    registered(&scratch, &gateway)?;
    let run = scratch.quinjet(&["graft", "tools"])?.success()?;
    ensure!(
        run.stdout.contains(
            "  draft_commit_message(branch*, recent_commits*, staged_diff*, tone)  Draft a commit message for the staged change"
        ),
        "{}",
        run.stdout
    );
    ensure!(run.stdout.contains("explain_check_failure("));
    ensure!(run.stdout.contains("Explain a failing check\n"));
    let json = scratch
        .quinjet(&["graft", "tools", "agent", "--json"])?
        .success()?
        .json()?;
    ensure!(json[0]["tools"].as_array().map(Vec::len) == Some(4));
    Ok(())
}

#[test]
fn calls_fill_repository_context_and_keep_explicit_arguments() -> Result<()> {
    let scratch = Scratch::repository()?;
    let gateway = FakeGateway::start()?;
    registered(&scratch, &gateway)?;
    scratch.write("README.md", "staged line\n")?;
    scratch.git(&["add", "README.md"])?;
    let run = scratch
        .quinjet(&[
            "graft",
            "call",
            "agent",
            "draftCommitMessage",
            "--arg",
            "tone=terse",
            "--json",
        ])?
        .success()?
        .json()?;
    ensure!(run["tool"] == "draft_commit_message", "{run}");
    ensure!(run["isError"] == false);
    let filled: Vec<&str> = run["filled"]
        .as_array()
        .context("filled was not listed")?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    ensure!(
        filled.len() == 3 && filled.contains(&"staged_diff"),
        "{filled:?}"
    );
    let calls = gateway.calls();
    let arguments = &calls.first().context("the gateway saw no call")?["arguments"];
    let diff = arguments["staged_diff"].as_str().unwrap_or_default();
    ensure!(diff.contains("+staged line"), "{diff}");
    ensure!(arguments["branch"] == "main");
    ensure!(arguments["recent_commits"] == "base\n");
    ensure!(arguments["tone"] == "terse");
    Ok(())
}

#[test]
fn calls_without_context_still_send_every_parameter() -> Result<()> {
    let scratch = Scratch::repository()?;
    let gateway = FakeGateway::start()?;
    registered(&scratch, &gateway)?;
    let run = scratch
        .quinjet(&[
            "graft",
            "call",
            "agent",
            "explain_check_failure",
            "--no-context",
            "-a",
            "pull_request=42",
        ])?
        .success()?;
    ensure!(
        run.stdout
            .starts_with("explain_check_failure: {\"check_log\":\"\",\"pull_request\":42}"),
        "{}",
        run.stdout
    );
    Ok(())
}

#[test]
fn tool_errors_streams_and_outages_have_distinct_outcomes() -> Result<()> {
    let scratch = Scratch::repository()?;
    let gateway = FakeGateway::start()?;
    registered(&scratch, &gateway)?;
    let refused = scratch.quinjet(&["graft", "call", "agent", "refuse"])?;
    ensure!(refused.code == 1, "{}", refused.stderr);
    ensure!(
        refused
            .stderr
            .contains("error: agent.refuse failed: the model is offline"),
        "{}",
        refused.stderr
    );
    ensure!(!refused.stderr.contains("unknown.refuse"));
    let json = scratch.quinjet(&["graft", "call", "agent", "refuse", "--json"])?;
    ensure!(json.code == 1 && json.json()?["isError"] == true);
    let streamed = scratch
        .quinjet(&["graft", "call", "agent", "stream"])?
        .success()?;
    ensure!(
        streamed.stdout == "streamed answer\n",
        "{}",
        streamed.stdout
    );
    let missing = scratch.quinjet(&["graft", "call", "agent", "nothing"])?;
    ensure!(missing.code == 3 && missing.stderr.contains("its tools are:"));
    scratch
        .quinjet(&["graft", "add", "down", "http://127.0.0.1:9"])?
        .success()?;
    let down = scratch.quinjet(&["graft", "call", "down", "anything"])?;
    ensure!(down.code == 4, "{}", down.stderr);
    ensure!(
        down.stderr
            .contains("check that a gateway is running at http://127.0.0.1:9/mcp")
    );
    let tools = scratch.quinjet(&["graft", "tools"])?;
    ensure!(
        tools.code == 4 && tools.stdout.contains("unavailable:"),
        "{}",
        tools.stdout
    );
    Ok(())
}

#[test]
fn serving_without_a_gateway_explains_how_to_install_one() -> Result<()> {
    let scratch = Scratch::repository()?;
    let mut command = scratch.quinjet_command(&["graft", "serve"]);
    command.env("QUINJET_GRAFTCODE_GATEWAY", scratch.path.join("missing-gg"));
    let run = Run::from(command.output().context("failed to run quinjet")?)?;
    ensure!(run.code == 1, "{}", run.stderr);
    ensure!(
        run.stderr.contains("no Graftcode gateway at"),
        "{}",
        run.stderr
    );
    ensure!(
        run.stderr
            .contains("hint: run `quinjet graft install` to download it"),
        "{}",
        run.stderr
    );
    Ok(())
}
