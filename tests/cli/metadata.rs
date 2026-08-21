use super::*;

#[test]
fn version_names_the_binary() -> Result<()> {
    let run = run_in(None, &["--version"])?.success()?;
    ensure!(
        run.stdout.starts_with("quinjet "),
        "unexpected version line: {}",
        run.stdout
    );
    Ok(())
}

#[test]
fn help_lists_every_group_verb() -> Result<()> {
    let run = run_in(None, &["--help"])?.success()?;
    for verb in [
        "tui",
        "status",
        "diff",
        "stage",
        "unstage",
        "discard",
        "commit",
        "fetch",
        "pull",
        "push",
        "sync",
        "log",
        "show",
        "branch",
        "stash",
        "cherry-pick",
        "revert",
        "resolve",
        "repos",
        "pr",
        "completions",
        "man",
        "capabilities",
        "update",
    ] {
        ensure!(run.stdout.contains(verb), "--help does not mention {verb}");
    }
    Ok(())
}

#[test]
fn tui_help_lists_every_theme_and_appearance() -> Result<()> {
    let run = run_in(None, &["tui", "--help"])?.success()?;
    ensure!(run.stdout.contains("--theme <THEME>"));
    ensure!(run.stdout.contains("--appearance <APPEARANCE>"));
    for theme in [
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
    ] {
        ensure!(run.stdout.contains(theme), "tui help omitted {theme}");
    }
    for appearance in ["system", "light", "dark"] {
        ensure!(
            run.stdout.contains(appearance),
            "tui help omitted {appearance}"
        );
    }
    Ok(())
}

#[test]
fn tui_accepts_theme_and_appearance_before_claiming_the_terminal() -> Result<()> {
    let run = run_in(
        None,
        &["tui", "--theme", "catppuccin", "--appearance", "dark"],
    )?;
    ensure!(run.code == 1, "non-interactive TUI exited {}", run.code);
    ensure!(
        run.stderr.contains("requires an interactive terminal"),
        "theme arguments did not reach TUI startup: {}",
        run.stderr
    );
    ensure!(!run.stderr.contains("unexpected argument"));
    Ok(())
}

#[test]
fn every_subcommand_answers_help() -> Result<()> {
    let capabilities = run_in(None, &["capabilities", "--json"])?
        .success()?
        .json()?;
    let commands = capabilities["commands"]
        .as_array()
        .context("capabilities commands were not an array")?;
    for command in commands {
        let path = command["path"]
            .as_str()
            .context("a capability command omitted its path")?;
        let Some(path) = path.strip_prefix("quinjet ") else {
            continue;
        };
        let mut args = path.split_whitespace().collect::<Vec<_>>();
        args.push("--help");
        let run = run_in(None, &args)?;
        ensure!(run.code == 0, "{path:?} --help exited {}", run.code);
    }
    Ok(())
}

#[test]
fn root_help_leads_with_examples_and_documentation() -> Result<()> {
    let run = run_in(None, &["--help"])?.success()?;
    ensure!(run.stdout.contains("Examples:"));
    ensure!(run.stdout.contains("quinjet status --json"));
    ensure!(run.stdout.contains("Documentation:"));
    Ok(())
}

#[test]
fn unknown_verbs_and_implicit_paths_are_usage_errors() -> Result<()> {
    for value in ["statsu", "/tmp/somewhere"] {
        let run = run_in(None, &[value])?;
        ensure!(run.code == 2, "{value} exited {}", run.code);
        ensure!(
            run.stderr.contains("unrecognized subcommand"),
            "unexpected error for {value}: {}",
            run.stderr
        );
    }
    Ok(())
}

#[test]
fn clap_rejects_incomplete_and_inert_arguments() -> Result<()> {
    for args in [
        &["stage"][..],
        &["unstage"][..],
        &["discard"][..],
        &["resolve", "README.md"][..],
        &["status", "--interval", "0"][..],
        &["pr", "checks", "1", "--watch", "--exit-code"][..],
    ] {
        let run = run_in(None, args)?;
        ensure!(run.code == 2, "{args:?} exited {}", run.code);
        ensure!(!run.stderr.is_empty(), "{args:?} produced no usage error");
    }
    Ok(())
}

#[test]
fn the_pr_launch_flag_is_refused_alongside_a_verb() -> Result<()> {
    let run = run_in(None, &["--pr", "12", "status"])?;
    ensure!(run.code == 1, "expected exit 1, got {}", run.code);
    ensure!(
        run.stderr.contains("terminal interface"),
        "the error does not explain where --pr applies: {}",
        run.stderr
    );
    Ok(())
}

#[test]
fn unknown_flags_are_usage_errors() -> Result<()> {
    let run = run_in(None, &["status", "--no-such-flag"])?;
    ensure!(run.code == 2, "expected exit 2, got {}", run.code);
    ensure!(
        run.stderr.contains("--no-such-flag"),
        "usage error does not name the flag"
    );
    Ok(())
}

#[test]
fn a_missing_repository_is_a_plain_failure() -> Result<()> {
    let scratch = Scratch::directory()?;
    let run = scratch.quinjet(&["status"])?;
    ensure!(run.code == 1, "expected exit 1, got {}", run.code);
    ensure!(
        run.stderr.contains("error:"),
        "failure did not report on stderr: {}",
        run.stderr
    );
    Ok(())
}

#[test]
fn status_reports_the_branch_in_both_faces() -> Result<()> {
    let scratch = Scratch::repository()?;
    let text = scratch.quinjet(&["status"])?.success()?;
    ensure!(
        text.stdout.contains("main"),
        "status does not name the branch: {}",
        text.stdout
    );
    let json = scratch.quinjet(&["status", "--json"])?.success()?;
    let document = json.json()?;
    ensure!(
        document["branch"]["head"] == "main",
        "unexpected JSON branch: {document}"
    );
    Ok(())
}

#[test]
fn captured_and_json_output_never_include_progress() -> Result<()> {
    let scratch = Scratch::repository()?;
    for args in [vec!["status"], vec!["status", "--json"]] {
        let run = scratch.quinjet(&args)?.success()?;
        ensure!(
            run.stderr.is_empty(),
            "{args:?} wrote progress to captured stderr: {}",
            run.stderr
        );
    }
    Ok(())
}
