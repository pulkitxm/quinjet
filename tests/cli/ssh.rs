use std::os::unix::fs::PermissionsExt;

use super::*;

fn fake_ssh(scratch: &Scratch) -> Result<(PathBuf, PathBuf)> {
    let bin = scratch.environment.join("ssh-bin");
    let executable = bin.join("ssh");
    let capture = scratch.environment.join("ssh-arguments");
    fs::create_dir_all(&bin)?;
    fs::write(
        &executable,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" >> \"$SSH_CAPTURE\"\nif [ -n \"$SSH_EXIT_CODE\" ]; then\n  exit \"$SSH_EXIT_CODE\"\nfi\nif [ -n \"$SSH_SWITCH_TARGET\" ] && [ \"$2\" = \"$SSH_SWITCH_TARGET\" ]; then\n  case \"$3\" in\n    *QUINJET_SSH_CONTEXT*) exit \"$SSH_SWITCH_CODE\" ;;\n  esac\nfi\nexit 0\n",
    )?;
    let mut permissions = fs::metadata(&executable)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions)?;
    Ok((bin, capture))
}

fn ssh_command(scratch: &Scratch, bin: &Path, capture: &Path, args: &[&str]) -> Result<Run> {
    let mut paths = vec![bin.to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    let mut command = ProcessCommand::new(env!("CARGO_BIN_EXE_quinjet"));
    command
        .args(args)
        .env("PATH", std::env::join_paths(paths)?)
        .env("SSH_CAPTURE", capture)
        .env("QUINJET_REMOTE_BINARY", "quinjet test");
    isolate_git(&mut command);
    isolate_quinjet(&mut command, &scratch.environment);
    Run::from(command.output()?)
}

#[test]
fn remote_command_forwards_the_folder_and_records_a_reachable_recent() -> Result<()> {
    let scratch = Scratch::directory()?;
    let (bin, capture) = fake_ssh(&scratch)?;
    drop(
        ssh_command(
            &scratch,
            &bin,
            &capture,
            &[
                "--remote",
                "test-host",
                "--folder",
                "/srv/a project",
                "status",
                "--json",
            ],
        )?
        .success()?,
    );
    let arguments = fs::read_to_string(&capture)?;
    ensure!(arguments.starts_with("--\ntest-host\n"));
    ensure!(
        arguments.contains("'quinjet test' --path '/srv/a project' status --json"),
        "unexpected remote command: {arguments}"
    );

    let recent = ssh_command(&scratch, &bin, &capture, &["remote", "list", "--json"])?.success()?;
    let document = recent.json()?;
    ensure!(document["remotes"][0]["target"] == "test-host");
    ensure!(document["remotes"][0]["folder"] == "/srv/a project");
    ensure!(document["remotes"][0]["accessible"] == true);
    ensure!(document["remotes"][0]["uses"] == 1);
    Ok(())
}

#[test]
fn implicit_remote_terminal_opens_the_folder_through_the_tui_verb() -> Result<()> {
    let scratch = Scratch::directory()?;
    let (bin, capture) = fake_ssh(&scratch)?;
    drop(
        ssh_command(
            &scratch,
            &bin,
            &capture,
            &["--remote", "test-host", "--folder", "/srv/a project"],
        )?
        .success()?,
    );
    let arguments = fs::read_to_string(&capture)?;
    ensure!(
        arguments.contains("'quinjet test' tui '/srv/a project'"),
        "unexpected remote command: {arguments}"
    );
    Ok(())
}

#[test]
fn unreachable_ssh_maps_to_the_unavailable_exit_code() -> Result<()> {
    let scratch = Scratch::directory()?;
    let (bin, capture) = fake_ssh(&scratch)?;
    let mut paths = vec![bin];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    let mut command = ProcessCommand::new(env!("CARGO_BIN_EXE_quinjet"));
    command
        .args(["--remote", "offline-host", "status"])
        .env("PATH", std::env::join_paths(paths)?)
        .env("SSH_CAPTURE", capture)
        .env("SSH_EXIT_CODE", "255");
    isolate_git(&mut command);
    isolate_quinjet(&mut command, &scratch.environment);
    let run = Run::from(command.output()?)?;
    ensure!(run.code == 4, "SSH unavailability exited {}", run.code);
    Ok(())
}

#[test]
fn terminal_machine_selection_reconnects_to_the_ranked_target() -> Result<()> {
    let scratch = Scratch::directory()?;
    let (bin, capture) = fake_ssh(&scratch)?;
    drop(
        ssh_command(
            &scratch,
            &bin,
            &capture,
            &["--remote", "second-host", "--folder", "/second", "status"],
        )?
        .success()?,
    );
    drop(
        ssh_command(
            &scratch,
            &bin,
            &capture,
            &["--remote", "second-host", "--folder", "/second", "status"],
        )?
        .success()?,
    );
    fs::write(&capture, "")?;
    let mut paths = vec![bin];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    let mut command = ProcessCommand::new(env!("CARGO_BIN_EXE_quinjet"));
    command
        .args(["--remote", "first-host", "--folder", "/first"])
        .env("PATH", std::env::join_paths(paths)?)
        .env("SSH_CAPTURE", &capture)
        .env("SSH_SWITCH_TARGET", "first-host")
        .env("SSH_SWITCH_CODE", "80")
        .env("QUINJET_REMOTE_BINARY", "quinjet test");
    isolate_git(&mut command);
    isolate_quinjet(&mut command, &scratch.environment);
    drop(Run::from(command.output()?)?.success()?);

    let arguments = fs::read_to_string(capture)?;
    ensure!(arguments.contains("\nfirst-host\n"));
    ensure!(arguments.contains("\nsecond-host\n"));
    ensure!(arguments.contains("\"current\":\"first-host\""));
    ensure!(arguments.contains("\"target\":\"second-host\""));
    ensure!(arguments.contains("'quinjet test' tui /second"));
    ensure!(arguments.matches("QUINJET_SSH_CONTEXT=").count() == 2);
    ensure!(arguments.matches("QUINJET_INHERITED_TERMINAL=1").count() == 1);
    Ok(())
}
