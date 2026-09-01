use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail, ensure};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{EXIT_UNAVAILABLE, Emitter, Failure, completion, package_manager};
use package_manager::ManagerKind;

const API_URL: &str = "https://api.github.com/repos/pulkitxm/quinjet/releases/latest";
const RELEASES_URL: &str = "https://github.com/pulkitxm/quinjet/releases";
const API_LIMIT: usize = 1024 * 1024;
const CHECKSUM_LIMIT: usize = 64 * 1024;
const BINARY_LIMIT: usize = 32 * 1024 * 1024;
#[cfg(not(windows))]
const NETWORK_TIMEOUT_SECONDS: &str = "30";
const USER_AGENT: &str = concat!("quinjet/", env!("CARGO_PKG_VERSION"));

pub(super) fn run(out: &Emitter, check_only: bool) -> Result<u8> {
    let executable = running_executable()?;
    if let Some(manager) = (!check_only)
        .then(|| package_manager::manager(&executable))
        .flatten()
    {
        if manager.kind == ManagerKind::Homebrew {
            return run_homebrew_upgrade(out);
        }
        return Err(Failure::new(
            EXIT_UNAVAILABLE,
            format!(
                "{} owns this executable, so Quinjet will not replace it",
                manager.name
            ),
        )
        .hint(format!("run `{}` instead", manager.upgrade))
        .into());
    }
    let context = UpdateContext {
        current_version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        translated: rosetta_translated(),
        api_url: API_URL,
        releases_url: RELEASES_URL,
    };
    let downloader = Downloader::detect()?;
    let result = perform_update(
        &context,
        check_only,
        |url, limit| {
            if url == API_URL {
                out.set_progress("Fetching release metadata");
            } else if url.ends_with("SHA256SUMS") {
                out.set_progress("Fetching release checksums");
            } else {
                out.set_progress("Downloading update");
            }
            downloader.fetch(url, limit)
        },
        |staged| {
            out.set_progress("Installing verified update");
            replace_executable(&executable, staged)
        },
    )?;
    if result.status == UpdateStatus::Updated {
        out.set_progress("Refreshing shell completions");
        completion::refresh_replaced_executable(&executable)?;
    }
    out.emit(&result, || result.text())?;
    Ok(0)
}

fn run_homebrew_upgrade(out: &Emitter) -> Result<u8> {
    out.finish_progress();
    out.note(
        "warning: Homebrew owns this installation; running `brew upgrade quinjet` for you. You can run that command directly next time.",
    );
    let mut command = homebrew_upgrade_command();
    if out.json {
        let output = command
            .stdin(Stdio::null())
            .output()
            .context("failed to start `brew upgrade quinjet`")?;
        ensure!(
            output.status.success(),
            "`brew upgrade quinjet` failed: {}",
            homebrew_failure(&output.stdout, &output.stderr)
        );
        out.message("Homebrew upgraded Quinjet")?;
    } else {
        let status = command
            .status()
            .context("failed to start `brew upgrade quinjet`")?;
        ensure!(status.success(), "`brew upgrade quinjet` failed");
    }
    Ok(0)
}

#[expect(
    unused_results,
    reason = "building a process command mutates and returns the command"
)]
fn homebrew_upgrade_command() -> Command {
    let mut command = Command::new("brew");
    command.args(["upgrade", "quinjet"]);
    command
}

fn homebrew_failure<'a>(stdout: &'a [u8], stderr: &'a [u8]) -> std::borrow::Cow<'a, str> {
    let stderr = String::from_utf8_lossy(stderr);
    if stderr.trim().is_empty() {
        String::from_utf8_lossy(stdout)
    } else {
        stderr
    }
}

fn running_executable() -> Result<PathBuf> {
    let executable = std::env::current_exe() // nosemgrep: rust.lang.security.current-exe.current-exe
        .context("failed to locate the running Quinjet executable")?;
    resolve_executable(&executable)
}

fn resolve_executable(current: &Path) -> Result<PathBuf> {
    #[cfg(unix)]
    {
        current.canonicalize().with_context(|| {
            format!(
                "failed to resolve the running executable {}",
                current.display()
            )
        })
    }
    #[cfg(not(unix))]
    {
        Ok(current.to_path_buf())
    }
}

fn replace_executable(current: &Path, staged: &Path) -> Result<()> {
    let executable = resolve_executable(current)?;
    #[cfg(unix)]
    {
        replace_unix_executable(&executable, staged)
    }
    #[cfg(windows)]
    {
        self_replace::self_replace(staged).with_context(|| {
            format!(
                "failed to replace the running executable {}",
                executable.display()
            )
        })
    }
    #[cfg(not(any(unix, windows)))]
    {
        bail!(
            "replacing {} is not supported on this platform",
            executable.display()
        )
    }
}

#[cfg(unix)]
fn replace_unix_executable(executable: &Path, staged: &Path) -> Result<()> {
    let parent = executable
        .parent()
        .context("the Quinjet executable has no parent directory")?;
    let permissions = executable
        .metadata()
        .with_context(|| format!("failed to read permissions for {}", executable.display()))?
        .permissions();
    let prefix = executable
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map_or_else(
            || String::from(".__temp__"),
            |stem| {
                let mut prefix = String::from(".");
                prefix.push_str(stem);
                prefix.push_str(".__temp__");
                prefix
            },
        );
    let tmp = tempfile::Builder::new()
        .prefix(&prefix)
        .tempfile_in(parent)
        .context("failed to stage the update beside the running executable")?;
    let copied = fs::copy(staged, tmp.path()).context("failed to copy the staged update")?;
    let staged_len = fs::metadata(staged)
        .context("failed to inspect the staged update")?
        .len();
    ensure!(
        copied == staged_len,
        "the staged update was not copied in full"
    );
    fs::set_permissions(tmp.path(), permissions)
        .context("failed to preserve executable permissions")?;
    drop(
        tmp.persist(executable)
            .map_err(|error| error.error)
            .context("failed to replace the running executable")?,
    );
    Ok(())
}

struct UpdateContext<'a> {
    current_version: &'a str,
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
                "Updated Quinjet from {} to {}\n",
                self.current_version, self.latest_version
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
    replacer: impl FnOnce(&Path) -> Result<()>,
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
        });
    }
    let asset = asset_for(context.os, context.arch, context.translated)?;
    if check_only {
        return Ok(UpdateResult {
            status: UpdateStatus::Available,
            current_version: current.to_string(),
            latest_version: release.version.to_string(),
            asset: Some(asset.to_owned()),
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
    let mut staged = tempfile::NamedTempFile::new().context("failed to stage the update")?;
    staged
        .write_all(&binary)
        .context("failed to write the staged update")?;
    staged
        .flush()
        .context("failed to flush the staged update")?;
    let staged = staged.into_temp_path();
    replacer(staged.as_ref())?;
    Ok(UpdateResult {
        status: UpdateStatus::Updated,
        current_version: current.to_string(),
        latest_version: release.version.to_string(),
        asset: Some(asset.to_owned()),
    })
}

#[derive(Clone, Copy)]
enum Downloader {
    #[cfg(not(windows))]
    Curl,
    #[cfg(not(windows))]
    Wget,
    #[cfg(windows)]
    PowerShell,
}

impl Downloader {
    fn detect() -> Result<Self> {
        #[cfg(windows)]
        {
            if command_available("powershell", &["-NoProfile", "-Command", "exit 0"]) {
                return Ok(Self::PowerShell);
            }
            bail!("PowerShell is required to download a Quinjet update")
        }
        #[cfg(not(windows))]
        {
            if command_available("curl", &["--version"]) {
                return Ok(Self::Curl);
            }
            if command_available("wget", &["--version"]) {
                return Ok(Self::Wget);
            }
            bail!("curl or wget is required to download a Quinjet update")
        }
    }

    fn fetch(self, url: &str, limit: usize) -> Result<Vec<u8>> {
        let destination = tempfile::NamedTempFile::new()
            .context("failed to create a temporary download")?
            .into_temp_path();
        let output = match self {
            #[cfg(not(windows))]
            Self::Curl => Command::new("curl")
                .args([
                    "--proto",
                    "=https",
                    "--tlsv1.2",
                    "--fail",
                    "--silent",
                    "--show-error",
                    "--location",
                    "--max-time",
                    NETWORK_TIMEOUT_SECONDS,
                    "--max-filesize",
                    &limit.to_string(),
                    "--user-agent",
                    USER_AGENT,
                    "--output",
                ])
                .arg(destination.as_os_str())
                .arg(url)
                .output(),
            #[cfg(not(windows))]
            Self::Wget => Command::new("wget")
                .args([
                    "--quiet",
                    "--https-only",
                    "--timeout",
                    NETWORK_TIMEOUT_SECONDS,
                    "--tries",
                    "1",
                    "--user-agent",
                    USER_AGENT,
                    "--output-document",
                ])
                .arg(destination.as_os_str())
                .arg(url)
                .output(),
            #[cfg(windows)]
            Self::PowerShell => Command::new("powershell")
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "$ErrorActionPreference='Stop'; $ProgressPreference='SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Uri $args[0] -OutFile $args[1] -TimeoutSec 30 -Headers @{'User-Agent'=$args[2]}",
                ])
                .arg(url)
                .arg(destination.as_os_str())
                .arg(USER_AGENT)
                .output(),
        }
        .with_context(|| format!("failed to start a downloader for {url}"))?;
        ensure!(
            output.status.success(),
            "failed to download {url}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        let length = fs::metadata(&destination)
            .with_context(|| format!("failed to inspect the download from {url}"))?
            .len();
        ensure!(
            length <= u64::try_from(limit).context("the download limit does not fit in 64 bits")?,
            "download from {url} exceeded {limit} bytes"
        );
        fs::read(&destination).with_context(|| format!("failed to read the download from {url}"))
    }
}

fn command_available(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
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
    hex::encode(Sha256::digest(bytes))
}

#[cfg(target_os = "macos")]
fn rosetta_translated() -> bool {
    Command::new("sysctl")
        .args(["-in", "sysctl.proc_translated"])
        .output()
        .is_ok_and(|output| output.status.success() && output.stdout == b"1\n")
}

#[cfg(not(target_os = "macos"))]
const fn rosetta_translated() -> bool {
    false
}

#[cfg(test)]
mod tests;
