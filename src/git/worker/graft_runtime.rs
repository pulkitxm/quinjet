#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " Gateway calls can wait minutes on a model, so they run on their own lane and"]
#[doc = " never delay a status refresh, a preview or a pull-request read."]
pub(super) fn run_graft_worker(
    repository: &Repository,
    mailbox: &Arc<SharedMailbox>,
    events: &Sender<WorkerEvent>,
) {
    let mut session = Session::new(repository.clone_for_worker());
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
            WorkerCommand::Shutdown => break,
            _ => continue,
        };
        if events.send(event).is_err() {
            break;
        }
    }
}
