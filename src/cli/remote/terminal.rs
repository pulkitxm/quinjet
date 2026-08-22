use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

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
        (false, true, Some(context)),
        Some(mode),
    )
}

pub(super) fn run_terminal_loop(
    target: &str,
    folder: &Path,
    binary: &str,
    original_arguments: &[OsString],
    handoff: (bool, bool, Option<SshContext>),
    initial_project_mode: Option<SshProjectOpenMode>,
) -> Result<u8> {
    let (implicit_terminal, mut switched, context) = handoff;
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
        let status = if current_local {
            local_status(
                &arguments,
                &context,
                project_mode.unwrap_or(SshProjectOpenMode::CurrentTab),
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
            )?
        };
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
) -> Result<std::process::ExitStatus> {
    let executable = env::var_os(LOCAL_BINARY_ENV).map_or_else(
        || {
            env::current_exe() // nosemgrep: rust.lang.security.current-exe.current-exe
                .context("failed to locate the local Quinjet binary")
        },
        |path| Ok(path.into()),
    )?;
    let serialized = serde_json::to_string(context)?;
    Command::new(executable)
        .args(arguments)
        .env("QUINJET_SSH_CONTEXT", serialized)
        .env(crate::terminal::INHERITED_TERMINAL_ENV, "1")
        .env(crate::ssh::OPEN_PROJECTS_ENV, mode.environment_value())
        .status()
        .context("failed to return to the local Quinjet session")
}
