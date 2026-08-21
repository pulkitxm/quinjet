use super::*;

#[test]
fn capabilities_describe_the_installed_command_tree() -> Result<()> {
    let run = run_in(None, &["capabilities", "--json"])?.success()?;
    let document = run.json()?;
    ensure!(document["schemaVersion"] == 1);
    ensure!(document["version"] == env!("CARGO_PKG_VERSION"));
    let commands = document["commands"]
        .as_array()
        .context("capabilities commands were not an array")?;
    for path in [
        "quinjet status",
        "quinjet branch create",
        "quinjet pr checks",
        "quinjet capabilities",
    ] {
        ensure!(
            commands.iter().any(|command| command["path"] == path),
            "capabilities omitted {path}"
        );
    }
    let completion = commands
        .iter()
        .find(|command| command["path"] == "quinjet completions")
        .context("capabilities omitted completions")?;
    ensure!(
        completion["arguments"][0]["possibleValues"]
            == serde_json::json!(["bash", "elvish", "fish", "powershell", "zsh"])
    );
    let tui = commands
        .iter()
        .find(|command| command["path"] == "quinjet tui")
        .context("capabilities omitted tui")?;
    let theme = tui["arguments"]
        .as_array()
        .and_then(|arguments| arguments.iter().find(|argument| argument["id"] == "theme"))
        .context("tui capabilities omitted --theme")?;
    ensure!(theme["defaultValues"] == serde_json::json!(["quinjet"]));
    ensure!(
        theme["possibleValues"]
            == serde_json::json!([
                "quinjet",
                "catppuccin",
                "dracula",
                "everforest",
                "gruvbox",
                "nord",
                "one",
                "rose-pine",
                "solarized",
                "tokyo-night",
                "ayu",
                "monokai",
                "github"
            ])
    );
    let appearance = tui["arguments"]
        .as_array()
        .and_then(|arguments| {
            arguments
                .iter()
                .find(|argument| argument["id"] == "appearance")
        })
        .context("tui capabilities omitted --appearance")?;
    ensure!(appearance["defaultValues"] == serde_json::json!(["system"]));
    ensure!(appearance["possibleValues"] == serde_json::json!(["system", "light", "dark"]));
    let stage = commands
        .iter()
        .find(|command| command["path"] == "quinjet stage")
        .context("capabilities omitted stage")?;
    ensure!(
        stage["usage"]
            .as_str()
            .is_some_and(|usage| usage.contains("<PATH|--all>"))
    );
    let all = stage["arguments"]
        .as_array()
        .and_then(|arguments| arguments.iter().find(|argument| argument["id"] == "all"))
        .context("stage capabilities omitted --all")?;
    ensure!(all["action"] == "set_true");
    ensure!(all["minValues"] == 0 && all["maxValues"] == 0);
    ensure!(all["possibleValues"] == serde_json::json!([]));
    let selection = stage["groups"]
        .as_array()
        .and_then(|groups| {
            groups
                .iter()
                .find(|group| group["arguments"] == serde_json::json!(["paths", "all"]))
        })
        .context("stage capabilities omitted its required selection group")?;
    ensure!(selection["required"] == true);
    ensure!(selection["multiple"] == false);

    let status = commands
        .iter()
        .find(|command| command["path"] == "quinjet status")
        .context("capabilities omitted status")?;
    let interval = status["arguments"]
        .as_array()
        .and_then(|arguments| {
            arguments
                .iter()
                .find(|argument| argument["id"] == "interval")
        })
        .context("status capabilities omitted --interval")?;
    ensure!(interval["action"] == "set");
    ensure!(interval["defaultValues"] == serde_json::json!(["2"]));
    Ok(())
}

#[cfg(not(windows))]
#[test]
fn bash_accepts_the_generated_completion_script() -> Result<()> {
    let scratch = Scratch::directory()?;
    let run = scratch.quinjet(&["completions", "bash"])?.success()?;
    scratch.write("quinjet.bash", &run.stdout)?;
    let mut command = ProcessCommand::new("bash");
    command.arg("-n").arg(scratch.path.join("quinjet.bash"));
    isolate_git(&mut command);
    let output = command
        .output()
        .context("failed to validate bash completions")?;
    ensure!(
        output.status.success(),
        "bash rejected completions: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn man_prints_one_page_and_writes_one_per_command() -> Result<()> {
    let scratch = Scratch::directory()?;
    let page = scratch.quinjet(&["man"])?.success()?;
    ensure!(
        page.stdout.contains(".TH QUINJET"),
        "man page misses its title header"
    );
    let target = scratch.path.join("man");
    let target_argument = target.display().to_string();
    drop(
        scratch
            .quinjet(&["man", "--dir", &target_argument])?
            .success()?,
    );
    ensure!(
        target.join("quinjet.1").is_file(),
        "the top page was not written"
    );
    ensure!(
        target.join("quinjet-branch-create.1").is_file(),
        "the nested page was not written"
    );
    let nested = fs::read_to_string(target.join("quinjet-branch-create.1"))?;
    ensure!(
        nested.contains("quinjet branch create"),
        "nested synopsis lost its command path: {nested}"
    );
    ensure!(
        nested.contains("\\-\\-json") && nested.contains("\\-C"),
        "nested page lost global options: {nested}"
    );
    Ok(())
}
