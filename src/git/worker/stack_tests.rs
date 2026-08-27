use super::*;
use crate::git::github::GitHubRepository;

pub(super) fn stack_lookup(generation: u64) -> WorkerCommand {
    WorkerCommand::LoadPullRequestStack {
        generation,
        pull_request: Box::default(),
        refresh: false,
    }
}

pub(super) fn stack_prepare(generation: u64) -> WorkerCommand {
    WorkerCommand::PreparePullRequestStack {
        generation,
        stack: Box::new(PullRequestStack {
            node_id: String::new(),
            number: 1,
            base_ref: "main".to_owned(),
            size: 0,
            selected_position: 1,
            members: Vec::new(),
            truncated: false,
            repository: GitHubRepository::default(),
        }),
        from: 1,
        to: 1,
    }
}

fn stack_identity(generation: u64) -> PullRequestStackMemberIdentity {
    PullRequestStackMemberIdentity {
        stack_node_id: "stack".to_owned(),
        entry_id: format!("entry-{generation}"),
        pull_request_node_id: format!("pull-request-{generation}"),
        repository_url: "https://github.com/acme/widget".to_owned(),
        number: generation,
    }
}

pub(super) fn stack_member(generation: u64) -> WorkerCommand {
    WorkerCommand::LoadPullRequestStackMember {
        identity: stack_identity(generation),
        generation,
        pull_request: Box::default(),
        refresh: false,
    }
}

pub(super) fn stack_member_checks(generation: u64) -> WorkerCommand {
    WorkerCommand::LoadPullRequestStackMemberChecks {
        identity: stack_identity(generation),
        generation,
        pull_request: Box::default(),
        refresh: false,
    }
}

pub(super) fn stack_tip_checks(generation: u64) -> WorkerCommand {
    WorkerCommand::LoadPullRequestStackTipChecks {
        identity: stack_identity(generation),
        generation,
        pull_request: Box::default(),
        refresh: false,
    }
}

pub(super) fn stack_member_conversation(generation: u64) -> WorkerCommand {
    WorkerCommand::LoadPullRequestStackMemberConversation {
        identity: stack_identity(generation),
        generation,
        pull_request: Box::default(),
    }
}

pub(super) fn stack_member_commits(generation: u64) -> WorkerCommand {
    WorkerCommand::LoadPullRequestStackMemberCommits {
        identity: stack_identity(generation),
        generation,
        pull_request: Box::default(),
    }
}
