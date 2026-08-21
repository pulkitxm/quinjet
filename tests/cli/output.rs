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
