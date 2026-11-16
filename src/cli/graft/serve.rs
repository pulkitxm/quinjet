use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};

use super::{Emitter, ServeArgs, ServeDocument, plan, receiver, render_graft, spawn};
use crate::git::Repository;
use crate::graft::call::LIST_TIMEOUT;
use crate::graft::client::McpSession;
use crate::graft::gateway;
use crate::graft::receiver::Access;
use crate::graft::registry::{self, Gateway};

const WATCH_INTERVAL: Duration = Duration::from_millis(200);

pub(super) fn serve(out: &mut Emitter, repository: &Path, args: &ServeArgs) -> Result<u8> {
    let repository = Repository::discover(repository)?;
    if let Some(name) = &args.register {
        registry::validate_name(name)?;
    }
    let plan = plan(args, &repository)?;
    let receiver = receiver(args.allow_writes);
    gateway::write_module(&plan.module_dir, &receiver)?;
    out.start_progress("Starting the Graftcode gateway")?;
    let mut process = spawn(&plan)?;
    gateway::wait_until_ready(
        plan.port,
        &mut process.0,
        &plan.log_file(),
        Duration::from_secs(args.startup_timeout.max(1)),
    )?;
    let base = plan.base_url();
    let mcp = format!("{base}/mcp");
    let listed = McpSession::connect(&mcp, None, LIST_TIMEOUT)
        .and_then(|mut session| session.list_tools())
        .with_context(|| format!("the gateway started but {mcp} did not list its tools"))?;
    let tools = receiver.tools();
    if listed.len() != tools.len() {
        out.note(&format!(
            "warning: the gateway lists {} tools but Quinjet generated {}",
            listed.len(),
            tools.len()
        ));
    }
    if let Some(name) = &args.register {
        let path = registry::registry_path()?;
        let mut gateways = registry::load(&path)?;
        let gateway = Gateway {
            name: name.clone(),
            url: mcp.clone(),
            token_env: None,
        };
        if !registry::upsert(&mut gateways, gateway) {
            out.note(&format!(
                "Replaced the `{name}` gateway that was registered before"
            ));
        }
        registry::save(&path, &gateways)?;
    }
    let document = ServeDocument {
        repository: plan.repository.display().to_string(),
        gateway: plan.gateway.display().to_string(),
        mcp,
        vision: format!("{base}/GV"),
        websocket: format!("ws://localhost:{}/ws", plan.port),
        access: if args.allow_writes {
            Access::Write
        } else {
            Access::Read
        },
        published: plan.publish,
        registered: args.register.clone(),
        tools,
        log: plan.log_file().display().to_string(),
    };
    let stop = stop_signal()?;
    out.emit(&document, || render_graft::serving(&document))?;
    loop {
        if let Some(status) = process
            .0
            .try_wait()
            .context("failed to watch the Graftcode gateway")?
        {
            return Ok(if status.success() {
                0
            } else {
                super::EXIT_FAILURE
            });
        }
        if stop.load(Ordering::SeqCst) {
            drop(process);
            return Ok(0);
        }
        thread::sleep(WATCH_INTERVAL);
    }
}

#[cfg(unix)]
fn stop_signal() -> Result<Arc<AtomicBool>> {
    let stop = Arc::new(AtomicBool::new(false));
    let _registered = [
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
    ]
    .into_iter()
    .map(|signal| signal_hook::flag::register(signal, Arc::clone(&stop)))
    .collect::<std::io::Result<Vec<_>>>()
    .context("failed to watch for stop signals")?;
    Ok(stop)
}

#[cfg(not(unix))]
fn stop_signal() -> Result<Arc<AtomicBool>> {
    Ok(Arc::new(AtomicBool::new(false)))
}
