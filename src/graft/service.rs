use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub(crate) const IDLE_LIMIT: Duration = Duration::from_secs(10 * 60);
pub(crate) const START_LIMIT: Duration = Duration::from_secs(180);
const LEASE_FRESH: Duration = Duration::from_secs(90);
const LEASE_TOUCH: Duration = Duration::from_secs(30);
const LOCK_STALE: Duration = Duration::from_secs(240);
const INSTALL_STALE: Duration = Duration::from_secs(1900);
const STOP_LIMIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(200);
const PORT_ATTEMPTS: u16 = 40;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServiceState {
    pub pid: u32,
    pub port: u16,
    pub token: String,
    pub version: String,
    pub module: String,
    pub gateway: PathBuf,
    pub log: PathBuf,
    pub started: u64,
}

impl ServiceState {
    pub(crate) fn mcp_url(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.port)
    }

    pub(crate) fn vision_url(&self) -> String {
        format!("http://localhost:{}/GV", self.port)
    }

    pub(crate) fn websocket_url(&self) -> String {
        format!("ws://localhost:{}/ws", self.port)
    }

    pub(crate) fn healthy(&self) -> bool {
        healthy(self.port)
    }

    fn current(&self, module: &str) -> bool {
        self.version == env!("CARGO_PKG_VERSION") && self.module == module
    }
}

pub(crate) fn healthy(port: u16) -> bool {
    super::http::request(
        "GET",
        &format!("http://127.0.0.1:{port}/status"),
        &[],
        &[],
        Duration::from_secs(2),
        4096,
    )
    .is_ok_and(|response| (200..300).contains(&response.status))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServicePaths {
    pub root: PathBuf,
}

impl ServicePaths {
    pub(crate) fn locate() -> Result<Self> {
        Ok(Self {
            root: crate::git::github::cache_root()
                .context("could not find a cache directory; set QUINJET_CACHE_DIR")?
                .join("graft")
                .join("service"),
        })
    }

    pub(crate) fn state(&self) -> PathBuf {
        self.root.join("state.json")
    }

    pub(crate) fn module_dir(&self) -> PathBuf {
        self.root.join("module")
    }

    pub(crate) fn log(&self) -> PathBuf {
        self.root.join("gateway.log")
    }

    pub(crate) fn activity(&self) -> PathBuf {
        self.root.join("activity")
    }

    pub(crate) fn failure(&self) -> PathBuf {
        self.root.join("failure")
    }

    fn stop(&self) -> PathBuf {
        self.root.join("stop")
    }

    fn lock(&self) -> PathBuf {
        self.root.join("lock")
    }

    fn leases(&self) -> PathBuf {
        self.root.join("leases")
    }

    pub(crate) fn read_state(&self) -> Option<ServiceState> {
        serde_json::from_slice(&fs::read(self.state()).ok()?).ok()
    }

    pub(crate) fn write_state(&self, state: &ServiceState) -> Result<()> {
        write_private(&self.state(), &serde_json::to_vec_pretty(state)?)
    }

    pub(crate) fn clear_state(&self, pid: u32) {
        if self.read_state().is_none_or(|state| state.pid == pid) {
            drop(fs::remove_file(self.state()));
            drop(fs::remove_file(self.stop()));
        }
    }

    pub(crate) fn running(&self) -> Option<ServiceState> {
        self.read_state().filter(ServiceState::healthy)
    }

    pub(crate) fn stop_requested(&self) -> bool {
        self.stop().exists()
    }

    pub(crate) fn request_stop(&self) -> Result<Option<ServiceState>> {
        let Some(state) = self.running() else {
            return Ok(None);
        };
        write_private(&self.stop(), b"stop\n")?;
        let started = Instant::now();
        while state.healthy() && started.elapsed() < STOP_LIMIT {
            thread::sleep(POLL);
        }
        if state.healthy() {
            bail!("the Graftcode gateway on port {} did not stop", state.port);
        }
        Ok(Some(state))
    }

    pub(crate) fn live_clients(&self, now: SystemTime) -> usize {
        let fresh = |path: &Path| {
            fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .is_some_and(|age| age < LEASE_FRESH)
        };
        let leases = fs::read_dir(self.leases())
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                    .filter(|path| {
                        let alive = fresh(path);
                        if !alive {
                            drop(fs::remove_file(path));
                        }
                        alive
                    })
                    .count()
            })
            .unwrap_or_default();
        leases.saturating_add(usize::from(fresh(&self.activity())))
    }

    pub(crate) fn touch_activity(&self) -> Result<()> {
        touch(&self.activity())
    }

    pub(crate) fn ensure(
        &self,
        module: &str,
        start: &mut dyn FnMut() -> Result<()>,
        limit: Duration,
    ) -> Result<ServiceState> {
        if let Some(state) = self.running().filter(|state| state.current(module)) {
            return Ok(state);
        }
        let _lock = StartLock::acquire(&self.root, &self.lock(), limit, LOCK_STALE)?;
        if let Some(state) = self.running() {
            if state.current(module) {
                return Ok(state);
            }
            drop(self.request_stop()?);
        }
        drop(fs::remove_file(self.state()));
        drop(fs::remove_file(self.failure()));
        start()?;
        let started = Instant::now();
        loop {
            if let Some(state) = self.running().filter(|state| state.current(module)) {
                return Ok(state);
            }
            if let Ok(failure) = fs::read_to_string(self.failure()) {
                bail!("the Graftcode gateway could not start: {}", failure.trim());
            }
            if started.elapsed() >= limit {
                bail!(
                    "the Graftcode gateway was not ready after {} seconds; see {}",
                    limit.as_secs(),
                    self.log().display()
                );
            }
            thread::sleep(POLL);
        }
    }

    pub(crate) fn lease(&self) -> Result<Lease> {
        fs::create_dir_all(self.leases())
            .with_context(|| format!("failed to create {}", self.leases().display()))?;
        let path = self.leases().join(format!(
            "{}-{:08x}",
            std::process::id(),
            rand::random::<u32>()
        ));
        touch(&path)?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let path = path.clone();
            let stop = Arc::clone(&stop);
            thread::Builder::new()
                .name("quinjet-graft-lease".to_owned())
                .spawn(move || {
                    let mut last = Instant::now();
                    while !stop.load(Ordering::SeqCst) {
                        if last.elapsed() >= LEASE_TOUCH {
                            drop(touch(&path));
                            last = Instant::now();
                        }
                        thread::sleep(POLL);
                    }
                })
                .context("failed to start the gateway lease")?
        };
        Ok(Lease {
            path,
            stop,
            thread: Some(thread),
        })
    }
}

#[derive(Debug)]
pub(crate) struct Lease {
    path: PathBuf,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            drop(thread.join());
        }
        drop(fs::remove_file(&self.path));
    }
}

pub(crate) struct StartLock(PathBuf);

impl StartLock {
    pub(crate) fn installing(paths: &ServicePaths) -> Result<Self> {
        Self::acquire(
            &paths.root,
            &paths.root.join("install.lock"),
            INSTALL_STALE,
            INSTALL_STALE,
        )
    }

    fn acquire(root: &Path, path: &Path, limit: Duration, stale_after: Duration) -> Result<Self> {
        fs::create_dir_all(root).with_context(|| format!("failed to create {}", root.display()))?;
        let path = path.to_path_buf();
        let started = Instant::now();
        loop {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    drop(writeln!(file, "{}", std::process::id()));
                    return Ok(Self(path));
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let stale = fs::metadata(&path)
                        .and_then(|metadata| metadata.modified())
                        .ok()
                        .and_then(|modified| modified.elapsed().ok())
                        .is_some_and(|age| age >= stale_after);
                    if stale {
                        drop(fs::remove_file(&path));
                        continue;
                    }
                }
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to create {}", path.display()));
                }
            }
            if started.elapsed() >= limit {
                bail!(
                    "another Quinjet is starting the Graftcode gateway; remove {} if none is",
                    path.display()
                );
            }
            thread::sleep(POLL);
        }
    }
}

impl Drop for StartLock {
    fn drop(&mut self) {
        drop(fs::remove_file(&self.0));
    }
}

pub(crate) fn free_port(first: u16) -> Option<u16> {
    (0..PORT_ATTEMPTS)
        .filter_map(|step| first.checked_add(step.saturating_mul(2)))
        .find(|port| {
            [*port, port.saturating_add(1)]
                .iter()
                .all(|candidate| TcpListener::bind(("0.0.0.0", *candidate)).is_ok())
        })
}

pub(crate) fn new_token() -> String {
    let bytes: [u8; 24] = rand::random();
    hex::encode(bytes)
}

pub(crate) fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}

fn touch(path: &Path) -> Result<()> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to touch {}", path.display()))?;
    file.set_modified(SystemTime::now())
        .with_context(|| format!("failed to touch {}", path.display()))
}

fn write_private(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut options = OpenOptions::new();
    let _ = options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        let _ = options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("failed to write {}", path.display()))?;
    file.write_all(contents)
        .with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
