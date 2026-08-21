
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::PathBuf;

use anyhow::{Result, ensure};

use super::*;

#[test]
fn assets_match_the_release_matrix() -> Result<()> {
    for (os, arch, translated, expected) in [
        ("linux", "x86_64", false, "quinjet-linux-x86_64"),
        ("linux", "aarch64", false, "quinjet-linux-aarch64"),
        ("macos", "x86_64", false, "quinjet-macos-x86_64"),
        ("macos", "x86_64", true, "quinjet-macos-aarch64"),
        ("macos", "aarch64", false, "quinjet-macos-aarch64"),
        ("windows", "x86_64", false, "quinjet-windows-x86_64.exe"),
        ("windows", "aarch64", false, "quinjet-windows-x86_64.exe"),
    ] {
        ensure!(asset_for(os, arch, translated)? == expected);
    }
    ensure!(asset_for("linux", "riscv64", false).is_err());
    Ok(())
}

#[test]
fn release_tags_must_be_stable_semantic_versions() -> Result<()> {
    ensure!(parse_release(br#"{"tag_name":"v1.2.3"}"#)?.version == Version::new(1, 2, 3));
    ensure!(parse_release(br#"{"tag_name":"1.2.3"}"#).is_err());
    ensure!(parse_release(br#"{"tag_name":"v1.2.3-beta.1"}"#).is_err());
    ensure!(parse_release(br#"{"tag_name":"v1/2/3"}"#).is_err());
    Ok(())
}

#[test]
fn checksum_selection_is_exact_and_unique() -> Result<()> {
    let digest = "a".repeat(64);
    let document = format!(
        "{}  quinjet-linux-aarch64\n{} *dist/quinjet-linux-x86_64\n",
        "b".repeat(64),
        digest
    );
    ensure!(release_checksum(&document, "quinjet-linux-x86_64")? == digest);
    ensure!(release_checksum(&document, "quinjet-macos-aarch64").is_err());
    let duplicate = format!("{document}{}  quinjet-linux-x86_64\n", "c".repeat(64));
    ensure!(release_checksum(&duplicate, "quinjet-linux-x86_64").is_err());
    ensure!(release_checksum("nope  quinjet-linux-x86_64\n", "quinjet-linux-x86_64").is_err());
    Ok(())
}

#[test]
fn equal_or_older_release_does_not_download_or_replace() -> Result<()> {
    for tag in ["v1.2.3", "v1.2.2"] {
        let context = context("1.2.3");
        let requests = RefCell::new(Vec::new());
        let replaced = Cell::new(false);
        let result = perform_update(
            &context,
            false,
            |url, _limit| {
                requests.borrow_mut().push(url.to_owned());
                Ok(format!(r#"{{"tag_name":"{tag}"}}"#).into_bytes())
            },
            |_staged| {
                replaced.set(true);
                Ok(())
            },
        )?;
        ensure!(result.status == UpdateStatus::UpToDate);
        ensure!(requests.borrow().as_slice() == [context.api_url]);
        ensure!(!replaced.get());
    }
    Ok(())
}

#[test]
fn check_reports_available_release_without_downloading_it() -> Result<()> {
    let context = context("1.2.3");
    let requests = RefCell::new(Vec::new());
    let result = perform_update(
        &context,
        true,
        |url, _limit| {
            requests.borrow_mut().push(url.to_owned());
            Ok(br#"{"tag_name":"v1.3.0"}"#.to_vec())
        },
        |_staged| bail!("check-only mode tried to replace the executable"),
    )?;
    ensure!(result.status == UpdateStatus::Available);
    ensure!(requests.borrow().as_slice() == [context.api_url]);
    Ok(())
}

#[test]
fn update_pins_downloads_and_verifies_the_staged_bytes() -> Result<()> {
    let context = context("1.2.3");
    let binary = b"new release binary";
    let checksum = sha256(binary);
    let requests = RefCell::new(Vec::new());
    let replaced = Cell::new(false);
    let result = perform_update(
        &context,
        false,
        |url, _limit| {
            requests.borrow_mut().push(url.to_owned());
            if url == context.api_url {
                Ok(br#"{"tag_name":"v1.3.0"}"#.to_vec())
            } else if url.ends_with("/SHA256SUMS") {
                Ok(format!("{checksum}  quinjet-linux-x86_64\n").into_bytes())
            } else {
                Ok(binary.to_vec())
            }
        },
        |staged| {
            ensure!(fs::read(staged)? == binary);
            replaced.set(true);
            Ok(())
        },
    )?;
    ensure!(result.status == UpdateStatus::Updated);
    ensure!(replaced.get());
    ensure!(
        requests.borrow().as_slice()
            == [
                context.api_url.to_owned(),
                "https://example.invalid/releases/download/v1.3.0/SHA256SUMS".to_owned(),
                "https://example.invalid/releases/download/v1.3.0/quinjet-linux-x86_64".to_owned(),
            ]
    );
    Ok(())
}

#[test]
fn checksum_failure_never_invokes_the_replacer() -> Result<()> {
    let context = context("1.2.3");
    let replaced = Cell::new(false);
    let result = perform_update(
        &context,
        false,
        |url, _limit| {
            if url == context.api_url {
                Ok(br#"{"tag_name":"v1.3.0"}"#.to_vec())
            } else if url.ends_with("/SHA256SUMS") {
                Ok(format!("{}  quinjet-linux-x86_64\n", "a".repeat(64)).into_bytes())
            } else {
                Ok(b"wrong bytes".to_vec())
            }
        },
        |_staged| {
            replaced.set(true);
            Ok(())
        },
    );
    ensure!(result.is_err());
    ensure!(!replaced.get());
    Ok(())
}

#[test]
fn replacement_failure_removes_the_stage() -> Result<()> {
    let context = context("1.2.3");
    let binary = b"new release binary";
    let checksum = sha256(binary);
    let staged_path = RefCell::new(None::<PathBuf>);
    let result = perform_update(
        &context,
        false,
        |url, _limit| {
            if url == context.api_url {
                Ok(br#"{"tag_name":"v1.3.0"}"#.to_vec())
            } else if url.ends_with("/SHA256SUMS") {
                Ok(format!("{checksum}  quinjet-linux-x86_64\n").into_bytes())
            } else {
                Ok(binary.to_vec())
            }
        },
        |staged| {
            drop(staged_path.replace(Some(staged.to_path_buf())));
            bail!("simulated replacement failure")
        },
    );
    ensure!(result.is_err());
    let staged_path = staged_path
        .borrow()
        .clone()
        .context("the replacer did not receive a staged path")?;
    ensure!(!staged_path.exists());
    Ok(())
}

fn context(current_version: &str) -> UpdateContext<'_> {
    UpdateContext {
        current_version,
        os: "linux",
        arch: "x86_64",
        translated: false,
        api_url: "https://example.invalid/latest",
        releases_url: "https://example.invalid/releases",
    }
}

#[cfg(unix)]
#[test]
fn relative_q_shortcut_replaces_the_resolved_binary() -> Result<()> {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let directory = tempfile::tempdir()?;
    let executable = directory.path().join("quinjet");
    let shortcut = directory.path().join("q");
    let staged = directory.path().join("staged");
    write_unix_executable(&executable, b"old")?;
    symlink("quinjet", &shortcut)?;
    fs::write(&staged, b"new")?;
    replace_executable(&shortcut, &staged)?;
    ensure!(fs::read(&executable)? == b"new");
    ensure!(fs::symlink_metadata(&shortcut)?.file_type().is_symlink());
    ensure!(fs::read_link(&shortcut)?.as_os_str() == "quinjet");
    ensure!(fs::metadata(&executable)?.permissions().mode() & 0o777 == 0o755);
    Ok(())
}

#[cfg(unix)]
#[test]
fn chained_q_shortcut_replaces_the_canonical_binary() -> Result<()> {
    use std::os::unix::fs::symlink;

    let cargo = tempfile::tempdir()?;
    let local = tempfile::tempdir()?;
    let canonical = cargo.path().join("quinjet");
    let linked = local.path().join("quinjet");
    let shortcut = local.path().join("q");
    let staged = local.path().join("staged");
    write_unix_executable(&canonical, b"old")?;
    symlink(&canonical, &linked)?;
    symlink("quinjet", &shortcut)?;
    fs::write(&staged, b"new")?;
    replace_executable(&shortcut, &staged)?;
    ensure!(fs::read(&canonical)? == b"new");
    ensure!(fs::symlink_metadata(&linked)?.file_type().is_symlink());
    ensure!(fs::symlink_metadata(&shortcut)?.file_type().is_symlink());
    ensure!(fs::read_link(&shortcut)?.as_os_str() == "quinjet");
    ensure!(fs::canonicalize(&shortcut)? == fs::canonicalize(&canonical)?);
    Ok(())
}

#[cfg(unix)]
fn write_unix_executable(path: &Path, contents: &[u8]) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::write(path, contents)?;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}
