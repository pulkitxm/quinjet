use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::{Duration, Instant};
use std::{env, fs, thread};

use anyhow::{Context, Result, bail};
use ureq::Agent;

use super::receiver::Receiver;

pub(crate) const DEFAULT_PORT: u16 = 7381;
pub(crate) const GATEWAY_ENV: &str = "QUINJET_GRAFTCODE_GATEWAY";
pub(crate) const INSTALL_HINT: &str = "install the Graftcode gateway with `curl -fsSL https://grft.dev/get | sh` (Windows: `iwr grft.dev/get | iex`), or download `gg` from https://github.com/grft-dev/graftcode-gateway/releases and pass --gateway";
const LOG_FILE: &str = "gateway.log";
const LOG_TAIL_BYTES: usize = 4096;
const POLL_INTERVAL: Duration = Duration::from_millis(400);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServePlan {
    pub gateway: PathBuf,
    pub module_dir: PathBuf,
    pub repository: PathBuf,
    pub quinjet: PathBuf,
    pub port: u16,
    pub publish: bool,
}

impl ServePlan {
    pub(crate) fn module_file(&self) -> PathBuf {
        self.module_dir.join("index.js")
    }

    pub(crate) fn log_file(&self) -> PathBuf {
        self.module_dir.join(LOG_FILE)
    }

    pub(crate) fn arguments(&self) -> Vec<OsString> {
        let mut arguments: Vec<OsString> = vec![
            "--runtime".into(),
            "nodejs".into(),
            "--modules".into(),
            self.module_file().into_os_string(),
            "--types".into(),
            super::receiver::CLASS_NAME.into(),
            "--port".into(),
            self.port.to_string().into(),
            "--httpPort".into(),
            self.port.saturating_add(1).to_string().into(),
        ];
        if !self.publish {
            arguments.push("--GSMU=0".into());
        }
        arguments
    }

    pub(crate) fn base_url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }
}

pub(crate) fn module_dir(writes: bool) -> Result<PathBuf> {
    let root = crate::git::github::cache_root()
        .context("could not find a cache directory; set QUINJET_CACHE_DIR")?;
    Ok(root.join("graft").join(format!(
        "{}-{}",
        env!("CARGO_PKG_VERSION"),
        if writes { "write" } else { "read" }
    )))
}

pub(crate) fn write_module(directory: &Path, receiver: &Receiver) -> Result<()> {
    fs::create_dir_all(directory)
        .with_context(|| format!("failed to create {}", directory.display()))?;
    for (name, contents) in [
        (
            "package.json",
            Receiver::package_json(env!("CARGO_PKG_VERSION"))?,
        ),
        ("index.js", receiver.index_js()?),
    ] {
        let path = directory.join(name);
        fs::write(&path, contents)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

pub(crate) fn locate_gateway(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        if path.is_file() {
            return Ok(path.to_path_buf());
        }
        bail!("no Graftcode gateway at {}", path.display());
    }
    if let Some(path) = env::var_os(GATEWAY_ENV).filter(|path| !path.is_empty()) {
        return locate_gateway(Some(Path::new(&path)));
    }
    let names: &[&str] = if cfg!(windows) {
        &["gg.exe", "gg"]
    } else {
        &["gg"]
    };
    env::var_os("PATH")
        .iter()
        .flat_map(env::split_paths)
        .flat_map(|directory| names.iter().map(move |name| directory.join(name)))
        .find(|candidate| candidate.is_file())
        .context("the Graftcode gateway `gg` is not on PATH")
}

pub(crate) fn wait_until_ready(
    port: u16,
    child: &mut Child,
    log: &Path,
    limit: Duration,
) -> Result<()> {
    let agent: Agent = Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .http_status_as_error(false)
        .build()
        .into();
    let url = format!("http://127.0.0.1:{port}/status");
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().context("failed to watch the gateway")? {
            bail!(
                "the gateway exited with {status} before it was ready\n{}",
                log_tail(log)
            );
        }
        if agent
            .get(&url)
            .call()
            .is_ok_and(|response| response.status().is_success())
        {
            return Ok(());
        }
        if started.elapsed() >= limit {
            bail!(
                "the gateway was not ready after {} seconds\n{}",
                limit.as_secs(),
                log_tail(log)
            );
        }
        thread::sleep(POLL_INTERVAL);
    }
}

pub(crate) fn log_tail(path: &Path) -> String {
    let Ok(bytes) = fs::read(path) else {
        return String::new();
    };
    let start = bytes.len().saturating_sub(LOG_TAIL_BYTES);
    let bytes = bytes.get(start..).unwrap_or_default();
    let text = String::from_utf8_lossy(bytes);
    let mut cleaned = String::new();
    for line in text
        .lines()
        .map(strip_ansi)
        .filter(|line| !line.contains(" - TRACE - ") && !line.trim().is_empty())
    {
        cleaned.push_str(&line);
        cleaned.push('\n');
    }
    cleaned
}

fn strip_ansi(line: &str) -> String {
    let mut text = String::with_capacity(line.len());
    let mut characters = line.chars();
    while let Some(character) = characters.next() {
        if character == '\u{1b}' {
            for next in characters.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            text.push(character);
        }
    }
    text
}

pub(crate) struct GatewayProcess(pub Child);

impl Drop for GatewayProcess {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            drop(self.0.kill());
            drop(self.0.wait());
        }
    }
}

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests {
    use super::*;

    fn plan(publish: bool) -> ServePlan {
        ServePlan {
            gateway: PathBuf::from("/opt/gg"),
            module_dir: PathBuf::from("/cache/graft/1-read"),
            repository: PathBuf::from("/work/repo"),
            quinjet: PathBuf::from("/bin/quinjet"),
            port: 7381,
            publish,
        }
    }

    #[test]
    fn the_gateway_hosts_only_the_generated_class_and_stays_local_by_default() {
        let arguments: Vec<String> = plan(false)
            .arguments()
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            arguments,
            [
                "--runtime",
                "nodejs",
                "--modules",
                "/cache/graft/1-read/index.js",
                "--types",
                "Quinjet",
                "--port",
                "7381",
                "--httpPort",
                "7382",
                "--GSMU=0",
            ]
        );
        assert!(
            !plan(true)
                .arguments()
                .iter()
                .any(|argument| argument == "--GSMU=0")
        );
    }

    #[test]
    fn log_tails_drop_trace_lines_and_colors() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(LOG_FILE);
        fs::write(
            &path,
            "\u{1b}[39m[Info] Starting\u{1b}[0m\n2026 - TRACE - noise\n\n[Error] broken\n",
        )
        .unwrap();
        assert_eq!(log_tail(&path), "[Info] Starting\n[Error] broken\n");
        assert_eq!(log_tail(&directory.path().join("missing")), "");
    }

    #[test]
    fn an_explicit_gateway_must_exist() {
        let directory = tempfile::tempdir().unwrap();
        let gateway = directory.path().join("gg");
        locate_gateway(Some(&gateway)).unwrap_err();
        fs::write(&gateway, b"").unwrap();
        assert_eq!(locate_gateway(Some(&gateway)).unwrap(), gateway);
    }
}
