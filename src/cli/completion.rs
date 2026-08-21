use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::{Context, Result, bail};
use clap::CommandFactory;
use clap_complete::{Shell, generate};

use super::{Cli, PROGRAM, package_manager};

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
    if development_binary() || package_manager::manages_running_executable() {
        return;
    }
    let active = detected_shell();
    if active.is_none() {
        drop(install_shortcut(true, shell_integration_exists(), false));
    }
    for shell in shells_to_refresh(active) {
        if completion_is_current(shell).unwrap_or(false) {
            continue;
        }
        drop(maintain(shell));
    }
}

pub(super) fn refresh_replaced_executable(executable: &Path) -> Result<()> {
    let Some(shell) = detected_shell() else {
        return Ok(());
    };
    refresh_with(executable, shell)
}

fn refresh_with(executable: &Path, shell: Shell) -> Result<()> {
    let output = ProcessCommand::new(executable)
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

fn shell_integration_exists() -> bool {
    [
        Shell::Bash,
        Shell::Elvish,
        Shell::Fish,
        Shell::PowerShell,
        Shell::Zsh,
    ]
    .into_iter()
    .any(|shell| shell_state(shell).is_ok_and(|state| state.exists()))
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

mod profile;
mod shortcut;
mod targets;

#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use profile::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use shortcut::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use targets::*;

#[cfg(test)]
mod tests;
