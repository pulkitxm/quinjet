use super::*;

pub(super) fn remove_profile_integration(profile: &Path, begin: &str, end: &str) -> Result<()> {
    let contents = match fs::read_to_string(profile) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", profile.display()));
        }
    };
    let Some((before, block)) = contents.split_once(begin) else {
        return Ok(());
    };
    let Some((_, after)) = block.split_once(end) else {
        return Ok(());
    };
    let after = after.strip_prefix('\n').unwrap_or(after);
    let mut updated = String::with_capacity(before.len() + after.len());
    updated.push_str(before);
    updated.push_str(after);
    let destination = profile_destination(profile)?;
    write_file(&destination, updated.as_bytes())
}

pub(super) fn integrate_profile(
    profile: &Path,
    begin: &str,
    end: &str,
    command: &str,
    add_missing: bool,
) -> Result<()> {
    let mut contents = match fs::read_to_string(profile) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", profile.display()));
        }
    };
    if contents.contains(begin) || !add_missing {
        return Ok(());
    }
    if !contents.is_empty() && !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents.push_str(begin);
    contents.push('\n');
    contents.push_str(command);
    contents.push('\n');
    contents.push_str(end);
    contents.push('\n');
    let destination = profile_destination(profile)?;
    write_file(&destination, contents.as_bytes())
}

pub(super) fn profile_destination(profile: &Path) -> Result<PathBuf> {
    match fs::canonicalize(profile) {
        Ok(destination) => Ok(destination),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if fs::symlink_metadata(profile).is_ok() {
                bail!(
                    "the shell profile symlink {} has no target",
                    profile.display()
                )
            }
            Ok(profile.to_path_buf())
        }
        Err(error) => {
            Err(error).with_context(|| format!("failed to resolve {}", profile.display()))
        }
    }
}

pub(super) fn write_file(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let permissions = fs::metadata(path)
        .ok()
        .map(|metadata| metadata.permissions());
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to stage {}", path.display()))?;
    temporary
        .write_all(contents)
        .with_context(|| format!("failed to write {}", path.display()))?;
    temporary
        .flush()
        .with_context(|| format!("failed to flush {}", path.display()))?;
    if let Some(permissions) = permissions {
        fs::set_permissions(temporary.path(), permissions)
            .with_context(|| format!("failed to preserve permissions for {}", path.display()))?;
    }
    drop(
        temporary
            .persist(path)
            .map_err(|error| error.error)
            .with_context(|| format!("failed to install {}", path.display()))?,
    );
    Ok(())
}

pub(super) fn binary_marker() -> String {
    format!("# quinjet-completion {}\n", env!("CARGO_PKG_VERSION"))
}

pub(super) fn completion_dirs() -> Result<CompletionDirs> {
    let home = env_path("HOME")
        .or_else(|| env_path("USERPROFILE"))
        .context("HOME is not set, so the completion directory cannot be determined")?;
    let config = env_path("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"));
    let data = env_path("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"));
    let zsh = env_path("ZDOTDIR").unwrap_or_else(|| home.clone());
    Ok(CompletionDirs {
        home,
        config,
        data,
        zsh,
    })
}

pub(super) fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub(super) fn single_quote(value: &str) -> String {
    value.replace('\'', "'\\''")
}
