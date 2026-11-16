use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use super::super::update::Downloader;
use crate::graft::gateway::{gateway_names, installed_gateway, installed_root};

const RELEASE_API: &str = "https://api.github.com/repos/grft-dev/graftcode-gateway/releases/latest";
const RELEASE_LIMIT: usize = 1024 * 1024;
const ARCHIVE_LIMIT: usize = 512 * 1024 * 1024;
const ARCHIVE_TIMEOUT_SECONDS: u64 = 1800;

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Debug, Clone, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

pub(crate) fn asset_name(os: &str, arch: &str) -> Option<&'static str> {
    Some(match (os, arch) {
        ("macos", "aarch64") => "gg_macos_arm64.tar.gz",
        ("macos", "x86_64") => "gg_macos_x86_64.tar.gz",
        ("linux", "x86_64") => "gg_linux_amd64.tar.gz",
        ("linux", "aarch64") => "gg_linux_arm64.tar.gz",
        ("windows", "x86_64") => "gg_windows_amd64.zip",
        ("windows", "aarch64") => "gg_windows_arm64.zip",
        _ => return None,
    })
}

pub(crate) fn install(report: &mut dyn FnMut(&str)) -> Result<PathBuf> {
    let name = asset_name(std::env::consts::OS, std::env::consts::ARCH).with_context(|| {
        format!(
            "Graftcode publishes no gateway for {} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })?;
    let downloader = Downloader::detect()?;
    let release: Release = serde_json::from_slice(&downloader.fetch(RELEASE_API, RELEASE_LIMIT)?)
        .context("GitHub returned invalid Graftcode release metadata")?;
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == name)
        .with_context(|| format!("Graftcode {} has no {name}", release.tag_name))?;
    let root = installed_root()?;
    let target = root.join(&release.tag_name);
    if let Some(existing) = installed_gateway(&root).filter(|path| path.starts_with(&target)) {
        return Ok(existing);
    }
    fs::create_dir_all(&root).with_context(|| format!("failed to create {}", root.display()))?;
    let staging = tempfile::Builder::new()
        .prefix(".download-")
        .tempdir_in(&root)
        .context("failed to stage the Graftcode download")?;
    let archive = staging.path().join(&asset.name);
    report(&format!(
        "Downloading the Graftcode gateway {} ({} MB)",
        release.tag_name,
        asset.size.div_ceil(1024 * 1024)
    ));
    downloader.download(
        &asset.browser_download_url,
        &archive,
        ARCHIVE_LIMIT,
        ARCHIVE_TIMEOUT_SECONDS,
    )?;
    verify(&archive, asset.digest.as_deref())?;
    report("Unpacking the Graftcode gateway");
    let unpacked = staging.path().join("unpacked");
    fs::create_dir_all(&unpacked)
        .with_context(|| format!("failed to create {}", unpacked.display()))?;
    unpack(&archive, &unpacked)?;
    let binary = find_binary(&unpacked)?
        .with_context(|| format!("{} did not contain the gateway", asset.name))?;
    let file_name = binary
        .file_name()
        .context("the gateway had no file name")?
        .to_owned();
    let version_dir = staging.path().join("version");
    fs::create_dir_all(&version_dir)
        .with_context(|| format!("failed to create {}", version_dir.display()))?;
    fs::rename(&binary, version_dir.join(&file_name))
        .context("failed to move the gateway into place")?;
    make_executable(&version_dir.join(&file_name))?;
    if target.exists() {
        fs::remove_dir_all(&target)
            .with_context(|| format!("failed to replace {}", target.display()))?;
    }
    fs::rename(&version_dir, &target)
        .with_context(|| format!("failed to install into {}", target.display()))?;
    Ok(target.join(file_name))
}

fn verify(archive: &Path, digest: Option<&str>) -> Result<()> {
    let expected = digest
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .context("GitHub published no checksum for the Graftcode gateway")?;
    let mut file =
        fs::File::open(archive).with_context(|| format!("failed to read {}", archive.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read =
            io::Read::read(&mut file, &mut buffer).context("failed to checksum the download")?;
        if read == 0 {
            break;
        }
        hasher.update(buffer.get(..read).unwrap_or_default());
    }
    let actual = hex::encode(hasher.finalize());
    ensure!(
        actual.eq_ignore_ascii_case(expected),
        "the Graftcode gateway download failed checksum verification"
    );
    Ok(())
}

fn unpack(archive: &Path, destination: &Path) -> Result<()> {
    let flags = if archive
        .extension()
        .is_some_and(|extension| extension == "zip")
    {
        "-xf"
    } else {
        "-xzf"
    };
    let output = std::process::Command::new("tar")
        .arg(flags)
        .arg(archive)
        .arg("-C")
        .arg(destination)
        .stdin(Stdio::null())
        .output()
        .context("failed to run tar to unpack the Graftcode gateway")?;
    if !output.status.success() {
        bail!(
            "tar could not unpack the Graftcode gateway: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn find_binary(directory: &Path) -> Result<Option<PathBuf>> {
    let mut pending = vec![directory.to_path_buf()];
    while let Some(folder) = pending.pop() {
        for entry in
            fs::read_dir(&folder).with_context(|| format!("failed to read {}", folder.display()))?
        {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| gateway_names().contains(&name))
            {
                return Ok(Some(path));
            }
        }
    }
    Ok(None)
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .with_context(|| format!("failed to make {} executable", path.display()))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests {
    use super::*;

    #[test]
    fn every_supported_platform_has_an_asset() {
        assert_eq!(
            asset_name("macos", "aarch64"),
            Some("gg_macos_arm64.tar.gz")
        );
        assert_eq!(asset_name("linux", "x86_64"), Some("gg_linux_amd64.tar.gz"));
        assert_eq!(
            asset_name("windows", "aarch64"),
            Some("gg_windows_arm64.zip")
        );
        assert_eq!(asset_name("freebsd", "x86_64"), None);
    }

    #[test]
    fn downloads_must_match_the_published_checksum() {
        let directory = tempfile::tempdir().unwrap();
        let archive = directory.path().join("gg.tar.gz");
        fs::write(&archive, b"gateway").unwrap();
        let good = format!("sha256:{}", hex::encode(Sha256::digest(b"gateway")));
        verify(&archive, Some(&good)).unwrap();
        verify(&archive, Some("sha256:00")).unwrap_err();
        verify(&archive, None).unwrap_err();
    }

    #[test]
    fn the_gateway_is_found_anywhere_in_the_archive() {
        let directory = tempfile::tempdir().unwrap();
        let nested = directory.path().join("usr").join("local").join("bin");
        fs::create_dir_all(&nested).unwrap();
        assert_eq!(find_binary(directory.path()).unwrap(), None);
        fs::write(nested.join(gateway_names()[0]), b"").unwrap();
        assert_eq!(
            find_binary(directory.path()).unwrap(),
            Some(nested.join(gateway_names()[0]))
        );
    }
}
