use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::{env, thread};

use anyhow::{Context, Result};

use crate::ssh::{SshContext, SshProjectOpenMode};

const LOCAL_BINARY_ENV: &str = "QUINJET_LOCAL_BINARY";

pub(crate) fn run_selected_terminal(
    target: &str,
    folder: &Path,
    context: SshContext,
    mode: SshProjectOpenMode,
) -> Result<u8> {
    super::validate_target(target)?;
    let binary = env::var(super::REMOTE_BINARY_ENV).unwrap_or_else(|_| "quinjet".to_owned());
    let original_arguments = wild::args_os().skip(1).collect::<Vec<_>>();
    run_terminal_loop(
        target,
        folder,
        &binary,
        &original_arguments,
        (false, true, Some(context), None),
        Some(mode),
    )
}

pub(super) fn run_terminal_loop(
    target: &str,
    folder: &Path,
    binary: &str,
    original_arguments: &[OsString],
    handoff: (bool, bool, Option<SshContext>, Option<std::path::PathBuf>),
    initial_project_mode: Option<SshProjectOpenMode>,
) -> Result<u8> {
    let _terminal = crate::terminal::HandoffTerminalGuard::enter()?;
    let (implicit_terminal, mut switched, context, control_path) = handoff;
    let (mut current_target, mut current_folder) = (target.to_owned(), folder.to_path_buf());
    let mut current_local = false;
    let mut project_mode = initial_project_mode;
    let mut context =
        context.unwrap_or_else(|| super::ssh_context(&current_target, &current_folder));
    loop {
        let arguments = if implicit_terminal || switched {
            super::switched_terminal_arguments(original_arguments.to_owned(), &current_folder)?
        } else {
            super::forwarded_arguments(original_arguments.to_owned())?
        };
        let outcome = if current_local {
            local_status(
                &arguments,
                &context,
                project_mode.unwrap_or(SshProjectOpenMode::Current),
            )?
        } else {
            super::ssh_status(
                &current_target,
                binary,
                &arguments,
                Some(&context),
                super::TerminalRelay {
                    allocate: true,
                    inherited: switched,
                    project_mode,
                },
                (current_target == target)
                    .then_some(control_path.as_deref())
                    .flatten(),
            )?
        };
        if let Some(updated) = outcome.context {
            context = updated;
        }
        let status = outcome.status;
        let code = status
            .code()
            .unwrap_or_else(|| i32::from(super::super::EXIT_FAILURE));
        if let Some(request) = crate::ssh::switch_request(code)
            && let Some(machine) = context.machines.get(request.index)
        {
            if !current_local {
                crate::state::record_recent_remote(&current_target, &current_folder);
            }
            current_target.clone_from(&machine.target);
            current_folder.clone_from(&machine.folder);
            current_local = machine.local;
            context.current.clone_from(&current_target);
            project_mode = Some(request.mode);
            switched = true;
            continue;
        }
        if status.success() && !current_local {
            crate::state::record_recent_remote(&current_target, &current_folder);
        }
        if switched {
            crate::terminal::restore_inherited_terminal();
        }
        return Ok(super::exit_code(status));
    }
}

fn local_status(
    arguments: &[OsString],
    context: &SshContext,
    mode: SshProjectOpenMode,
) -> Result<super::TerminalStatus> {
    let executable = env::var_os(LOCAL_BINARY_ENV).map_or_else(
        || {
            env::current_exe() // nosemgrep: rust.lang.security.current-exe.current-exe
                .context("failed to locate the local Quinjet binary")
        },
        |path| Ok(path.into()),
    )?;
    let serialized = serde_json::to_string(context)?;
    let mut command = Command::new(executable);
    let _command = command
        .args(arguments)
        .env("QUINJET_SSH_CONTEXT", serialized)
        .env(crate::terminal::INHERITED_TERMINAL_ENV, "1")
        .env(crate::ssh::OPEN_PROJECTS_ENV, mode.environment_value());
    relay_status(
        &mut command,
        "failed to return to the local Quinjet session",
    )
}

pub(super) fn relay_status(command: &mut Command, failure: &str) -> Result<super::TerminalStatus> {
    let mut child = command
        .stdout(Stdio::piped())
        .spawn()
        .with_context(|| failure.to_owned())?;
    let output = child
        .stdout
        .take()
        .context("terminal output was not piped")?;
    let relay = thread::spawn(move || relay_output(output, io::stdout()));
    let status = child.wait().with_context(|| failure.to_owned())?;
    let context = relay
        .join()
        .map_err(|_| anyhow::anyhow!("terminal output relay stopped unexpectedly"))??;
    Ok(super::TerminalStatus { status, context })
}

fn relay_output(mut input: impl Read, mut output: impl Write) -> Result<Option<SshContext>> {
    let prefix = crate::ssh::HANDOFF_CONTEXT_PREFIX;
    let mut buffer = [0_u8; 8192];
    let mut candidate = Vec::with_capacity(prefix.len());
    let mut payload = None::<Vec<u8>>;
    let mut context = None;
    loop {
        let read = input
            .read(&mut buffer)
            .context("failed to read terminal output")?;
        if read == 0 {
            break;
        }
        let Some(bytes) = buffer.get(..read) else {
            continue;
        };
        for &byte in bytes {
            if let Some(frame) = payload.as_mut() {
                if byte == crate::ssh::HANDOFF_CONTEXT_SUFFIX {
                    context = serde_json::from_slice(frame).ok();
                    payload = None;
                } else {
                    frame.push(byte);
                }
                continue;
            }
            let expected = prefix.get(candidate.len()).copied();
            if expected == Some(byte) {
                candidate.push(byte);
                if candidate.len() == prefix.len() {
                    candidate.clear();
                    payload = Some(Vec::new());
                }
                continue;
            }
            if !candidate.is_empty() {
                output
                    .write_all(&candidate)
                    .context("failed to relay terminal output")?;
                candidate.clear();
            }
            if prefix.first().copied() == Some(byte) {
                candidate.push(byte);
            } else {
                output
                    .write_all(&[byte])
                    .context("failed to relay terminal output")?;
            }
        }
        output.flush().context("failed to flush terminal output")?;
    }
    if let Some(frame) = payload {
        output
            .write_all(prefix)
            .and_then(|()| output.write_all(&frame))
            .context("failed to relay incomplete terminal handoff")?;
    } else if !candidate.is_empty() {
        output
            .write_all(&candidate)
            .context("failed to relay terminal output")?;
    }
    output.flush().context("failed to flush terminal output")?;
    Ok(context)
}

pub(super) fn remote_command(
    binary: &str,
    arguments: &[OsString],
    context: Option<&SshContext>,
    inherited_terminal: bool,
    project_mode: Option<SshProjectOpenMode>,
) -> Result<String> {
    let mut command = super::quote(binary)?;
    for argument in arguments {
        command.push(' ');
        command.push_str(&super::quote(&argument.to_string_lossy())?);
    }
    if let Some(context) = context {
        let serialized = serde_json::to_string(context)?;
        command = format!(
            "QUINJET_SSH_CONTEXT={} {command}",
            super::quote(&serialized)?
        );
    }
    if inherited_terminal {
        let mode = project_mode.unwrap_or(SshProjectOpenMode::Current);
        command = format!(
            "{}=1 {}={} {command}",
            crate::terminal::INHERITED_TERMINAL_ENV,
            crate::ssh::OPEN_PROJECTS_ENV,
            mode.environment_value()
        );
    }
    Ok(command)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn remote_command_quotes_every_argument() {
        let command = remote_command(
            "quinjet test",
            &[OsString::from("--path"), OsString::from("a'b c")],
            None,
            false,
            None,
        )
        .unwrap();
        assert_eq!(command, "'quinjet test' --path \"a'b c\"");
    }

    #[test]
    fn switched_terminal_inherits_the_existing_alternate_screen() {
        let command =
            remote_command("quinjet", &[], None, true, Some(SshProjectOpenMode::New)).unwrap();
        assert_eq!(
            command,
            "QUINJET_INHERITED_TERMINAL=1 QUINJET_OPEN_PROJECTS=new-tab quinjet"
        );
    }

    #[test]
    fn terminal_relay_hides_and_decodes_a_fragmented_handoff_frame() {
        let mut tabs = crate::ssh::SshTabs::default();
        let _id = tabs.append("macbook", "repo", "/work/repo");
        let context = SshContext {
            current: "macbook".to_owned(),
            machines: Vec::new(),
            tabs,
            probing: false,
        };
        let mut input = vec![b'x'; 8191];
        input.extend_from_slice(crate::ssh::HANDOFF_CONTEXT_PREFIX);
        input.extend_from_slice(&serde_json::to_vec(&context).unwrap());
        input.push(crate::ssh::HANDOFF_CONTEXT_SUFFIX);
        input.extend_from_slice(b"ready");
        let mut output = Vec::new();

        let decoded = relay_output(Cursor::new(input), &mut output).unwrap();

        assert_eq!(decoded, Some(context));
        assert_eq!(output.len(), 8196);
        assert!(output.starts_with(&[b'x'; 8191]));
        assert!(output.ends_with(b"ready"));
    }
}
