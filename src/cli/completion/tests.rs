
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
        targets_in(Shell::Zsh, &directories)?[0].script == directories.zsh.join(".zfunc/_quinjet")
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

#[cfg(unix)]
#[test]
fn replaced_executable_refresh_uses_its_pre_replacement_path() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir()?;
    let executable = directory.path().join("quinjet");
    let replaced = directory.path().join("quinjet-replaced");
    let invocation = directory.path().join("invocation");
    fs::write(&executable, "#!/bin/sh\nexit 99\n")?;
    let captured = executable.clone();
    fs::rename(&executable, &replaced)?;
    let escaped = single_quote(&invocation.to_string_lossy());
    fs::write(
        &executable,
        format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >'{escaped}'\n"),
    )?;
    let mut permissions = fs::metadata(&executable)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions)?;
    refresh_with(&captured, Shell::Bash)?;
    ensure!(fs::read_to_string(invocation)? == "completions bash --install --automatic\n");
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
