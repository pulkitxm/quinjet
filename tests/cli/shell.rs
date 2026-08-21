use super::*;

#[cfg(not(windows))]
#[test]
fn shell_integration_makes_q_immediate_without_restoring_removals() -> Result<()> {
    let scratch = Scratch::directory()?;
    let bin = scratch.path.join("bin");
    let data = scratch.path.join("data");
    let state = scratch.path.join(".local/state/quinjet");
    let executable = bin.join("quinjet");
    let shortcut = bin.join("q");
    let completion = data.join("bash-completion/completions/quinjet");
    let bashrc = scratch.path.join(".bashrc");
    fs::create_dir_all(&bin)?;
    fs::create_dir_all(&state)?;
    fs::create_dir_all(completion.parent().context("completion had no parent")?)?;
    let staged = bin.join("quinjet-stage");
    fs::copy(env!("CARGO_BIN_EXE_quinjet"), &staged)?;
    fs::rename(staged, &executable)?;
    fs::write(state.join("bash-installed"), "installed\n")?;
    fs::write(&completion, "stale completion\n")?;
    fs::write(
        &bashrc,
        "# >>> quinjet shortcut >>>\nalias q='quinjet'\n# <<< quinjet shortcut <<<\n",
    )?;

    let maintain = || -> Result<Run> {
        let mut update = ProcessCommand::new(&executable);
        update
            .args(["completions", "bash", "--install", "--automatic"])
            .env("HOME", &scratch.path)
            .env("XDG_DATA_HOME", &data)
            .env("PATH", &bin)
            .env("SHELL", "/bin/bash")
            .env("PSModulePath", "inherited-but-not-active");
        isolate_git(&mut update);
        Run::from(copied_binary_output(
            &mut update,
            "failed to refresh completions",
        )?)?
        .success()
    };

    fs::write(&shortcut, "unrelated q command\n")?;
    drop(maintain()?);
    ensure!(fs::read_to_string(&shortcut)? == "unrelated q command\n");
    ensure!(fs::read_to_string(&bashrc)?.contains("quinjet shortcut"));
    fs::remove_file(&shortcut)?;
    fs::remove_file(state.join("shortcut-installed"))?;

    drop(maintain()?);
    ensure!(fs::read_to_string(&completion)?.contains("complete -F _quinjet"));
    ensure!(shortcut.exists());
    ensure!(!fs::read_to_string(&bashrc)?.contains("quinjet shortcut"));
    ensure!(state.join("shortcut-installed").is_file());

    let mut path = vec![bin.clone()];
    if let Some(existing) = std::env::var_os("PATH") {
        path.extend(std::env::split_paths(&existing));
    }
    let mut invoke_q = ProcessCommand::new("q");
    invoke_q
        .arg("--version")
        .env("PATH", std::env::join_paths(path)?);
    let q = Run::from(copied_binary_output(
        &mut invoke_q,
        "q was unavailable in the current shell",
    )?)?
    .success()?;
    ensure!(q.stdout.contains("quinjet"));

    fs::remove_file(&shortcut)?;
    drop(maintain()?);
    ensure!(!shortcut.exists());

    fs::remove_file(&completion)?;
    drop(maintain()?);
    ensure!(!completion.exists());

    let mut restore = ProcessCommand::new(&executable);
    restore
        .args(["completions", "bash", "--install"])
        .env("HOME", &scratch.path)
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &bin)
        .env("SHELL", "/bin/bash");
    isolate_git(&mut restore);
    drop(
        Run::from(copied_binary_output(
            &mut restore,
            "failed to restore completions",
        )?)?
        .success()?,
    );
    ensure!(completion.exists());
    ensure!(shortcut.exists());
    Ok(())
}

#[cfg(all(not(windows), not(debug_assertions)))]
#[test]
fn shell_integration_without_a_detected_shell_still_installs_q() -> Result<()> {
    let scratch = Scratch::directory()?;
    let bin = scratch.path.join("bin");
    let executable = bin.join("quinjet");
    let shortcut = bin.join("q");
    fs::create_dir_all(&bin)?;
    let staged = bin.join("quinjet-stage");
    fs::copy(env!("CARGO_BIN_EXE_quinjet"), &staged)?;
    fs::rename(staged, &executable)?;

    let mut first_run = ProcessCommand::new(&executable);
    first_run
        .arg("--version")
        .env("HOME", &scratch.path)
        .env("PATH", &bin)
        .env_remove("PSModulePath")
        .env_remove("SHELL");
    isolate_git(&mut first_run);
    drop(Run::from(copied_binary_output(&mut first_run, "failed first run")?)?.success()?);
    ensure!(shortcut.exists());

    let mut invoke_q = ProcessCommand::new("q");
    invoke_q
        .arg("--version")
        .env("HOME", &scratch.path)
        .env("PATH", &bin)
        .env_remove("PSModulePath")
        .env_remove("SHELL");
    let q = Run::from(copied_binary_output(
        &mut invoke_q,
        "q was unavailable on PATH",
    )?)?
    .success()?;
    ensure!(q.stdout.contains("quinjet"));
    Ok(())
}

#[cfg(windows)]
#[test]
fn shell_integration_makes_q_immediate_on_windows() -> Result<()> {
    let scratch = Scratch::directory()?;
    let bin = scratch.path.join("bin");
    let data = scratch.path.join("data");
    let executable = bin.join("quinjet.exe");
    let shortcut = bin.join("q.cmd");
    fs::create_dir_all(&bin)?;
    let staged = bin.join("quinjet-stage.exe");
    fs::copy(env!("CARGO_BIN_EXE_quinjet"), &staged)?;
    fs::rename(staged, &executable)?;

    let mut install = ProcessCommand::new(&executable);
    install
        .args(["completions", "bash", "--install"])
        .env("HOME", &scratch.path)
        .env("USERPROFILE", &scratch.path)
        .env("LOCALAPPDATA", scratch.path.join("local"))
        .env("XDG_DATA_HOME", &data)
        .env("PATH", &bin);
    isolate_git(&mut install);
    drop(
        Run::from(
            install
                .output()
                .context("failed to install shell integration")?,
        )?
        .success()?,
    );
    ensure!(shortcut.exists());

    let mut path = vec![bin];
    if let Some(existing) = std::env::var_os("PATH") {
        path.extend(std::env::split_paths(&existing));
    }
    let mut invoke_q = ProcessCommand::new("cmd.exe");
    invoke_q
        .args(["/D", "/C", "q --version"])
        .env("PATH", std::env::join_paths(path)?);
    let q = Run::from(
        invoke_q
            .output()
            .context("q was unavailable in the current shell")?,
    )?
    .success()?;
    ensure!(q.stdout.contains("quinjet"));
    Ok(())
}
