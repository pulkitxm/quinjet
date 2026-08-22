use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{self, IsTerminal};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

use anyhow::{Context, Result};

use serde::Serialize;

use super::{EXIT_FAILURE, EXIT_UNAVAILABLE, Emitter, RemoteVerb};

const REMOTE_BINARY_ENV: &str = "QUINJET_REMOTE_BINARY";

pub(super) fn run(target: &str, terminal: bool, folder: &Path) -> Result<u8> {
    validate_target(target)?;
    let binary = env::var(REMOTE_BINARY_ENV).unwrap_or_else(|_| "quinjet".to_owned());
    let arguments = forwarded_arguments(env::args_os().skip(1))?;
    let command = remote_command(&binary, &arguments)?;
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
    if status.success() {
        crate::state::record_recent_remote(target, folder);
    }
    Ok(match status.code() {
        Some(255) => EXIT_UNAVAILABLE,
        Some(code) => u8::try_from(code).unwrap_or(EXIT_FAILURE),
        None => EXIT_FAILURE,
    })
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

fn remote_command(binary: &str, arguments: &[OsString]) -> Result<String> {
    let mut command = quote(binary)?;
    for argument in arguments {
        command.push(' ');
        command.push_str(&quote(&argument.to_string_lossy())?);
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
        )
        .unwrap();
        assert_eq!(command, "'quinjet test' --path \"a'b c\"");
    }

    #[test]
    fn unsafe_ssh_targets_are_refused() {
        assert!(validate_target("-oProxyCommand=bad").is_err());
        assert!(validate_target("two hosts").is_err());
    }
}
