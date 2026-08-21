use super::*;

pub(super) fn legacy_shortcut_exists(targets: &[Target]) -> Result<bool> {
    for target in targets {
        let Some(profile) = &target.profile else {
            continue;
        };
        match fs::read_to_string(&profile.path) {
            Ok(contents) if contents.contains(LEGACY_SHORTCUT_BEGIN) => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read {}", profile.path.display()));
            }
        }
    }
    Ok(false)
}

pub(super) fn install_shortcut(
    automatic: bool,
    shell_installed_before: bool,
    legacy_shortcut: bool,
) -> Result<Option<PathBuf>> {
    let state = shortcut_state()?;
    if automatic && state.exists() {
        return Ok(None);
    }
    if automatic && shell_installed_before && !legacy_shortcut {
        write_file(&state, b"removed\n")?;
        return Ok(None);
    }
    let (executable, shortcuts) = shortcut_paths()?;
    let mut failure = None;
    for shortcut in shortcuts {
        match create_shortcut(&executable, &shortcut) {
            Ok(()) => {
                let mut record = shortcut.display().to_string();
                record.push('\n');
                write_file(&state, record.as_bytes())?;
                return Ok(Some(shortcut));
            }
            Err(error) => failure = Some(error),
        }
    }
    let error =
        failure.unwrap_or_else(|| anyhow::anyhow!("no directory on PATH can hold the q shortcut"));
    if automatic {
        write_file(&state, b"removed\n")?;
        return Ok(None);
    }
    Err(error)
}

pub(super) fn shortcut_paths() -> Result<(PathBuf, Vec<PathBuf>)> {
    // nosemgrep: rust.lang.security.current-exe.current-exe
    let executable = env::current_exe().context("failed to locate the Quinjet executable")?;
    let parent = executable
        .parent()
        .context("the Quinjet executable has no parent directory")?;
    let name = if cfg!(windows) { "q.cmd" } else { "q" };
    let path_dirs: Vec<PathBuf> = env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect())
        .unwrap_or_default();
    let home = env_path("HOME").or_else(|| env_path("USERPROFILE"));
    let local_bin = home.as_ref().map(|home| home.join(".local/bin"));
    let cargo_bin = env_path("CARGO_HOME")
        .map(|cargo| cargo.join("bin"))
        .or_else(|| home.as_ref().map(|home| home.join(".cargo/bin")));
    let xdg_bin = env_path("XDG_BIN_HOME");
    let mut shortcuts = Vec::new();
    let mut blocked = false;
    for directory in path_dirs {
        let shortcut = directory.join(name);
        if fs::symlink_metadata(&shortcut).is_ok() {
            if !shortcuts.contains(&shortcut) {
                shortcuts.push(shortcut);
            }
            blocked = true;
            break;
        }
        let user_bin = [&local_bin, &cargo_bin, &xdg_bin]
            .into_iter()
            .flatten()
            .any(|candidate| candidate == &directory);
        if (directory == parent || user_bin) && !shortcuts.contains(&shortcut) {
            shortcuts.push(shortcut);
        }
    }
    let adjacent = parent.join(name);
    if !blocked && !shortcuts.contains(&adjacent) {
        shortcuts.push(adjacent);
    }
    Ok((executable, shortcuts))
}

#[cfg(unix)]
pub(super) fn create_shortcut(executable: &Path, shortcut: &Path) -> Result<()> {
    use std::os::unix::fs::symlink;

    let parent = shortcut
        .parent()
        .context("the q shortcut has no parent directory")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    match fs::symlink_metadata(shortcut) {
        Ok(_) => {
            let resolves_to_executable = fs::canonicalize(shortcut)
                .and_then(|resolved| {
                    fs::canonicalize(executable).map(|executable| resolved == executable)
                })
                .unwrap_or(false);
            if resolves_to_executable {
                return Ok(());
            }
            bail!(
                "refusing to replace the existing q command at {}",
                shortcut.display()
            )
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", shortcut.display()));
        }
    }
    let name = executable
        .file_name()
        .context("the Quinjet executable has no file name")?;
    let target = if shortcut.parent() == executable.parent() {
        Path::new(name)
    } else {
        executable
    };
    symlink(target, shortcut)
        .with_context(|| format!("failed to install the q shortcut at {}", shortcut.display()))
}

#[cfg(windows)]
pub(super) fn create_shortcut(executable: &Path, shortcut: &Path) -> Result<()> {
    let command = if shortcut.parent() == executable.parent() {
        let name = executable
            .file_name()
            .and_then(OsStr::to_str)
            .context("the Quinjet executable name is not valid UTF-8")?;
        format!("%~dp0{name}")
    } else {
        executable
            .to_str()
            .context("the Quinjet executable path is not valid UTF-8")?
            .replace('%', "%%")
    };
    let contents = format!("@\"{command}\" %*\r\n");
    match fs::read_to_string(shortcut) {
        Ok(existing) if existing == contents => return Ok(()),
        Ok(_) => bail!(
            "refusing to replace the existing q command at {}",
            shortcut.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", shortcut.display()));
        }
    }
    write_file(shortcut, contents.as_bytes())
}

#[cfg(not(any(unix, windows)))]
pub(super) fn create_shortcut(_executable: &Path, _shortcut: &Path) -> Result<()> {
    bail!("q shortcut installation is not supported on this platform")
}
