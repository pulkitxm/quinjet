use std::ffi::{OsStr, OsString};
use std::path::Path;

use anyhow::{Context, Result};

pub(super) fn forwarded(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<OsString>> {
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

pub(super) fn implicit(
    arguments: impl IntoIterator<Item = OsString>,
    folder: &Path,
) -> Result<Vec<OsString>> {
    let forwarded = forwarded(arguments)?;
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

pub(super) fn switched(
    arguments: impl IntoIterator<Item = OsString>,
    folder: &Path,
) -> Result<Vec<OsString>> {
    let forwarded = forwarded(arguments)?;
    let Some(tui) = forwarded
        .iter()
        .position(|argument| argument == OsStr::new("tui"))
    else {
        return implicit(forwarded, folder);
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
            forwarded(arguments).unwrap(),
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
            forwarded(arguments).unwrap(),
            ["--client", "edith", "tui", "/repo"]
                .map(OsString::from)
                .to_vec()
        );
    }

    #[test]
    fn implicit_terminal_uses_the_tui_path_for_released_remote_binaries() {
        let arguments = ["--remote", "host", "--folder", "/repos/a project"]
            .into_iter()
            .map(OsString::from);
        assert_eq!(
            implicit(arguments, Path::new("/repos/a project")).unwrap(),
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
            switched(arguments, Path::new("/new/repository")).unwrap(),
            ["tui", "/new/repository", "--theme", "quinjet"]
                .map(OsString::from)
                .to_vec()
        );
    }
}
