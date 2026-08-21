use super::*;

#[test]
fn json_output_is_one_document_per_invocation() -> Result<()> {
    let scratch = Scratch::repository()?;
    for args in [
        vec!["status", "--json"],
        vec!["log", "-n", "2", "--json"],
        vec!["branch", "list", "--json"],
        vec!["stash", "list", "--json"],
        vec!["worktree", "list", "--json"],
        vec!["diff", "--json"],
    ] {
        let run = scratch.quinjet(&args)?.success()?;
        drop(run.json().with_context(|| format!("for {args:?}"))?);
    }
    Ok(())
}

#[test]
fn diff_json_exposes_theme_independent_syntax_roles() -> Result<()> {
    let scratch = Scratch::repository()?;
    scratch.write("main.rs", "fn main() {}\n")?;
    scratch.git(&["add", "main.rs"])?;
    scratch.git(&["commit", "--message=rust"])?;
    scratch.write("main.rs", "fn main() { let value = 1; }\n")?;
    let document = scratch.quinjet(&["diff", "--json"])?.success()?.json()?;
    let foregrounds: Vec<&str> = document["lines"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|line| line["spans"].as_array())
        .flatten()
        .filter_map(|span| span["foreground"].as_str())
        .collect();
    ensure!(
        !foregrounds.is_empty(),
        "highlighted diff JSON omitted semantic foregrounds"
    );
    let roles = [
        "text", "comment", "red", "orange", "yellow", "green", "cyan", "blue", "purple", "brown",
    ];
    for foreground in &foregrounds {
        ensure!(
            roles.contains(foreground),
            "unexpected syntax role: {foreground}"
        );
    }
    ensure!(
        foregrounds.iter().any(|foreground| *foreground != "text"),
        "syntax highlighting collapsed to the default text role"
    );
    Ok(())
}

#[test]
fn completions_cover_every_supported_shell() -> Result<()> {
    let scratch = Scratch::directory()?;
    for shell in ["bash", "zsh", "fish", "elvish", "powershell"] {
        let run = scratch.quinjet(&["completions", shell])?.success()?;
        ensure!(
            run.stdout.contains("quinjet"),
            "{shell} completions never mention the binary"
        );
    }
    let json = scratch
        .quinjet(&["completions", "bash", "--json"])?
        .success()?;
    let document = json.json()?;
    ensure!(
        document["shell"] == "bash",
        "unexpected completions JSON: {document}"
    );
    let alias = scratch.quinjet(&["completion", "bash"])?.success()?;
    ensure!(alias.stdout.contains("quinjet"));
    Ok(())
}

#[test]
fn status_json_covers_unborn_detached_and_mixed_worktrees() -> Result<()> {
    let unborn = Scratch::unborn_repository()?;
    let document = unborn.quinjet(&["status", "--json"])?.success()?.json()?;
    ensure!(document["branch"]["head"] == "main");
    ensure!(document["branch"]["oid"].is_null());
    ensure!(document["changes"] == serde_json::json!([]));

    let detached = Scratch::repository()?;
    detached.git(&["switch", "--detach"])?;
    let document = detached.quinjet(&["status", "--json"])?.success()?.json()?;
    ensure!(document["branch"]["detached"] == true);
    ensure!(
        document["branch"]["head"]
            .as_str()
            .is_some_and(|head| head.len() == 8)
    );

    let mixed = Scratch::repository()?;
    mixed.write("mixed.txt", "base\n")?;
    mixed.git(&["add", "mixed.txt"])?;
    mixed.git(&["commit", "--message=mixed base"])?;
    mixed.git(&["mv", "README.md", "renamed file.txt"])?;
    mixed.write("mixed.txt", "staged\n")?;
    mixed.git(&["add", "mixed.txt"])?;
    mixed.write("mixed.txt", "unstaged\n")?;
    mixed.write("目录/untracked file.txt", "new\n")?;
    let run = mixed.quinjet(&["status", "--json"])?.success()?;
    let document = run.json()?;
    let changes = document["changes"]
        .as_array()
        .context("status changes were not an array")?;
    ensure!(changes.iter().any(|change| {
        change["path"] == "renamed file.txt"
            && change["originalPath"] == "README.md"
            && change["status"] == "renamed"
            && change["area"] == "staged"
    }));
    for area in ["staged", "unstaged"] {
        ensure!(changes.iter().any(|change| {
            change["path"] == "mixed.txt"
                && change["status"] == "modified"
                && change["area"] == area
        }));
    }
    ensure!(changes.iter().any(|change| {
        change["path"] == "目录/untracked file.txt"
            && change["status"] == "untracked"
            && change["area"] == "unstaged"
    }));
    let text = mixed.quinjet(&["status"])?.success()?;
    ensure!(text.stdout.contains("Staged Changes"));
    ensure!(text.stdout.contains("Changes"));
    ensure!(text.stdout.contains("(from README.md)"));
    Ok(())
}

#[test]
fn status_reports_tracking_divergence_in_both_output_modes() -> Result<()> {
    let scratch = Scratch::repository()?;
    let base = scratch.git(&["rev-parse", "HEAD"])?;
    scratch.write("local.txt", "local\n")?;
    scratch.git(&["add", "local.txt"])?;
    scratch.git(&["commit", "--message=local"])?;
    scratch.git(&["switch", "--create", "remote-main", &base])?;
    scratch.write("remote.txt", "remote\n")?;
    scratch.git(&["add", "remote.txt"])?;
    scratch.git(&["commit", "--message=remote"])?;
    let remote = scratch.git(&["rev-parse", "HEAD"])?;
    scratch.git(&["switch", "main"])?;
    let root = scratch.path.display().to_string();
    scratch.git(&["remote", "add", "origin", &root])?;
    scratch.git(&["update-ref", "refs/remotes/origin/main", &remote])?;
    scratch.git(&["branch", "--set-upstream-to=origin/main", "main"])?;

    let text = scratch.quinjet(&["status"])?.success()?;
    ensure!(
        text.stdout
            .contains("Tracking origin/main ahead 1 behind 1"),
        "unexpected status: {}",
        text.stdout
    );
    let document = scratch.quinjet(&["status", "--json"])?.success()?.json()?;
    ensure!(document["branch"]["upstream"] == "origin/main");
    ensure!(document["branch"]["ahead"] == 1);
    ensure!(document["branch"]["behind"] == 1);
    Ok(())
}

#[test]
fn diff_filters_staged_unstaged_paths_and_context() -> Result<()> {
    let scratch = Scratch::repository()?;
    scratch.write(
        "context.txt",
        "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\neleven\ntwelve\n",
    )?;
    scratch.write("other.txt", "before\n")?;
    scratch.git(&["add", "context.txt", "other.txt"])?;
    scratch.git(&["commit", "--message=context"])?;
    scratch.write(
        "context.txt",
        "one\ntwo\nthree\nfour\nfive\nSIX\nseven\neight\nnine\nten\neleven\ntwelve\n",
    )?;
    scratch.write("other.txt", "after\n")?;

    let selected = scratch.quinjet(&["diff", "context.txt"])?.success()?;
    ensure!(selected.stdout.contains("SIX"));
    ensure!(!selected.stdout.contains("after"));
    ensure!(!selected.stdout.contains(" one\n"));
    let expanded = scratch
        .quinjet(&["diff", "context.txt", "--expanded"])?
        .success()?;
    ensure!(expanded.stdout.contains(" one\n"));

    scratch.git(&["add", "context.txt"])?;
    scratch.write(
        "context.txt",
        "one\ntwo\nthree\nfour\nfive\nSIX\nseven\neight\nnine\nten\neleven\ntwelve\nunstaged only\n",
    )?;
    let staged = scratch
        .quinjet(&["diff", "--staged", "context.txt"])?
        .success()?;
    ensure!(staged.stdout.contains("SIX"));
    ensure!(!staged.stdout.contains("unstaged only"));
    let unstaged = scratch
        .quinjet(&["diff", "--unstaged", "context.txt"])?
        .success()?;
    ensure!(unstaged.stdout.contains("unstaged only"));
    ensure!(!unstaged.stdout.contains("-six"));

    let missing = scratch
        .quinjet(&["diff", "missing.txt", "--json"])?
        .success()?
        .json()?;
    ensure!(missing == serde_json::json!({ "message": "No changes match" }));
    Ok(())
}

#[test]
fn log_paginates_and_accepts_named_and_abbreviated_revisions() -> Result<()> {
    let scratch = Scratch::repository()?;
    let base = scratch.git(&["rev-parse", "HEAD"])?;
    scratch.git(&["tag", "v-base", &base])?;
    for subject in ["first", "second", "third"] {
        scratch.write("history.txt", &format!("{subject}\n"))?;
        scratch.git(&["add", "history.txt"])?;
        scratch.git(&["commit", &format!("--message={subject}")])?;
    }

    let page = scratch
        .quinjet(&["log", "--skip", "1", "--limit", "2", "--json"])?
        .success()?
        .json()?;
    let commits = page.as_array().context("log JSON was not an array")?;
    ensure!(commits.len() == 2);
    ensure!(commits[0]["subject"] == "second");
    ensure!(commits[1]["subject"] == "first");

    let tagged = scratch
        .quinjet(&["log", "v-base", "-n", "1", "--json"])?
        .success()?
        .json()?;
    ensure!(tagged[0]["id"] == base);
    let short = base.get(..8).context("base commit ID was too short")?;
    let abbreviated = scratch
        .quinjet(&["log", short, "-n", "1", "--json"])?
        .success()?
        .json()?;
    ensure!(abbreviated[0]["id"] == base);
    Ok(())
}

#[test]
fn show_handles_root_expanded_and_missing_revisions() -> Result<()> {
    let scratch = Scratch::repository()?;
    let root = scratch.git(&["rev-list", "--max-parents=0", "HEAD"])?;
    let before = scratch.git(&["rev-parse", "HEAD"])?;
    let shown = scratch
        .quinjet(&["show", &root, "--expanded", "--json"])?
        .success()?
        .json()?;
    ensure!(shown["commit"]["id"] == root);
    ensure!(shown["commit"]["subject"] == "base");
    ensure!(
        shown["diff"]["lines"]
            .as_array()
            .is_some_and(|lines| !lines.is_empty())
    );
    ensure!(scratch.git(&["rev-parse", "HEAD"])? == before);

    for verb in ["show", "log"] {
        let run = scratch.quinjet(&[verb, "does-not-exist", "--json"])?;
        ensure!(run.code == 3, "{verb} exited {}", run.code);
        ensure!(
            run.stdout.is_empty(),
            "{verb} failure wrote JSON: {}",
            run.stdout
        );
        ensure!(run.stderr.contains("error:"));
        ensure!(run.stderr.contains("hint:"));
    }
    Ok(())
}

#[test]
fn global_repository_and_json_options_work_at_each_command_depth() -> Result<()> {
    let scratch = Scratch::repository()?;
    let path = scratch.path.display().to_string();
    let before = run_in(None, &["--json", "-C", &path, "status"])?
        .success()?
        .json()?;
    let after = run_in(None, &["status", "--path", &path, "--json"])?
        .success()?
        .json()?;
    ensure!(before == after);

    let branches = run_in(None, &["branch", "list", "--json", "--path", &path])?
        .success()?
        .json()?;
    ensure!(branches.as_array().is_some_and(|values| values.len() == 1));
    ensure!(branches[0]["name"] == "main");
    Ok(())
}

#[test]
fn completion_json_matches_plain_scripts_for_every_shell() -> Result<()> {
    let scratch = Scratch::directory()?;
    for shell in ["bash", "zsh", "fish", "elvish", "powershell"] {
        let plain = scratch.quinjet(&["completions", shell])?.success()?;
        let json = scratch
            .quinjet(&["completions", shell, "--json"])?
            .success()?
            .json()?;
        ensure!(json["shell"] == shell);
        ensure!(json["script"] == plain.stdout);
    }
    Ok(())
}
