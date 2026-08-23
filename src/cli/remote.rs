use std::ffi::{OsStr, OsString};
use std::io::{self, IsTerminal};
use std::path::Path;
use std::process::{Command, Stdio};
use std::{env, thread};

use anyhow::{Context, Result};
use serde::Serialize;

use super::{EXIT_FAILURE, EXIT_UNAVAILABLE, Emitter, RemoteVerb};
use crate::ssh::{SshContext, SshProjectOpenMode};

const REMOTE_BINARY_ENV: &str = "QUINJET_REMOTE_BINARY";
mod terminal;

pub(crate) use terminal::run_selected_terminal;

struct TerminalStatus {
    status: std::process::ExitStatus,
    context: Option<SshContext>,
}

#[derive(Clone, Copy, Default)]
struct TerminalRelay {
    allocate: bool,
    inherited: bool,
    project_mode: Option<SshProjectOpenMode>,
}

pub(super) fn run(
    target: &str,
    terminal: bool,
    implicit_terminal: bool,
    folder: &Path,
    control_path: Option<&Path>,
) -> Result<u8> {
    validate_target(target)?;
    let binary = env::var(REMOTE_BINARY_ENV).unwrap_or_else(|_| "quinjet".to_owned());
    let original_arguments = wild::args_os().skip(1).collect::<Vec<_>>();
    if !terminal {
        let arguments = forwarded_arguments(original_arguments)?;
        return run_once(target, folder, &binary, &arguments, None, control_path);
    }
    terminal::run_terminal_loop(
        target,
        folder,
        &binary,
        &original_arguments,
        (
            implicit_terminal,
            false,
            None,
            control_path.map(Path::to_path_buf),
        ),
        None,
    )
}

fn run_once(
    target: &str,
    folder: &Path,
    binary: &str,
    arguments: &[OsString],
    context: Option<&SshContext>,
    control_path: Option<&Path>,
) -> Result<u8> {
    let outcome = ssh_status(
        target,
        binary,
        arguments,
        context,
        TerminalRelay::default(),
        control_path,
    )?;
    if outcome.status.success() {
        crate::state::record_recent_remote(target, folder);
    }
    Ok(exit_code(outcome.status))
}

fn ssh_status(
    target: &str,
    binary: &str,
    arguments: &[OsString],
    context: Option<&SshContext>,
    terminal: TerminalRelay,
    control_path: Option<&Path>,
) -> Result<TerminalStatus> {
    let command = terminal::remote_command(
        binary,
        arguments,
        context,
        terminal.inherited,
        terminal.project_mode,
    )?;
    let mut ssh = Command::new("ssh");
    let ssh = match control_path {
        Some(path) => ssh.arg("-S").arg(path),
        None => &mut ssh,
    };
    let ssh = if terminal.allocate && io::stdin().is_terminal() && io::stdout().is_terminal() {
        ssh.arg("-tt")
    } else {
        ssh
    };
    let _command = ssh.arg("--").arg(target).arg(command);
    if terminal.allocate {
        return terminal::relay_status(ssh, &format!("failed to start ssh for {target}"));
    }
    let status = ssh
        .status()
        .with_context(|| format!("failed to start ssh for {target}"))?;
    Ok(TerminalStatus {
        status,
        context: None,
    })
}

fn exit_code(status: std::process::ExitStatus) -> u8 {
    match status.code() {
        Some(255) => EXIT_UNAVAILABLE,
        Some(code) => u8::try_from(code).unwrap_or(EXIT_FAILURE),
        None => EXIT_FAILURE,
    }
}

fn ssh_context(target: &str, folder: &Path) -> SshContext {
    let machines = with_host_machine(
        crate::state::load_recent_ssh_machines_with_current(target, folder),
        &env::current_dir().unwrap_or_default(),
    );
    context_with_reachability(target, machines, Some(target))
}

pub(crate) fn local_ssh_context(folder: &Path) -> Option<SshContext> {
    let machines = crate::state::load_recent_ssh_machines();
    (!machines.is_empty()).then(|| {
        let machines = with_host_machine(machines, folder);
        let current = machines
            .first()
            .map_or_else(|| "local".to_owned(), |machine| machine.target.clone());
        context_with_reachability(&current, machines, None)
    })
}

fn with_host_machine(
    mut machines: Vec<crate::ssh::SshMachine>,
    folder: &Path,
) -> Vec<crate::ssh::SshMachine> {
    machines.truncate(crate::ssh::MAX_SSH_MACHINES.saturating_sub(1));
    machines.insert(
        0,
        crate::ssh::SshMachine {
            target: host_name(),
            folder: folder.to_path_buf(),
            accessible: true,
            uses: 0,
            local: true,
        },
    );
    machines
}

fn host_name() -> String {
    Command::new("hostname")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "local".to_owned())
}

fn context_with_reachability(
    current: &str,
    mut machines: Vec<crate::ssh::SshMachine>,
    assumed_reachable: Option<&str>,
) -> SshContext {
    let targets = machines
        .iter()
        .map(|machine| (machine.target.clone(), machine.local))
        .collect::<Vec<_>>();
    let accessible = thread::scope(|scope| {
        #[expect(
            clippy::needless_collect,
            reason = "every reachability probe must start before any join can block"
        )]
        let checks = targets
            .iter()
            .map(|(candidate, local)| {
                let current = assumed_reachable.is_some_and(|target| candidate == target);
                scope.spawn(move || *local || current || probe(candidate))
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
    SshContext {
        current: current.to_owned(),
        machines,
        tabs: crate::ssh::SshTabs::default(),
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
        } else if argument == OsStr::new("--ssh-control-path") {
            drop(
                arguments
                    .next()
                    .context("--ssh-control-path requires a path")?,
            );
        } else if argument
            .to_str()
            .is_some_and(|value| value.starts_with("--ssh-control-path="))
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
            "--ssh-control-path",
            "/tmp/edith.sock",
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
    fn edith_client_is_forwarded_through_the_remote_transport() {
        let arguments = ["--client", "edith", "--remote", "host", "tui", "/repo"]
            .into_iter()
            .map(OsString::from);
        assert_eq!(
            forwarded_arguments(arguments).unwrap(),
            ["--client", "edith", "tui", "/repo"]
                .map(OsString::from)
                .to_vec()
        );
    }

    #[test]
    fn host_machine_is_named_and_pinned_before_remotes() {
        let machines = with_host_machine(
            vec![crate::ssh::SshMachine {
                target: "remote".to_owned(),
                folder: "/remote".into(),
                accessible: true,
                uses: 9,
                local: false,
            }],
            Path::new("/host"),
        );
        assert_eq!(machines.len(), 2);
        assert!(machines[0].local);
        assert_ne!(machines[0].target, "");
        assert_eq!(machines[0].folder, Path::new("/host"));
        assert_eq!(machines[1].target, "remote");
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
