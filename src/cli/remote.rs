use std::ffi::{OsStr, OsString};
use std::io::{self, IsTerminal};
use std::path::Path;
use std::process::{Command, Stdio};
use std::{env, thread};

use anyhow::{Context, Result};
use serde::Serialize;

use super::{EXIT_FAILURE, EXIT_UNAVAILABLE, Emitter, RemoteVerb};

const REMOTE_BINARY_ENV: &str = "QUINJET_REMOTE_BINARY";

pub(super) fn run(
    target: &str,
    terminal: bool,
    implicit_terminal: bool,
    folder: &Path,
) -> Result<u8> {
    validate_target(target)?;
    let binary = env::var(REMOTE_BINARY_ENV).unwrap_or_else(|_| "quinjet".to_owned());
    let original_arguments = wild::args_os().skip(1).collect::<Vec<_>>();
    if !terminal {
        let arguments = forwarded_arguments(original_arguments)?;
        return run_once(target, folder, &binary, &arguments, None);
    }
    run_terminal_loop(
        target,
        folder,
        &binary,
        &original_arguments,
        implicit_terminal,
        false,
    )
}

pub(crate) fn run_selected_terminal(target: &str, folder: &Path) -> Result<u8> {
    validate_target(target)?;
    let binary = env::var(REMOTE_BINARY_ENV).unwrap_or_else(|_| "quinjet".to_owned());
    let original_arguments = wild::args_os().skip(1).collect::<Vec<_>>();
    run_terminal_loop(target, folder, &binary, &original_arguments, false, true)
}

fn run_terminal_loop(
    target: &str,
    folder: &Path,
    binary: &str,
    original_arguments: &[OsString],
    implicit_terminal: bool,
    mut switched: bool,
) -> Result<u8> {
    let mut current_target = target.to_owned();
    let mut current_folder = folder.to_path_buf();
    loop {
        let arguments = if implicit_terminal || switched {
            switched_terminal_arguments(original_arguments.to_owned(), &current_folder)?
        } else {
            forwarded_arguments(original_arguments.to_owned())?
        };
        let context = ssh_context(&current_target, &current_folder);
        let status = ssh_status(
            &current_target,
            binary,
            &arguments,
            Some(&context),
            true,
            switched,
        )?;
        let code = status.code().unwrap_or_else(|| i32::from(EXIT_FAILURE));
        if let Some(index) = crate::ssh::switch_index(code)
            && let Some(machine) = context.machines.get(index)
        {
            crate::state::record_recent_remote(&current_target, &current_folder);
            current_target.clone_from(&machine.target);
            current_folder.clone_from(&machine.folder);
            switched = true;
            continue;
        }
        if status.success() {
            crate::state::record_recent_remote(&current_target, &current_folder);
        }
        if switched {
            crate::terminal::restore_inherited_terminal();
        }
        return Ok(exit_code(status));
    }
}

fn run_once(
    target: &str,
    folder: &Path,
    binary: &str,
    arguments: &[OsString],
    context: Option<&crate::ssh::SshContext>,
) -> Result<u8> {
    let status = ssh_status(target, binary, arguments, context, false, false)?;
    if status.success() {
        crate::state::record_recent_remote(target, folder);
    }
    Ok(exit_code(status))
}

fn ssh_status(
    target: &str,
    binary: &str,
    arguments: &[OsString],
    context: Option<&crate::ssh::SshContext>,
    terminal: bool,
    inherited_terminal: bool,
) -> Result<std::process::ExitStatus> {
    let command = remote_command(binary, arguments, context, inherited_terminal)?;
    let mut ssh = Command::new("ssh");
    let ssh = if terminal && io::stdin().is_terminal() && io::stdout().is_terminal() {
        ssh.arg("-tt")
    } else {
        &mut ssh
    };
    let status = ssh
        .arg("--")
        .arg(target)
        .arg(command)
        .status()
        .with_context(|| format!("failed to start ssh for {target}"))?;
    Ok(status)
}

fn exit_code(status: std::process::ExitStatus) -> u8 {
    match status.code() {
        Some(255) => EXIT_UNAVAILABLE,
        Some(code) => u8::try_from(code).unwrap_or(EXIT_FAILURE),
        None => EXIT_FAILURE,
    }
}

fn ssh_context(target: &str, folder: &Path) -> crate::ssh::SshContext {
    let machines = crate::state::load_recent_ssh_machines_with_current(target, folder);
    context_with_reachability(target, machines, Some(target))
}

pub(crate) fn local_ssh_context() -> Option<crate::ssh::SshContext> {
    let machines = crate::state::load_recent_ssh_machines();
    (!machines.is_empty()).then(|| context_with_reachability("local", machines, None))
}

fn context_with_reachability(
    current: &str,
    mut machines: Vec<crate::ssh::SshMachine>,
    assumed_reachable: Option<&str>,
) -> crate::ssh::SshContext {
    let targets = machines
        .iter()
        .map(|machine| machine.target.clone())
        .collect::<Vec<_>>();
    let accessible = thread::scope(|scope| {
        #[expect(
            clippy::needless_collect,
            reason = "every reachability probe must start before any join can block"
        )]
        let checks = targets
            .iter()
            .map(|candidate| {
                let current = assumed_reachable.is_some_and(|target| candidate == target);
                scope.spawn(move || current || probe(candidate))
            })
            .collect::<Vec<_>>();
        checks
            .into_iter()
            .map(|check| check.join().is_ok_and(|reachable| reachable))
            .collect::<Vec<_>>()
    });
    for (machine, reachable) in machines.iter_mut().zip(accessible) {
        machine.accessible = reachable;
    }
    crate::ssh::SshContext {
        current: current.to_owned(),
        machines,
    }
}

pub(super) fn manage(out: &Emitter, command: RemoteVerb) -> Result<u8> {
    match command {
        RemoteVerb::List => list(out),
        RemoteVerb::Forget { target, folder } => {
            crate::state::forget_recent_remote(&target, folder.as_deref());
            out.emit(
                &RemoteForget {
                    target: &target,
                    folder: folder.as_deref(),
                },
                || {
                    folder.as_deref().map_or_else(
                        || format!("Forgot every recent folder on {target}\n"),
                        |folder| format!("Forgot {target}:{folder}\n"),
                    )
                },
            )?;
            Ok(0)
        }
    }
}

fn list(out: &Emitter) -> Result<u8> {
    let entries = crate::state::load_recent_remotes();
    let accessible = thread::scope(|scope| {
        #[expect(
            clippy::needless_collect,
            reason = "every reachability probe must start before any join can block"
        )]
        let checks = entries
            .iter()
            .map(|entry| scope.spawn(|| probe(&entry.target)))
            .collect::<Vec<_>>();
        checks
            .into_iter()
            .map(|check| check.join().is_ok_and(|accessible| accessible))
            .collect::<Vec<_>>()
    });
    let remotes = entries
        .into_iter()
        .zip(accessible)
        .map(|(entry, accessible)| RemoteStatus {
            accessible,
            target: entry.target,
            folder: entry.folder,
            uses: entry.uses,
        })
        .collect::<Vec<_>>();
    out.emit(&RemoteList { remotes: &remotes }, || {
        if remotes.is_empty() {
            return "No recent SSH repositories\n".to_owned();
        }
        let mut text = String::new();
        for remote in &remotes {
            text.push_str(if remote.accessible {
                "accessible   "
            } else {
                "unavailable  "
            });
            text.push_str(&remote.target);
            text.push(':');
            text.push_str(&remote.folder);
            text.push_str("   used ");
            text.push_str(&remote.uses.to_string());
            text.push('\n');
        }
        text
    })?;
    Ok(0)
}

fn probe(target: &str) -> bool {
    Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=3",
            "--",
            target,
            "true",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteStatus {
    target: String,
    folder: String,
    accessible: bool,
    uses: u64,
}

#[derive(Serialize)]
struct RemoteList<'a> {
    remotes: &'a [RemoteStatus],
}

#[derive(Serialize)]
struct RemoteForget<'a> {
    target: &'a str,
    folder: Option<&'a str>,
}

fn validate_target(target: &str) -> Result<()> {
    anyhow::ensure!(!target.is_empty(), "SSH target cannot be empty");
    anyhow::ensure!(
        !target.starts_with('-'),
        "SSH target cannot start with a hyphen"
    );
    anyhow::ensure!(
        !target.chars().any(char::is_whitespace),
        "SSH target cannot contain whitespace"
    );
    Ok(())
}

fn forwarded_arguments(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<OsString>> {
    let mut forwarded = Vec::new();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if argument == OsStr::new("--remote") {
            drop(
                arguments
                    .next()
                    .context("--remote requires an SSH target")?,
            );
        } else if argument
            .to_str()
            .is_some_and(|value| value.starts_with("--remote="))
        {
        } else if argument == OsStr::new("--folder") {
            forwarded.push(OsString::from("--path"));
            forwarded.push(arguments.next().context("--folder requires a directory")?);
        } else if let Some(folder) = argument
            .to_str()
            .and_then(|value| value.strip_prefix("--folder="))
        {
            forwarded.push(OsString::from("--path"));
            forwarded.push(OsString::from(folder));
        } else {
            forwarded.push(argument);
        }
    }
    Ok(forwarded)
}

fn implicit_terminal_arguments(
    arguments: impl IntoIterator<Item = OsString>,
    folder: &Path,
) -> Result<Vec<OsString>> {
    let forwarded = forwarded_arguments(arguments)?;
    let mut terminal = vec![OsString::from("tui"), folder.as_os_str().to_os_string()];
    let mut forwarded = forwarded.into_iter();
    while let Some(argument) = forwarded.next() {
        if argument == OsStr::new("--path") || argument == OsStr::new("-C") {
            drop(
                forwarded
                    .next()
                    .context("repository path requires a directory")?,
            );
        } else if argument
            .to_str()
            .is_some_and(|value| value.starts_with("--path=") || value.starts_with("-C"))
        {
        } else {
            terminal.push(argument);
        }
    }
    Ok(terminal)
}

fn switched_terminal_arguments(
    arguments: impl IntoIterator<Item = OsString>,
    folder: &Path,
) -> Result<Vec<OsString>> {
    let forwarded = forwarded_arguments(arguments)?;
    let Some(tui) = forwarded
        .iter()
        .position(|argument| argument == OsStr::new("tui"))
    else {
        return implicit_terminal_arguments(forwarded, folder);
    };
    let mut terminal = vec![OsString::from("tui"), folder.as_os_str().to_os_string()];
    let mut trailing = forwarded.into_iter().skip(tui.saturating_add(1));
    if let Some(argument) = trailing.next()
        && argument
            .to_str()
            .is_some_and(|value| value.starts_with('-'))
    {
        terminal.push(argument);
    }
    terminal.extend(trailing);
    Ok(terminal)
}

fn remote_command(
    binary: &str,
    arguments: &[OsString],
    context: Option<&crate::ssh::SshContext>,
    inherited_terminal: bool,
) -> Result<String> {
    let mut command = quote(binary)?;
    for argument in arguments {
        command.push(' ');
        command.push_str(&quote(&argument.to_string_lossy())?);
    }
    if let Some(context) = context {
        let serialized = serde_json::to_string(context)?;
        command = format!("QUINJET_SSH_CONTEXT={} {command}", quote(&serialized)?);
    }
    if inherited_terminal {
        command = format!("{}=1 {command}", crate::terminal::INHERITED_TERMINAL_ENV);
    }
    Ok(command)
}

fn quote(value: &str) -> Result<String> {
    shlex::try_quote(value)
        .map(std::borrow::Cow::into_owned)
        .context("remote arguments cannot contain a null byte")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_flags_are_removed_and_folder_becomes_path() {
        let arguments = [
            "--remote",
            "host",
            "--folder=/repos/a project",
            "status",
            "--json",
        ]
        .into_iter()
        .map(OsString::from);
        assert_eq!(
            forwarded_arguments(arguments).unwrap(),
            ["--path", "/repos/a project", "status", "--json"]
                .map(OsString::from)
                .to_vec()
        );
    }

    #[test]
    fn remote_command_quotes_every_argument() {
        let command = remote_command(
            "quinjet test",
            &[OsString::from("--path"), OsString::from("a'b c")],
            None,
            false,
        )
        .unwrap();
        assert_eq!(command, "'quinjet test' --path \"a'b c\"");
    }

    #[test]
    fn switched_terminal_inherits_the_existing_alternate_screen() {
        let command = remote_command("quinjet", &[], None, true).unwrap();
        assert_eq!(command, "QUINJET_INHERITED_TERMINAL=1 quinjet");
    }

    #[test]
    fn implicit_terminal_uses_the_tui_path_for_released_remote_binaries() {
        let arguments = ["--remote", "host", "--folder", "/repos/a project"]
            .into_iter()
            .map(OsString::from);
        assert_eq!(
            implicit_terminal_arguments(arguments, Path::new("/repos/a project")).unwrap(),
            ["tui", "/repos/a project"].map(OsString::from).to_vec()
        );
    }

    #[test]
    fn switched_terminal_replaces_the_repository_and_preserves_options() {
        let arguments = [
            "--remote",
            "host",
            "tui",
            "/old/repository",
            "--theme",
            "quinjet",
        ]
        .into_iter()
        .map(OsString::from);
        assert_eq!(
            switched_terminal_arguments(arguments, Path::new("/new/repository")).unwrap(),
            ["tui", "/new/repository", "--theme", "quinjet"]
                .map(OsString::from)
                .to_vec()
        );
    }

    #[test]
    fn unsafe_ssh_targets_are_refused() {
        assert!(validate_target("-oProxyCommand=bad").is_err());
        assert!(validate_target("two hosts").is_err());
    }
}
