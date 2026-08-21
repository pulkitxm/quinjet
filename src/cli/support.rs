use super::*;

pub(super) fn operate(session: &mut Session, out: &Emitter, operation: GitOperation) -> Result<u8> {
    let (_, _, message) = session.execute(Command::Operate(operation))?.operation()?;
    out.message(&message)?;
    Ok(0)
}

pub(super) fn revision_operation(
    session: &mut Session,
    out: &Emitter,
    args: &RevisionArgs,
    action: &str,
    operation: impl FnOnce(String) -> GitOperation,
) -> Result<u8> {
    let revision = revision(session, &args.revision)?;
    if !args.yes {
        out.message(&format!(
            "Would {action} `{revision}`. Pass --yes to {action} it."
        ))?;
        return Ok(0);
    }
    operate(session, out, operation(revision))
}

pub(super) fn revision(session: &Session, value: &str) -> Result<String> {
    session.repository_revision(value).map_err(|error| {
        Failure::new(EXIT_NOT_FOUND, format!("{error:#}"))
            .hint("run `quinjet log` or `quinjet branch list --all` for what this repository holds")
            .into()
    })
}

pub(super) fn require_paths(paths: Vec<PathBuf>, verb: &str) -> Result<Vec<PathBuf>> {
    if paths.is_empty() {
        return Err(Failure::new(
            EXIT_FAILURE,
            format!("{verb} needs paths, or --all for every change"),
        )
        .into());
    }
    Ok(paths)
}

pub(super) fn matches(path: &Path, filters: &[PathBuf]) -> bool {
    filters.is_empty() || filters.iter().any(|filter| path.starts_with(filter))
}

pub(super) const fn interval(seconds: u64, floor: u64) -> Duration {
    Duration::from_secs(if seconds < floor { floor } else { seconds })
}

pub(crate) fn open_url(url: &str) -> Result<()> {
    if std::env::var_os("CMUX_SOCKET_PATH").is_some()
        && let Ok(child) = std::process::Command::new("cmux")
            .args(["browser", "open"])
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    {
        drop(child);
        return Ok(());
    }
    open_target(OsStr::new(url), url)
}

#[expect(
    dead_code,
    reason = "filesystem counterpart to open_url, kept for path links"
)]
pub(crate) fn open_path(path: &Path) -> Result<()> {
    open_target(path.as_os_str(), &path.display().to_string())
}

pub(super) fn open_target(target: &OsStr, display: &str) -> Result<()> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    drop(
        std::process::Command::new(opener)
            .arg(target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("failed to hand {display} to {opener}"))?,
    );
    Ok(())
}

pub(super) fn note(text: &str) {
    drop(writeln!(io::stderr().lock(), "{text}"));
}

pub(crate) fn report(error: &anyhow::Error) -> u8 {
    if let Some(broken) = error.downcast_ref::<io::Error>()
        && broken.kind() == io::ErrorKind::BrokenPipe
    {
        return 0;
    }
    let failure = error.downcast_ref::<Failure>();
    note(&format!("error: {error:#}"));
    if let Some(hint) = failure.and_then(|failure| failure.hint.as_deref()) {
        note(&format!("hint: {hint}"));
    }
    failure.map_or(EXIT_FAILURE, |failure| failure.code)
}

pub(crate) fn stdout_is_terminal() -> bool {
    io::stdout().is_terminal()
}
