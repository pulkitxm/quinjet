use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::{Context, Result, bail};
use clap::CommandFactory;
use clap_complete::{Shell, generate};

use super::{Cli, PROGRAM};

const COMPLETION_BEGIN: &str = "# >>> quinjet completions >>>";
const COMPLETION_END: &str = "# <<< quinjet completions <<<";
const LEGACY_SHORTCUT_BEGIN: &str = "# >>> quinjet shortcut >>>";
const LEGACY_SHORTCUT_END: &str = "# <<< quinjet shortcut <<<";

struct ProfileIntegration {
    path: PathBuf,
    completion: Option<String>,
}

struct Target {
    script: PathBuf,
    profile: Option<ProfileIntegration>,
}

struct CompletionDirs {
    home: PathBuf,
    config: PathBuf,
    data: PathBuf,
    zsh: PathBuf,
}

pub(super) fn script(shell: Shell) -> Result<String> {
    let mut command = Cli::command();
    let mut script = Vec::new();
    generate(shell, &mut command, PROGRAM, &mut script);
    String::from_utf8(script).context("the completion script was not valid UTF-8")
}

pub(super) fn install(shell: Shell) -> Result<Vec<PathBuf>> {
    install_with_mode(shell, false)
}

pub(super) fn maintain(shell: Shell) -> Result<Vec<PathBuf>> {
    install_with_mode(shell, true)
}

fn install_with_mode(shell: Shell, automatic: bool) -> Result<Vec<PathBuf>> {
    let state = shell_state(shell)?;
    let installed_before = state.exists();
    let marker = binary_marker();
    let script = script(shell)?;
    let contents = format!("{marker}{script}");
    let targets = targets(shell)?;
    let legacy_shortcut = legacy_shortcut_exists(&targets)?;
    let mut installed = Vec::new();
    for target in &targets {
        if !automatic || !installed_before || target.script.exists() {
            write_file(&target.script, contents.as_bytes())?;
            installed.push(target.script.clone());
        }
        if let Some(profile) = &target.profile
            && let Some(command) = &profile.completion
        {
            integrate_profile(
                &profile.path,
                COMPLETION_BEGIN,
                COMPLETION_END,
                command,
                !automatic || !installed_before,
            )?;
        }
    }
    if let Some(shortcut) = install_shortcut(automatic, installed_before, legacy_shortcut)? {
        installed.push(shortcut);
    }
    if shortcut_is_enabled()? {
        for target in &targets {
            if let Some(profile) = &target.profile {
                remove_profile_integration(
                    &profile.path,
                    LEGACY_SHORTCUT_BEGIN,
                    LEGACY_SHORTCUT_END,
                )?;
            }
        }
    }
    write_file(&state, b"installed\n")?;
    Ok(installed)
}

pub(super) fn detected_shell() -> Option<Shell> {
    let configured = env::var_os("SHELL").and_then(|shell| shell_from_path(&shell));
    if configured.is_some() {
        return configured;
    }
    if cfg!(windows) || env::var_os("PSModulePath").is_some() {
        return Some(Shell::PowerShell);
    }
    None
}

fn shell_from_path(shell: &OsStr) -> Option<Shell> {
    match Path::new(shell)
        .file_stem()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("bash") => Some(Shell::Bash),
        Some("elvish") => Some(Shell::Elvish),
        Some("fish") => Some(Shell::Fish),
        Some("pwsh" | "powershell") => Some(Shell::PowerShell),
        Some("zsh") => Some(Shell::Zsh),
        _ => None,
    }
}

pub(super) fn auto_install() {
    if development_binary() {
        return;
    }
    let active = detected_shell();
    for shell in shells_to_refresh(active) {
        if completion_is_current(shell).unwrap_or(false) {
            continue;
        }
        drop(maintain(shell));
    }
}

pub(super) fn refresh_replaced_executable() -> Result<()> {
    let Some(shell) = detected_shell() else {
        return Ok(());
    };
    // nosemgrep: rust.lang.security.current-exe.current-exe
    let executable = env::current_exe().context("failed to locate the updated executable")?;
    let output = ProcessCommand::new(&executable)
        .args([
            "completions",
            &shell.to_string(),
            "--install",
            "--automatic",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| {
            format!(
                "failed to refresh completions with {}",
                executable.display()
            )
        })?;
    if output.status.success() {
        return Ok(());
    }
    bail!(
        "the executable was updated, but its completions could not be refreshed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

const fn development_binary() -> bool {
    cfg!(debug_assertions)
}

fn shells_to_refresh(active: Option<Shell>) -> Vec<Shell> {
    let mut shells = Vec::new();
    for shell in [
        Shell::Bash,
        Shell::Elvish,
        Shell::Fish,
        Shell::PowerShell,
        Shell::Zsh,
    ] {
        if Some(shell) == active || has_installed_target(shell) {
            shells.push(shell);
        }
    }
    shells
}

fn has_installed_target(shell: Shell) -> bool {
    if shell == Shell::PowerShell {
        return recorded_powershell_profiles().is_ok_and(|profiles| {
            profiles
                .iter()
                .any(|profile| powershell_target(profile).script.exists())
        });
    }
    targets(shell).is_ok_and(|targets| targets.iter().any(|target| target.script.exists()))
}

fn completion_is_current(shell: Shell) -> Result<bool> {
    if !shell_state(shell)?.exists() || !shortcut_state()?.exists() {
        return Ok(false);
    }
    let marker = binary_marker();
    let targets = targets(shell)?;
    Ok(!targets.is_empty()
        && targets.iter().all(|target| {
            !target.script.exists() || first_line(&target.script).is_ok_and(|line| line == marker)
        }))
}

fn first_line(path: &Path) -> Result<String> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut line = String::new();
    BufReader::new(file)
        .read_line(&mut line)
        .map(|_| ())
        .with_context(|| format!("failed to read {}", path.display()))?;
    Ok(line)
}

fn targets(shell: Shell) -> Result<Vec<Target>> {
    if shell == Shell::PowerShell {
        return powershell_targets();
    }
    targets_in(shell, &completion_dirs()?)
}

fn targets_in(shell: Shell, directories: &CompletionDirs) -> Result<Vec<Target>> {
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

fn powershell_targets() -> Result<Vec<Target>> {
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

fn powershell_target(profile: &Path) -> Target {
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

fn recorded_powershell_profiles() -> Result<Vec<PathBuf>> {
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

fn powershell_record() -> Result<PathBuf> {
    Ok(state_root()?.join("powershell-profiles"))
}

fn shell_state(shell: Shell) -> Result<PathBuf> {
    Ok(state_root()?.join(format!("{shell}-installed")))
}

fn shortcut_state() -> Result<PathBuf> {
    Ok(state_root()?.join("shortcut-installed"))
}

fn shortcut_is_enabled() -> Result<bool> {
    let state = shortcut_state()?;
    match fs::read_to_string(&state) {
        Ok(contents) => Ok(contents != "removed\n"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", state.display())),
    }
}

fn state_root() -> Result<PathBuf> {
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

fn legacy_shortcut_exists(targets: &[Target]) -> Result<bool> {
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

fn install_shortcut(
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

fn shortcut_paths() -> Result<(PathBuf, Vec<PathBuf>)> {
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
fn create_shortcut(executable: &Path, shortcut: &Path) -> Result<()> {
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
fn create_shortcut(executable: &Path, shortcut: &Path) -> Result<()> {
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
fn create_shortcut(_executable: &Path, _shortcut: &Path) -> Result<()> {
    bail!("q shortcut installation is not supported on this platform")
}

fn remove_profile_integration(profile: &Path, begin: &str, end: &str) -> Result<()> {
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

fn integrate_profile(
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

fn profile_destination(profile: &Path) -> Result<PathBuf> {
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

fn write_file(path: &Path, contents: &[u8]) -> Result<()> {
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

fn binary_marker() -> String {
    format!("# quinjet-completion {}\n", env!("CARGO_PKG_VERSION"))
}

fn completion_dirs() -> Result<CompletionDirs> {
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

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn single_quote(value: &str) -> String {
    value.replace('\'', "'\\''")
}

#[cfg(test)]
mod tests {
    use anyhow::ensure;

    use super::*;

    #[test]
    fn user_completion_paths_follow_xdg_and_shell_conventions() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let directories = CompletionDirs {
            home: directory.path().join("home"),
            config: directory.path().join("config"),
            data: directory.path().join("data"),
            zsh: directory.path().join("zsh"),
        };

        ensure!(
            targets_in(Shell::Bash, &directories)?[0].script
                == directories.data.join("bash-completion/completions/quinjet")
        );
        ensure!(
            targets_in(Shell::Fish, &directories)?[0].script
                == directories.config.join("fish/completions/quinjet.fish")
        );
        ensure!(
            targets_in(Shell::Elvish, &directories)?[0].script
                == directories.config.join("elvish/lib/quinjet.elv")
        );
        ensure!(
            targets_in(Shell::Zsh, &directories)?[0].script
                == directories.zsh.join(".zfunc/_quinjet")
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn q_shortcut_resolves_to_the_executable() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let executable = directory.path().join("quinjet");
        let shortcut = directory.path().join("q");
        fs::write(&executable, "binary")?;
        create_shortcut(&executable, &shortcut)?;
        ensure!(fs::canonicalize(&shortcut)? == fs::canonicalize(&executable)?);
        create_shortcut(&executable, &shortcut)?;
        let fallback = directory.path().join("bin/q");
        create_shortcut(&executable, &fallback)?;
        ensure!(fs::canonicalize(&fallback)? == fs::canonicalize(&executable)?);
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn q_shortcut_batch_file_invokes_the_executable() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let executable = directory.path().join("app/quinjet.exe");
        let shortcut = directory.path().join("bin/q.cmd");
        fs::create_dir_all(executable.parent().context("executable had no parent")?)?;
        fs::write(&executable, "binary")?;
        create_shortcut(&executable, &shortcut)?;
        ensure!(fs::read_to_string(&shortcut)?.contains(&executable.display().to_string()));
        Ok(())
    }

    #[test]
    fn legacy_shortcut_block_is_removed() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let profile = directory.path().join("profile");
        fs::write(
            &profile,
            "existing\n# >>> quinjet shortcut >>>\nalias q='quinjet'\n# <<< quinjet shortcut <<<\nafter\n",
        )?;
        remove_profile_integration(&profile, LEGACY_SHORTCUT_BEGIN, LEGACY_SHORTCUT_END)?;
        ensure!(fs::read_to_string(&profile)? == "existing\nafter\n");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn profile_integration_preserves_a_dotfiles_symlink() -> Result<()> {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir()?;
        let target = directory.path().join("dotfiles/bashrc");
        let parent = target.parent().context("test target had no parent")?;
        fs::create_dir_all(parent)?;
        fs::write(&target, "existing\n")?;
        let profile = directory.path().join(".bashrc");
        symlink(&target, &profile)?;
        integrate_profile(
            &profile,
            LEGACY_SHORTCUT_BEGIN,
            LEGACY_SHORTCUT_END,
            "load quinjet",
            true,
        )?;
        ensure!(fs::symlink_metadata(&profile)?.file_type().is_symlink());
        ensure!(fs::read_to_string(&target)?.contains("load quinjet"));
        Ok(())
    }

    #[test]
    fn profile_integration_is_idempotent() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let profile = directory.path().join("profile");
        integrate_profile(
            &profile,
            LEGACY_SHORTCUT_BEGIN,
            LEGACY_SHORTCUT_END,
            "load quinjet",
            true,
        )?;
        integrate_profile(
            &profile,
            LEGACY_SHORTCUT_BEGIN,
            LEGACY_SHORTCUT_END,
            "load quinjet",
            true,
        )?;
        let contents = fs::read_to_string(&profile)?;
        ensure!(contents.matches(LEGACY_SHORTCUT_BEGIN).count() == 1);
        ensure!(contents.contains("load quinjet"));
        Ok(())
    }
}
