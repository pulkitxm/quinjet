use super::*;

pub(super) fn targets(shell: Shell) -> Result<Vec<Target>> {
    if shell == Shell::PowerShell {
        return powershell_targets();
    }
    targets_in(shell, &completion_dirs()?)
}

pub(super) fn targets_in(shell: Shell, directories: &CompletionDirs) -> Result<Vec<Target>> {
    match shell {
        Shell::Bash => Ok(vec![Target {
            script: directories.data.join("bash-completion/completions/quinjet"),
            profile: Some(ProfileIntegration {
                path: directories.home.join(".bashrc"),
                completion: None,
            }),
        }]),
        Shell::Elvish => {
            let root = directories.config.join("elvish");
            Ok(vec![Target {
                script: root.join("lib/quinjet.elv"),
                profile: Some(ProfileIntegration {
                    path: root.join("rc.elv"),
                    completion: Some("use quinjet".to_owned()),
                }),
            }])
        }
        Shell::Fish => Ok(vec![Target {
            script: directories.config.join("fish/completions/quinjet.fish"),
            profile: Some(ProfileIntegration {
                path: directories.config.join("fish/config.fish"),
                completion: None,
            }),
        }]),
        Shell::Zsh => {
            let functions = directories.zsh.join(".zfunc");
            let escaped = single_quote(&functions.to_string_lossy());
            Ok(vec![Target {
                script: functions.join("_quinjet"),
                profile: Some(ProfileIntegration {
                    path: directories.zsh.join(".zshrc"),
                    completion: Some(format!(
                        "fpath=('{escaped}' $fpath)\nautoload -Uz compinit\ncompinit"
                    )),
                }),
            }])
        }
        Shell::PowerShell => bail!("PowerShell profiles are resolved by PowerShell"),
        _ => bail!("this shell does not support generated completions"),
    }
}

pub(super) fn powershell_targets() -> Result<Vec<Target>> {
    let recorded = recorded_powershell_profiles()?;
    if !recorded.is_empty() {
        return Ok(recorded
            .iter()
            .map(|profile| powershell_target(profile))
            .collect());
    }
    let mut profiles = Vec::new();
    for program in ["pwsh", "powershell"] {
        let output = ProcessCommand::new(program)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "[Console]::Out.Write($PROFILE.CurrentUserAllHosts)",
            ])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output();
        let Ok(output) = output else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let profile_text = String::from_utf8_lossy(&output.stdout);
        let profile = PathBuf::from(profile_text.trim());
        if profile.as_os_str().is_empty() || profiles.contains(&profile) {
            continue;
        }
        profiles.push(profile);
    }
    if profiles.is_empty() {
        bail!("could not locate a PowerShell profile")
    }
    let mut record = String::new();
    for profile in &profiles {
        record.push_str(&profile.to_string_lossy());
        record.push('\n');
    }
    write_file(&powershell_record()?, record.as_bytes())?;
    Ok(profiles
        .iter()
        .map(|profile| powershell_target(profile))
        .collect())
}

pub(super) fn powershell_target(profile: &Path) -> Target {
    let parent = profile.parent().unwrap_or(profile);
    let script = parent.join("quinjet-completions.ps1");
    let escaped = script.to_string_lossy().replace('\'', "''");
    Target {
        script,
        profile: Some(ProfileIntegration {
            path: profile.to_path_buf(),
            completion: Some(format!(". '{escaped}'")),
        }),
    }
}

pub(super) fn recorded_powershell_profiles() -> Result<Vec<PathBuf>> {
    let record = powershell_record()?;
    let contents = match fs::read_to_string(&record) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", record.display()));
        }
    };
    Ok(contents
        .lines()
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect())
}

pub(super) fn powershell_record() -> Result<PathBuf> {
    Ok(state_root()?.join("powershell-profiles"))
}

pub(super) fn shell_state(shell: Shell) -> Result<PathBuf> {
    Ok(state_root()?.join(format!("{shell}-installed")))
}

pub(super) fn shortcut_state() -> Result<PathBuf> {
    Ok(state_root()?.join("shortcut-installed"))
}

pub(super) fn shortcut_is_enabled() -> Result<bool> {
    let state = shortcut_state()?;
    match fs::read_to_string(&state) {
        Ok(contents) => Ok(contents != "removed\n"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", state.display())),
    }
}

pub(super) fn state_root() -> Result<PathBuf> {
    #[cfg(windows)]
    if let Some(local) = env_path("LOCALAPPDATA") {
        return Ok(local.join("Quinjet/state"));
    }
    let home = env_path("HOME")
        .or_else(|| env_path("USERPROFILE"))
        .context("HOME is not set, so the completion state cannot be determined")?;
    Ok(env_path("XDG_STATE_HOME")
        .unwrap_or_else(|| home.join(".local/state"))
        .join("quinjet"))
}
