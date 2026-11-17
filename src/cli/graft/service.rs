use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime};
use std::{env, fs, thread};

use anyhow::{Context, Result};
use serde::Serialize;

use super::{EXIT_FAILURE, Emitter, Failure, install, render_graft, serve};
use crate::graft::gateway::{
    self, DEFAULT_PORT, GatewayProcess, INSTALL_HINT, ServePlan, locate_gateway,
};
use crate::graft::service::{
    IDLE_LIMIT, START_LIMIT, ServicePaths, ServiceState, free_port, new_token, now_seconds,
};

const DAEMON_TICK: Duration = Duration::from_secs(1);
const IDLE_ENV: &str = "QUINJET_GRAFT_IDLE_SECONDS";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServiceDocument {
    pub running: bool,
    pub port: Option<u16>,
    pub mcp: Option<String>,
    pub vision: Option<String>,
    pub websocket: Option<String>,
    pub gateway: Option<String>,
    pub log: Option<String>,
    pub clients: usize,
    pub tools: usize,
}

impl ServiceDocument {
    pub(crate) fn from_state(paths: &ServicePaths, state: Option<&ServiceState>) -> Self {
        Self {
            running: state.is_some(),
            port: state.map(|state| state.port),
            mcp: state.map(ServiceState::mcp_url),
            vision: state.map(ServiceState::vision_url),
            websocket: state.map(ServiceState::websocket_url),
            gateway: state.map(|state| state.gateway.display().to_string()),
            log: state.map(|state| state.log.display().to_string()),
            clients: state.map_or(0, |_| paths.live_clients(SystemTime::now())),
            tools: super::receiver(true).methods.len(),
        }
    }
}

pub(crate) fn ensure_shared_gateway(
    explicit: Option<&Path>,
    install_missing: bool,
    report: &mut dyn FnMut(&str),
) -> Result<ServiceState> {
    let paths = ServicePaths::locate()?;
    let module = gateway::module_digest(&super::receiver(true))?;
    let gateway = match locate_gateway(explicit) {
        Ok(path) => path,
        Err(_) if install_missing && !gateway::gateway_configured(explicit) => {
            install::install(report)?
        }
        Err(error) => {
            return Err(Failure::new(EXIT_FAILURE, format!("{error:#}"))
                .hint(INSTALL_HINT)
                .into());
        }
    };
    if paths.running().is_none() {
        report("Starting the Graftcode gateway");
    }
    paths.ensure(&module, &mut || start_daemon(&gateway), START_LIMIT)
}

pub(super) fn start(out: &Emitter, gateway: Option<&Path>) -> Result<u8> {
    let mut report = |line: &str| out.note(line);
    let state = ensure_shared_gateway(gateway, true, &mut report)?;
    let document = ServiceDocument::from_state(&ServicePaths::locate()?, Some(&state));
    out.emit(&document, || render_graft::service(&document))?;
    Ok(0)
}

pub(super) fn status(out: &Emitter) -> Result<u8> {
    let paths = ServicePaths::locate()?;
    let state = paths.running();
    let document = ServiceDocument::from_state(&paths, state.as_ref());
    out.emit(&document, || render_graft::service(&document))?;
    Ok(0)
}

pub(super) fn stop(out: &Emitter) -> Result<u8> {
    let stopped = ServicePaths::locate()?.request_stop()?;
    let document = serde_json::json!({ "stopped": stopped.is_some() });
    out.emit(&document, || {
        if stopped.is_some() {
            "Stopped the shared Graftcode gateway\n".to_owned()
        } else {
            "The shared Graftcode gateway was not running\n".to_owned()
        }
    })?;
    Ok(0)
}

pub(super) fn install_command(out: &Emitter) -> Result<u8> {
    let path = if let Ok(path) = locate_gateway(None) {
        path
    } else {
        let mut report = |line: &str| out.note(line);
        install::install(&mut report)?
    };
    let document = serde_json::json!({ "gateway": path.display().to_string() });
    out.emit(&document, || {
        format!("The Graftcode gateway is at {}\n", path.display())
    })?;
    Ok(0)
}

pub(super) fn daemon(gateway: Option<&Path>) -> Result<u8> {
    let paths = ServicePaths::locate()?;
    fs::create_dir_all(&paths.root)
        .with_context(|| format!("failed to create {}", paths.root.display()))?;
    let outcome = supervise(&paths, gateway);
    if let Err(error) = &outcome {
        drop(fs::write(paths.failure(), format!("{error:#}\n")));
    }
    paths.clear_state(std::process::id());
    outcome
}

fn supervise(paths: &ServicePaths, gateway: Option<&Path>) -> Result<u8> {
    let gateway = locate_gateway(gateway)?;
    let receiver = super::receiver(true);
    let module = gateway::module_digest(&receiver)?;
    gateway::write_module(&paths.module_dir(), &receiver)?;
    let port =
        free_port(DEFAULT_PORT).context("no free pair of ports for the Graftcode gateway")?;
    let token = new_token();
    paths.touch_activity()?;
    let plan = ServePlan {
        gateway: gateway.clone(),
        module_dir: paths.module_dir(),
        repository: None,
        quinjet: current_executable()?,
        port,
        publish: false,
        token: Some(token.clone()),
        activity: Some(paths.activity()),
        log: paths.log(),
    };
    let mut process = spawn(&plan)?;
    gateway::wait_until_ready(port, &mut process.0, &plan.log, START_LIMIT)?;
    paths.write_state(&ServiceState {
        pid: std::process::id(),
        port,
        token,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        module,
        gateway,
        log: plan.log.clone(),
        started: now_seconds(),
    })?;
    let stop = serve::stop_signal()?;
    let idle = idle_limit();
    let mut quiet_since = Instant::now();
    loop {
        thread::sleep(DAEMON_TICK);
        if process
            .0
            .try_wait()
            .context("failed to watch the Graftcode gateway")?
            .is_some()
            || stop.load(Ordering::SeqCst)
            || paths.stop_requested()
        {
            break;
        }
        if paths.live_clients(SystemTime::now()) > 0 {
            quiet_since = Instant::now();
        } else if quiet_since.elapsed() >= idle {
            break;
        }
    }
    drop(process);
    Ok(0)
}

fn idle_limit() -> Duration {
    env::var(IDLE_ENV)
        .ok()
        .and_then(|seconds| seconds.parse().ok())
        .map_or(IDLE_LIMIT, Duration::from_secs)
}

fn current_executable() -> Result<PathBuf> {
    // nosemgrep: rust.lang.security.current-exe.current-exe
    env::current_exe().context("could not find the running Quinjet executable")
}

fn start_daemon(gateway: &Path) -> Result<()> {
    let mut command = std::process::Command::new(current_executable()?);
    let _ = command
        .args(["graft", "daemon", "--gateway"])
        .arg(gateway)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    detach(&mut command);
    let mut child = command
        .spawn()
        .context("failed to start the Graftcode gateway supervisor")?;
    drop(
        thread::Builder::new()
            .name("quinjet-graft-reaper".to_owned())
            .spawn(move || drop(child.wait())),
    );
    Ok(())
}

#[cfg(unix)]
fn detach(command: &mut std::process::Command) {
    use std::os::unix::process::CommandExt as _;
    let _ = command.process_group(0);
}

#[cfg(windows)]
fn detach(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt as _;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    let _ = command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(any(unix, windows)))]
fn detach(_command: &mut std::process::Command) {}

pub(super) fn spawn(plan: &ServePlan) -> Result<GatewayProcess> {
    if let Some(parent) = plan.log.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let log = fs::File::create(&plan.log)
        .with_context(|| format!("failed to create {}", plan.log.display()))?;
    let mut command = std::process::Command::new(&plan.gateway);
    let _ = command
        .args(plan.arguments())
        .current_dir(&plan.module_dir)
        .env("QUINJET_GRAFT_BIN", &plan.quinjet)
        .stdin(Stdio::null())
        .stdout(log.try_clone().context("failed to share the gateway log")?)
        .stderr(log);
    for (name, value) in [
        (
            "QUINJET_GRAFT_REPO",
            plan.repository
                .as_ref()
                .map(|path| path.as_os_str().to_owned()),
        ),
        (
            "QUINJET_GRAFT_TOKEN",
            plan.token.as_ref().map(OsString::from),
        ),
        (
            "QUINJET_GRAFT_ACTIVITY",
            plan.activity
                .as_ref()
                .map(|path| path.as_os_str().to_owned()),
        ),
    ] {
        let _ = match value {
            Some(value) => command.env(name, value),
            None => command.env_remove(name),
        };
    }
    let child = command
        .spawn()
        .with_context(|| format!("failed to start {}", plan.gateway.display()))?;
    Ok(GatewayProcess(child))
}
