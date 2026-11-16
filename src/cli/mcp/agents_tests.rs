use super::*;

fn launcher() -> Launcher {
    Launcher {
        command: "/opt/quinjet \"q\"".to_owned(),
        args: vec!["mcp".to_owned()],
    }
}

#[test]
fn json_configs_gain_the_server_and_keep_everything_else() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mcp.json");
    fs::write(
        &path,
        r#"{"theme": "dark", "mcpServers": {"other": {"command": "x"}}}"#,
    )
    .unwrap();
    assert_eq!(
        json_file(&path, Agent::Cursor, &launcher()).unwrap(),
        Outcome::Added
    );
    let written: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(written["theme"], "dark");
    assert_eq!(written["mcpServers"]["other"]["command"], "x");
    assert_eq!(
        written["mcpServers"]["quinjet"],
        json!({"command": "/opt/quinjet \"q\"", "args": ["mcp"]})
    );
    assert_eq!(
        json_file(&path, Agent::Cursor, &launcher()).unwrap(),
        Outcome::Unchanged
    );
    let moved = Launcher {
        command: "quinjet".to_owned(),
        args: vec!["mcp".to_owned()],
    };
    assert_eq!(
        json_file(&path, Agent::Cursor, &moved).unwrap(),
        Outcome::Updated
    );
}

#[test]
fn vs_code_uses_its_own_section_and_server_type() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("User").join("mcp.json");
    assert_eq!(
        json_file(&path, Agent::VsCode, &launcher()).unwrap(),
        Outcome::Added
    );
    let written: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(written["servers"]["quinjet"]["type"], "stdio");
}

#[test]
fn commented_json_is_left_alone() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mcp.json");
    fs::write(&path, "{ // mine\n}").unwrap();
    let error = json_file(&path, Agent::VsCode, &launcher()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("add the `quinjet` server by hand")
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "{ // mine\n}");
}

#[test]
fn codex_tables_are_added_then_replaced_in_place() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    fs::write(
        &path,
        "model = \"o4\"\n\n[mcp_servers.other]\ncommand = \"x\"\n",
    )
    .unwrap();
    assert_eq!(toml_file(&path, &launcher()).unwrap(), Outcome::Added);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.ends_with(
        "[mcp_servers.quinjet]\ncommand = \"/opt/quinjet \\\"q\\\"\"\nargs = [\"mcp\"]\n"
    ));
    assert_eq!(toml_file(&path, &launcher()).unwrap(), Outcome::Unchanged);
    fs::write(
        &path,
        "[mcp_servers.quinjet]\ncommand = \"old\"\nargs = [\"mcp\"]\n\n[profiles.x]\nmodel = \"y\"\n",
    )
    .unwrap();
    assert_eq!(toml_file(&path, &launcher()).unwrap(), Outcome::Updated);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("[mcp_servers.quinjet]\ncommand = \"/opt/quinjet"));
    assert!(text.contains("[profiles.x]\nmodel = \"y\"\n"));
    assert!(!text.contains("old"));
}

#[test]
fn only_installed_agents_are_reported_unless_named() {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir_all(directory.path().join(".cursor")).unwrap();
    let found = connect(
        directory.path(),
        &launcher(),
        &[Agent::Cursor, Agent::Windsurf],
        false,
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].agent, Agent::Cursor);
    assert_eq!(found[0].outcome, Outcome::Added);
    let named = connect(directory.path(), &launcher(), &[Agent::Windsurf], true);
    assert_eq!(named[0].outcome, Outcome::Added);
    assert!(
        directory
            .path()
            .join(".codeium/windsurf/mcp_config.json")
            .is_file()
    );
    assert!(render(&found).starts_with("Cursor       connected"));
    assert!(render(&[]).starts_with("No supported AI agents were found"));
}
