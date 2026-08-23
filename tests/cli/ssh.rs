use std::os::unix::fs::PermissionsExt;

use super::*;

fn fake_ssh(scratch: &Scratch) -> Result<(PathBuf, PathBuf)> {
    let bin = scratch.environment.join("ssh-bin");
    let executable = bin.join("ssh");
    let capture = scratch.environment.join("ssh-arguments");
    fs::create_dir_all(&bin)?;
    fs::write(
        &executable,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" >> \"$SSH_CAPTURE\"\nif [ -n \"$SSH_EXIT_CODE\" ]; then\n  exit \"$SSH_EXIT_CODE\"\nfi\ntarget=\nremote_command=\nwhile [ \"$#\" -gt 0 ]; do\n  case \"$1\" in\n    -q|-tt) shift ;;\n    -S) shift 2 ;;\n    --)\n      shift\n      target=$1\n      shift\n      remote_command=$1\n      break\n      ;;\n    *) shift ;;\n  esac\ndone\nif [ -n \"$SSH_SWITCH_TARGET\" ] && [ \"$target\" = \"$SSH_SWITCH_TARGET\" ]; then\n  case \"$remote_command\" in\n    *QUINJET_SSH_CONTEXT*)\n      if [ -n \"$SSH_SWITCH_CONTEXT\" ]; then\n        printf '\\033]777;quinjet-context=%s\\007' \"$SSH_SWITCH_CONTEXT\"\n      fi\n      exit \"$SSH_SWITCH_CODE\"\n      ;;\n  esac\nfi\nexit 0\n",
    )?;
    let mut permissions = fs::metadata(&executable)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions)?;
    Ok((bin, capture))
}

fn fake_local(scratch: &Scratch) -> Result<(PathBuf, PathBuf)> {
    let executable = scratch.environment.join("local-quinjet");
    let capture = scratch.environment.join("local-arguments");
    fs::write(
        &executable,
        "#!/bin/sh\nprintf 'arguments:%s\\n' \"$*\" > \"$LOCAL_CAPTURE\"\nprintf 'context:%s\\n' \"$QUINJET_SSH_CONTEXT\" >> \"$LOCAL_CAPTURE\"\nprintf 'inherited:%s\\n' \"$QUINJET_INHERITED_TERMINAL\" >> \"$LOCAL_CAPTURE\"\nprintf 'projects:%s\\n' \"$QUINJET_OPEN_PROJECTS\" >> \"$LOCAL_CAPTURE\"\n",
    )?;
    let mut permissions = fs::metadata(&executable)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions)?;
    Ok((executable, capture))
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
fn remote_command_reuses_an_existing_control_socket() -> Result<()> {
    let scratch = Scratch::directory()?;
    let (bin, capture) = fake_ssh(&scratch)?;
    let socket = scratch.environment.join("edith.sock");
    drop(
        ssh_command(
            &scratch,
            &bin,
            &capture,
            &[
                "--remote",
                "test-host",
                "--ssh-control-path",
                socket.to_str().context("socket path was not UTF-8")?,
                "--folder",
                "/srv/project",
                "status",
            ],
        )?
        .success()?,
    );

    let arguments = fs::read_to_string(&capture)?;
    ensure!(
        arguments.starts_with(&format!("-S\n{}\n--\ntest-host\n", socket.display())),
        "unexpected SSH arguments: {arguments}"
    );
    ensure!(
        !arguments
            .lines()
            .last()
            .unwrap_or_default()
            .contains("ssh-control-path"),
        "the remote command received a host-only option: {arguments}"
    );
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
        .env("SSH_SWITCH_CODE", "81")
        .env("QUINJET_REMOTE_BINARY", "quinjet test");
    isolate_git(&mut command);
    isolate_quinjet(&mut command, &scratch.environment);
    drop(Run::from(command.output()?)?.success()?);

    let arguments = fs::read_to_string(capture)?;
    ensure!(arguments.contains("-q\n--\nfirst-host\n"));
    ensure!(arguments.contains("\nfirst-host\n"));
    ensure!(arguments.contains("\nsecond-host\n"));
    ensure!(arguments.contains("\"current\":\"first-host\""));
    ensure!(arguments.contains("\"local\":true"));
    ensure!(arguments.contains("\"target\":\"second-host\""));
    ensure!(arguments.contains("'quinjet test' tui /second"));
    ensure!(arguments.matches("QUINJET_SSH_CONTEXT=").count() == 2);
    ensure!(arguments.matches("QUINJET_INHERITED_TERMINAL=1").count() == 1);
    Ok(())
}

#[test]
fn terminal_machine_selection_returns_to_the_named_host_picker() -> Result<()> {
    let scratch = Scratch::directory()?;
    let (bin, ssh_capture) = fake_ssh(&scratch)?;
    let (local_binary, local_capture) = fake_local(&scratch)?;
    let mut paths = vec![bin];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    let mut command = ProcessCommand::new(env!("CARGO_BIN_EXE_quinjet"));
    command
        .args(["--remote", "first-host", "--folder", "/first"])
        .env("PATH", std::env::join_paths(paths)?)
        .env("SSH_CAPTURE", &ssh_capture)
        .env("SSH_SWITCH_TARGET", "first-host")
        .env("SSH_SWITCH_CODE", "80")
        .env("QUINJET_LOCAL_BINARY", local_binary)
        .env("LOCAL_CAPTURE", &local_capture)
        .env("QUINJET_REMOTE_BINARY", "quinjet test");
    isolate_git(&mut command);
    isolate_quinjet(&mut command, &scratch.environment);
    drop(Run::from(command.output()?)?.success()?);

    let local = fs::read_to_string(local_capture)?;
    ensure!(local.contains("arguments:tui "));
    ensure!(local.contains("\"local\":true"));
    ensure!(local.contains("inherited:1"));
    ensure!(local.contains("projects:current-tab"));
    let ssh = fs::read_to_string(ssh_capture)?;
    ensure!(ssh.matches("\nfirst-host\n").count() == 1);
    Ok(())
}

#[test]
fn terminal_machine_selection_preserves_new_tab_mode_on_the_host() -> Result<()> {
    let scratch = Scratch::directory()?;
    let (bin, ssh_capture) = fake_ssh(&scratch)?;
    let (local_binary, local_capture) = fake_local(&scratch)?;
    let mut paths = vec![bin];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    let mut command = ProcessCommand::new(env!("CARGO_BIN_EXE_quinjet"));
    command
        .args(["--remote", "first-host", "--folder", "/first"])
        .env("PATH", std::env::join_paths(paths)?)
        .env("SSH_CAPTURE", &ssh_capture)
        .env("SSH_SWITCH_TARGET", "first-host")
        .env("SSH_SWITCH_CODE", "96")
        .env("QUINJET_LOCAL_BINARY", local_binary)
        .env("LOCAL_CAPTURE", &local_capture)
        .env("QUINJET_REMOTE_BINARY", "quinjet test");
    isolate_git(&mut command);
    isolate_quinjet(&mut command, &scratch.environment);
    drop(Run::from(command.output()?)?.success()?);

    let local = fs::read_to_string(local_capture)?;
    ensure!(local.contains("arguments:tui "));
    ensure!(local.contains("inherited:1"));
    ensure!(local.contains("projects:new-tab"));
    let ssh = fs::read_to_string(ssh_capture)?;
    ensure!(ssh.matches("\nfirst-host\n").count() == 1);
    Ok(())
}

#[test]
fn terminal_tab_handoff_relays_the_shared_strip_to_its_owner() -> Result<()> {
    let scratch = Scratch::directory()?;
    let (bin, capture) = fake_ssh(&scratch)?;
    let mut paths = vec![bin];
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    let relayed = r#"{"current":"first-host","machines":[{"target":"local","folder":"/local","accessible":true,"uses":0,"local":true},{"target":"second-host","folder":"/second","accessible":true,"uses":2,"local":false}],"tabs":{"entries":[{"id":7,"machine":"second-host","title":"remote-repo","root":"/second/repo"}],"active":7,"activeByMachine":{"second-host":7},"nextId":8}}"#;
    let mut command = ProcessCommand::new(env!("CARGO_BIN_EXE_quinjet"));
    command
        .args(["--remote", "first-host", "--folder", "/first"])
        .env("PATH", std::env::join_paths(paths)?)
        .env("SSH_CAPTURE", &capture)
        .env("SSH_SWITCH_TARGET", "first-host")
        .env("SSH_SWITCH_CODE", "113")
        .env("SSH_SWITCH_CONTEXT", relayed)
        .env("QUINJET_REMOTE_BINARY", "quinjet test");
    isolate_git(&mut command);
    isolate_quinjet(&mut command, &scratch.environment);

    drop(Run::from(command.output()?)?.success()?);

    let arguments = fs::read_to_string(capture)?;
    ensure!(arguments.contains("\nsecond-host\n"));
    ensure!(arguments.contains("QUINJET_OPEN_PROJECTS=activate-tab"));
    ensure!(arguments.contains("remote-repo"));
    ensure!(arguments.contains("/second/repo"));
    Ok(())
}
