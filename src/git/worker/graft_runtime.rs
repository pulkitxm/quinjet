#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;
use crate::cli::ServiceDocument;
use crate::graft::service::{Lease, ServicePaths};

#[doc = " Gateway calls can wait minutes on a model, so they run on their own lane and"]
#[doc = " never delay a status refresh, a preview or a pull-request read."]
pub(super) fn run_graft_worker(
    repository: &Repository,
    mailbox: &Arc<SharedMailbox>,
    events: &Sender<WorkerEvent>,
) {
    let mut session = Session::new(repository.clone_for_worker());
    let mut lease: Option<Lease> = None;
    while let Some(command) = next_command(mailbox) {
        let event = match command {
            WorkerCommand::LoadGraftTools { generation } => WorkerEvent::GraftTools {
                generation,
                result: answer(
                    crate::graft::registry::registry_path()
                        .and_then(|path| crate::graft::registry::load(&path))
                        .and_then(|gateways| session.execute(Command::GraftTools(gateways)))
                        .and_then(Outcome::graft_tools),
                ),
            },
            WorkerCommand::CallGraft { generation, call } => WorkerEvent::GraftReply {
                generation,
                result: answer(
                    session
                        .execute(Command::GraftCall(call))
                        .and_then(Outcome::graft_reply),
                ),
            },
            WorkerCommand::EnsureGateway => WorkerEvent::GatewayReady {
                result: answer(shared_gateway(false, &mut lease).map(Box::new)),
            },
            WorkerCommand::RunAi { generation, task } => WorkerEvent::AiAnswer {
                generation,
                result: answer(crate::cli::ai::run_task(repository, &task)),
            },
            WorkerCommand::ConnectAgents { generation } => WorkerEvent::AgentsConnected {
                generation,
                result: answer(connect_agents(&mut lease)),
            },
            WorkerCommand::Shutdown => break,
            _ => continue,
        };
        if events.send(event).is_err() {
            break;
        }
    }
}

fn shared_gateway(install: bool, lease: &mut Option<Lease>) -> anyhow::Result<ServiceDocument> {
    let state = crate::cli::ensure_shared_gateway(None, install, &mut |_| {})?;
    let paths = ServicePaths::locate()?;
    if lease.is_none() {
        *lease = Some(paths.lease()?);
    }
    Ok(ServiceDocument::from_state(&paths, Some(&state)))
}

fn connect_agents(lease: &mut Option<Lease>) -> anyhow::Result<String> {
    let connections = crate::cli::connect_agents()?;
    let text = crate::cli::render_connections(&connections);
    let gateway = match shared_gateway(true, lease) {
        Ok(service) => format!(
            "The Graftcode gateway is ready{}. Agents start `quinjet mcp` by themselves from now on.",
            service
                .port
                .map(|port| format!(" on port {port}"))
                .unwrap_or_default()
        ),
        Err(error) => format!("The Graftcode gateway could not start: {error:#}"),
    };
    Ok(format!("{text}\n{gateway}\n"))
}
