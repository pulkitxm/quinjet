use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::Emitter;

const API_URL: &str = "https://api.github.com/repos/pulkitxm/quinjet/releases/latest";
const RELEASES_URL: &str = "https://github.com/pulkitxm/quinjet/releases";
const API_LIMIT: usize = 1024 * 1024;
const CHECKSUM_LIMIT: usize = 64 * 1024;
const BINARY_LIMIT: usize = 32 * 1024 * 1024;
const NETWORK_TIMEOUT: Duration = Duration::from_secs(30);
const USER_AGENT: &str = concat!("quinjet/", env!("CARGO_PKG_VERSION"));

pub(super) fn run(out: &Emitter, check_only: bool) -> Result<u8> {
    let executable = std::env::current_exe().context("failed to locate the running executable")?;
    let context = UpdateContext {
        current_version: env!("CARGO_PKG_VERSION"),
        executable: &executable,
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        translated: rosetta_translated(),
        api_url: API_URL,
        releases_url: RELEASES_URL,
    };
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(NETWORK_TIMEOUT))
        .https_only(true)
        .build()
        .into();
    let result = perform_update(
        &context,
        check_only,
        |url, limit| fetch(&agent, url, limit),
        |staged, _executable| {
            self_replace::self_replace(staged).context("failed to replace the running executable")
        },
    )?;
    out.emit(&result, || result.text())?;
    Ok(0)
}

struct UpdateContext<'a> {
    current_version: &'a str,
    executable: &'a Path,
    os: &'a str,
    arch: &'a str,
    translated: bool,
    api_url: &'a str,
    releases_url: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum UpdateStatus {
    UpToDate,
    Available,
    Updated,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateResult {
    status: UpdateStatus,
    current_version: String,
    latest_version: String,
    asset: Option<String>,
    path: Option<PathBuf>,
}

impl UpdateResult {
    fn text(&self) -> String {
        match self.status {
            UpdateStatus::UpToDate => {
                format!("Quinjet {} is up to date\n", self.current_version)
            }
            UpdateStatus::Available => format!(
                "Quinjet {} is available (current {})\n",
                self.latest_version, self.current_version
            ),
            UpdateStatus::Updated => format!(
                "Updated Quinjet from {} to {} at {}\n",
                self.current_version,
                self.latest_version,
                self.path.as_deref().map_or_else(
                    || "the running executable".to_owned(),
                    |path| path.display().to_string()
                )
            ),
        }
    }
}

#[derive(Deserialize)]
struct LatestRelease {
    tag_name: String,
}

struct Release {
    tag: String,
    version: Version,
}

fn perform_update(
    context: &UpdateContext<'_>,
    check_only: bool,
    mut fetcher: impl FnMut(&str, usize) -> Result<Vec<u8>>,
    replacer: impl FnOnce(&Path, &Path) -> Result<()>,
) -> Result<UpdateResult> {
    let current = Version::parse(context.current_version).with_context(|| {
        format!(
            "the compiled version '{}' is not semantic versioning",
            context.current_version
        )
    })?;
    let release = parse_release(&fetcher(context.api_url, API_LIMIT)?)?;
    if release.version <= current {
        return Ok(UpdateResult {
            status: UpdateStatus::UpToDate,
            current_version: current.to_string(),
            latest_version: release.version.to_string(),
            asset: None,
            path: None,
        });
    }
    let asset = asset_for(context.os, context.arch, context.translated)?;
    if check_only {
        return Ok(UpdateResult {
            status: UpdateStatus::Available,
            current_version: current.to_string(),
            latest_version: release.version.to_string(),
            asset: Some(asset.to_owned()),
            path: Some(context.executable.to_path_buf()),
        });
    }
    let release_url = format!("{}/download/{}", context.releases_url, release.tag);
    let checksum_url = format!("{release_url}/SHA256SUMS");
    let checksum_bytes = fetcher(&checksum_url, CHECKSUM_LIMIT)?;
    let checksum_document = std::str::from_utf8(&checksum_bytes)
        .context("the release checksum file was not valid UTF-8")?;
    let expected_checksum = release_checksum(checksum_document, asset)?;
    let binary_url = format!("{release_url}/{asset}");
    let binary = fetcher(&binary_url, BINARY_LIMIT)?;
    let actual_checksum = sha256(&binary);
    ensure!(
        actual_checksum == expected_checksum,
        "checksum verification failed for {asset}"
    );
    let directory = context
        .executable
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .context("the running executable has no parent directory")?;
    let mut staged = tempfile::NamedTempFile::new_in(directory)
        .with_context(|| format!("failed to stage the update in {}", directory.display()))?;
    staged
        .write_all(&binary)
        .context("failed to write the staged update")?;
    staged
        .flush()
        .context("failed to flush the staged update")?;
    let staged = staged.into_temp_path();
    replacer(staged.as_ref(), context.executable)?;
    Ok(UpdateResult {
        status: UpdateStatus::Updated,
        current_version: current.to_string(),
        latest_version: release.version.to_string(),
        asset: Some(asset.to_owned()),
        path: Some(context.executable.to_path_buf()),
    })
}

fn fetch(agent: &ureq::Agent, url: &str, limit: usize) -> Result<Vec<u8>> {
    let read_limit = u64::try_from(limit)
        .context("the download limit does not fit in 64 bits")?
        .saturating_add(1);
    let mut response = agent
        .get(url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .call()
        .with_context(|| format!("failed to download {url}"))?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(read_limit)
        .read_to_vec()
        .with_context(|| format!("failed to read {url}"))?;
    ensure!(
        bytes.len() <= limit,
        "download from {url} exceeded {limit} bytes"
    );
    Ok(bytes)
}

fn parse_release(document: &[u8]) -> Result<Release> {
    let latest: LatestRelease =
        serde_json::from_slice(document).context("GitHub returned invalid release metadata")?;
    let version_text = latest
        .tag_name
        .strip_prefix('v')
        .context("the latest release tag does not start with 'v'")?;
    let version = Version::parse(version_text)
        .with_context(|| format!("the latest release tag '{}' is invalid", latest.tag_name))?;
    ensure!(
        version.pre.is_empty(),
        "the latest release '{}' is not stable",
        latest.tag_name
    );
    Ok(Release {
        tag: latest.tag_name,
        version,
    })
}

fn asset_for(os: &str, arch: &str, translated: bool) -> Result<&'static str> {
    match (os, arch, translated) {
        ("linux", "x86_64", _) => Ok("quinjet-linux-x86_64"),
        ("linux", "aarch64", _) => Ok("quinjet-linux-aarch64"),
        ("macos", "x86_64", false) => Ok("quinjet-macos-x86_64"),
        ("macos", "x86_64" | "aarch64", true) | ("macos", "aarch64", false) => {
            Ok("quinjet-macos-aarch64")
        }
        ("windows", "x86_64" | "aarch64", _) => Ok("quinjet-windows-x86_64.exe"),
        _ => bail!("Quinjet does not publish a release for {os} {arch}"),
    }
}

fn release_checksum(document: &str, asset: &str) -> Result<String> {
    let mut matches = document.lines().filter_map(|line| {
        let mut fields = line.split_ascii_whitespace();
        let checksum = fields.next()?;
        let name = fields.next()?.trim_start_matches('*');
        let name = name.strip_prefix("dist/").unwrap_or(name);
        (name == asset).then_some(checksum)
    });
    let checksum = matches
        .next()
        .with_context(|| format!("the release checksum for {asset} is missing"))?;
    ensure!(
        matches.next().is_none(),
        "the release checksum for {asset} is duplicated"
    );
    ensure!(
        checksum.len() == 64 && checksum.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "the release checksum for {asset} is invalid"
    );
    Ok(checksum.to_ascii_lowercase())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(target_os = "macos")]
fn rosetta_translated() -> bool {
    std::process::Command::new("sysctl")
        .args(["-in", "sysctl.proc_translated"])
        .output()
        .is_ok_and(|output| output.status.success() && output.stdout == b"1\n")
}

#[cfg(not(target_os = "macos"))]
const fn rosetta_translated() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::fs;

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
            let scratch = tempfile::tempdir()?;
            let executable = scratch.path().join("quinjet");
            fs::write(&executable, b"old")?;
            let context = context(&executable, "1.2.3");
            let requests = RefCell::new(Vec::new());
            let replaced = Cell::new(false);
            let result = perform_update(
                &context,
                false,
                |url, _limit| {
                    requests.borrow_mut().push(url.to_owned());
                    Ok(format!(r#"{{"tag_name":"{tag}"}}"#).into_bytes())
                },
                |_staged, _destination| {
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
        let scratch = tempfile::tempdir()?;
        let executable = scratch.path().join("quinjet");
        let context = context(&executable, "1.2.3");
        let requests = RefCell::new(Vec::new());
        let result = perform_update(
            &context,
            true,
            |url, _limit| {
                requests.borrow_mut().push(url.to_owned());
                Ok(br#"{"tag_name":"v1.3.0"}"#.to_vec())
            },
            |_staged, _destination| bail!("check-only mode tried to replace the executable"),
        )?;
        ensure!(result.status == UpdateStatus::Available);
        ensure!(requests.borrow().as_slice() == [context.api_url]);
        Ok(())
    }

    #[test]
    fn update_pins_downloads_verifies_bytes_and_targets_the_running_path() -> Result<()> {
        let scratch = tempfile::tempdir()?;
        let executable = scratch.path().join("custom-bin").join("quinjet");
        fs::create_dir_all(
            executable
                .parent()
                .context("test executable has no parent")?,
        )?;
        fs::write(&executable, b"old")?;
        let context = context(&executable, "1.2.3");
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
            |staged, destination| {
                ensure!(destination == executable);
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
                    "https://example.invalid/releases/download/v1.3.0/quinjet-linux-x86_64"
                        .to_owned(),
                ]
        );
        Ok(())
    }

    #[test]
    fn checksum_failure_preserves_the_running_executable() -> Result<()> {
        let scratch = tempfile::tempdir()?;
        let executable = scratch.path().join("quinjet");
        fs::write(&executable, b"old")?;
        let context = context(&executable, "1.2.3");
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
            |_staged, _destination| {
                replaced.set(true);
                Ok(())
            },
        );
        ensure!(result.is_err());
        ensure!(!replaced.get());
        ensure!(fs::read(executable)? == b"old");
        Ok(())
    }

    #[test]
    fn replacement_failure_preserves_the_executable_and_removes_the_stage() -> Result<()> {
        let scratch = tempfile::tempdir()?;
        let executable = scratch.path().join("quinjet");
        fs::write(&executable, b"old")?;
        let context = context(&executable, "1.2.3");
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
            |staged, _destination| {
                drop(staged_path.replace(Some(staged.to_path_buf())));
                bail!("simulated replacement failure")
            },
        );
        ensure!(result.is_err());
        ensure!(fs::read(&executable)? == b"old");
        let staged_path = staged_path
            .borrow()
            .clone()
            .context("the replacer did not receive a staged path")?;
        ensure!(!staged_path.exists());
        Ok(())
    }

    fn context<'a>(executable: &'a Path, current_version: &'a str) -> UpdateContext<'a> {
        UpdateContext {
            current_version,
            executable,
            os: "linux",
            arch: "x86_64",
            translated: false,
            api_url: "https://example.invalid/latest",
            releases_url: "https://example.invalid/releases",
        }
    }
}
