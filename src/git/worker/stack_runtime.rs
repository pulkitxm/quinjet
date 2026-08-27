#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn load_stack_member(
    session: &mut Session,
    identity: PullRequestStackMemberIdentity,
    generation: u64,
    pull_request: &PullRequest,
    refresh: bool,
) -> WorkerEvent {
    let repository = pull_request.base_repository.clone();
    WorkerEvent::PullRequestStackMember {
        identity,
        generation,
        result: answer(
            session
                .execute(Command::PullRequestLookup {
                    repositories: vec![repository.clone()],
                    repository: Some(Box::new(repository)),
                    number: pull_request.number,
                    refresh,
                })
                .and_then(Outcome::pull_request),
        ),
    }
}

pub(super) fn load_stack_member_checks(
    session: &mut Session,
    identity: PullRequestStackMemberIdentity,
    generation: u64,
    pull_request: Box<PullRequest>,
    refresh: bool,
) -> WorkerEvent {
    WorkerEvent::PullRequestStackMemberChecks {
        identity,
        generation,
        result: load_checks(session, pull_request, refresh),
    }
}

pub(super) fn load_stack_tip_checks(
    session: &mut Session,
    identity: PullRequestStackMemberIdentity,
    generation: u64,
    pull_request: Box<PullRequest>,
    refresh: bool,
) -> WorkerEvent {
    WorkerEvent::PullRequestStackTipChecks {
        identity,
        generation,
        result: load_checks(session, pull_request, refresh),
    }
}

pub(super) fn load_stack_member_conversation(
    session: &mut Session,
    identity: PullRequestStackMemberIdentity,
    generation: u64,
    pull_request: Box<PullRequest>,
) -> WorkerEvent {
    WorkerEvent::PullRequestStackMemberConversation {
        identity,
        generation,
        result: answer(
            session
                .execute(Command::PullRequestConversation { pull_request })
                .and_then(Outcome::conversation),
        ),
    }
}

pub(super) fn load_stack_member_commits(
    session: &mut Session,
    identity: PullRequestStackMemberIdentity,
    generation: u64,
    pull_request: Box<PullRequest>,
) -> WorkerEvent {
    WorkerEvent::PullRequestStackMemberCommits {
        identity,
        generation,
        result: answer(
            session
                .execute(Command::PullRequestCommits { pull_request })
                .and_then(Outcome::commits),
        ),
    }
}

fn load_checks(
    session: &mut Session,
    pull_request: Box<PullRequest>,
    refresh: bool,
) -> Result<PullRequestChecks, String> {
    answer(
        session
            .execute(Command::PullRequestChecks {
                pull_request,
                refresh,
            })
            .and_then(Outcome::checks),
    )
}
