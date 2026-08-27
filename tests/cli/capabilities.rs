use super::*;

fn capability_document() -> Result<serde_json::Value> {
    run_in(None, &["capabilities", "--json"])?.success()?.json()
}

#[test]
fn capabilities_describe_the_installed_command_tree() -> Result<()> {
    let document = capability_document()?;
    ensure!(document["schemaVersion"] == 1);
    ensure!(document["version"] == env!("CARGO_PKG_VERSION"));
    let commands = document["commands"]
        .as_array()
        .context("capabilities commands were not an array")?;
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

#[test]
fn capability_paths_are_complete_unique_and_parent_first() -> Result<()> {
    let document = capability_document()?;
    let commands = document["commands"]
        .as_array()
        .context("capabilities commands were not an array")?;
    ensure!(
        commands.len() == 106,
        "unexpected command count: {}",
        commands.len()
    );
    let root = commands.first().context("capabilities omitted the root")?;
    ensure!(root["path"] == "quinjet");
    ensure!(
        root["subcommands"]
            == serde_json::json!([
                "tui",
                "status",
                "diff",
                "stage",
                "unstage",
                "discard",
                "remove",
                "commit",
                "fetch",
                "pull",
                "push",
                "sync",
                "log",
                "show",
                "branch",
                "stash",
                "worktree",
                "project",
                "cherry-pick",
                "revert",
                "resolve",
                "repos",
                "remote",
                "pr",
                "stack",
                "completions",
                "man",
                "capabilities",
                "update"
            ])
    );
    let paths: Vec<&str> = commands
        .iter()
        .map(|command| {
            command["path"]
                .as_str()
                .context("a capability command omitted its path")
        })
        .collect::<Result<_>>()?;
    let mut unique = std::collections::HashSet::new();
    for (index, path) in paths.iter().enumerate() {
        ensure!(unique.insert(*path), "duplicate capability path: {path}");
        let Some((parent, name)) = path.rsplit_once(' ') else {
            continue;
        };
        let parent_index = paths
            .iter()
            .position(|candidate| *candidate == parent)
            .with_context(|| format!("{path} has no parent capability"))?;
        ensure!(parent_index < index, "{parent} did not precede {path}");
        ensure!(
            commands[parent_index]["subcommands"]
                .as_array()
                .is_some_and(|children| children.iter().any(|child| child == name))
        );
    }
    ensure!(!unique.contains("quinjet rm"));
    ensure!(!unique.contains("quinjet completion"));
    Ok(())
}

#[test]
fn every_capability_carries_the_global_arguments() -> Result<()> {
    let document = capability_document()?;
    let commands = document["commands"]
        .as_array()
        .context("capabilities commands were not an array")?;
    for command in commands {
        let path = command["path"]
            .as_str()
            .context("command path was absent")?;
        let arguments = command["arguments"]
            .as_array()
            .with_context(|| format!("{path} arguments were not an array"))?;
        let repository = arguments
            .iter()
            .find(|argument| argument["id"] == "repository")
            .with_context(|| format!("{path} omitted -C/--path"))?;
        ensure!(repository["short"] == "C" && repository["long"] == "path");
        ensure!(repository["action"] == "set");
        ensure!(repository["defaultValues"] == serde_json::json!(["."]));
        let json = arguments
            .iter()
            .find(|argument| argument["id"] == "json")
            .with_context(|| format!("{path} omitted --json"))?;
        ensure!(json["short"].is_null() && json["long"] == "json");
        ensure!(json["action"] == "set_true");
        ensure!(json["defaultValues"] == serde_json::json!(["false"]));
    }
    Ok(())
}

#[test]
fn plain_capabilities_preserve_the_json_path_order() -> Result<()> {
    let document = capability_document()?;
    let commands = document["commands"]
        .as_array()
        .context("capabilities commands were not an array")?;
    let plain = run_in(None, &["capabilities"])?.success()?;
    let listed: Vec<&str> = plain
        .stdout
        .lines()
        .filter(|line| line.starts_with("quinjet"))
        .collect();
    ensure!(listed.len() == commands.len());
    for (line, command) in listed.iter().zip(commands) {
        let path = command["path"]
            .as_str()
            .context("command path was absent")?;
        ensure!(
            line.starts_with(&format!("{path}  ")),
            "unexpected line: {line}"
        );
    }
    ensure!(plain.stderr.is_empty());
    Ok(())
}

#[test]
fn completion_json_contains_the_exact_plain_script() -> Result<()> {
    let plain = run_in(None, &["completions", "bash"])?.success()?;
    let structured = run_in(None, &["completions", "bash", "--json"])?
        .success()?
        .json()?;
    let object = structured
        .as_object()
        .context("completion JSON was not an object")?;
    ensure!(object.len() == 2);
    ensure!(structured["shell"] == "bash");
    ensure!(structured["script"] == plain.stdout);
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
    let capabilities = capability_document()?;
    let commands = capabilities["commands"]
        .as_array()
        .context("capabilities commands were not an array")?;
    let expected: std::collections::BTreeSet<String> = commands
        .iter()
        .map(|command| {
            command["path"]
                .as_str()
                .context("command path was absent")
                .map(|path| format!("{}.1", path.replace(' ', "-")))
        })
        .collect::<Result<_>>()?;
    let scratch = Scratch::directory()?;
    let page = scratch.quinjet(&["man"])?.success()?;
    ensure!(
        page.stdout.contains(".TH QUINJET"),
        "man page misses its title header"
    );
    let target = scratch.path.join("man");
    let target_argument = target.display().to_string();
    let written = scratch
        .quinjet(&["man", "--dir", &target_argument, "--json"])?
        .success()?
        .json()?;
    let object = written.as_object().context("man JSON was not an object")?;
    ensure!(object.len() == 1);
    let reported: std::collections::BTreeSet<String> = written["pages"]
        .as_array()
        .context("man JSON pages were not an array")?
        .iter()
        .map(|page| {
            Path::new(page.as_str().context("man page path was not text")?)
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_owned)
                .context("man page path had no UTF-8 file name")
        })
        .collect::<Result<_>>()?;
    let actual: std::collections::BTreeSet<String> = fs::read_dir(&target)?
        .map(|entry| {
            entry?
                .file_name()
                .into_string()
                .map_err(|_| anyhow::anyhow!("man page file name was not UTF-8"))
        })
        .collect::<Result<_>>()?;
    ensure!(
        reported == expected,
        "reported pages differ from capabilities"
    );
    ensure!(actual == expected, "written pages differ from capabilities");
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
