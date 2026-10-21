#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl Mailbox {
    pub(super) fn push(&mut self, command: WorkerCommand) {
        match command {
            command @ (WorkerCommand::Operate { .. }
            | WorkerCommand::OperatePullRequest { .. }
            | WorkerCommand::OperatePullRequestReview { .. }) => {
                self.operations.push_back(command);
            }
            command @ (WorkerCommand::LoadAccounts { .. }
            | WorkerCommand::LoadBranches { .. }
            | WorkerCommand::LoadHistoryBranches { .. }
            | WorkerCommand::LoadStashes { .. }) => self.branches = Some(command),
            command @ (WorkerCommand::LoadWorktrees { .. }
            | WorkerCommand::LoadRecentProjects { .. }) => self.projects = Some(command),
            command @ WorkerCommand::Refresh { .. } => self.refresh = Some(command),
            command @ (WorkerCommand::PrepareLocalDiff { .. }
            | WorkerCommand::LoadLocalDiffFile { .. }
            | WorkerCommand::PreparePullRequest { .. }
            | WorkerCommand::PreparePullRequestStack { .. }
            | WorkerCommand::LoadPullRequestFile { .. }) => {
                self.preview = Some(command);
            }
            command @ WorkerCommand::LoadPullRequestFileBatch { .. } => {
                self.prefetch = Some(command);
            }
            command @ WorkerCommand::LoadHistory { .. } => self.history = Some(command),
            command @ (WorkerCommand::LoadGitHubRepositories { .. }
            | WorkerCommand::LoadLocalGitHubRepository) => {
                self.repositories = Some(command);
            }
            command @ (WorkerCommand::LookupPullRequest { .. }
            | WorkerCommand::LoadPullRequestStack { .. }) => {
                self.pull_request = Some(command);
            }
            command @ WorkerCommand::LoadPullRequestStackMember { .. } => {
                self.stack_member = Some(command);
            }
            command @ WorkerCommand::LoadPullRequestChecks { .. } => self.checks = Some(command),
            command @ WorkerCommand::LoadPullRequestStackMemberChecks { .. } => {
                self.stack_member_checks = Some(command);
            }
            command @ WorkerCommand::LoadPullRequestStackTipChecks { .. } => {
                self.stack_tip_checks = Some(command);
            }
            command @ WorkerCommand::LoadPullRequestConversation { .. } => {
                self.conversation = Some(command);
            }
            command @ WorkerCommand::LoadPullRequestStackMemberConversation { .. } => {
                self.stack_member_conversation = Some(command);
            }
            command @ (WorkerCommand::LoadPullRequestCommits { .. }
            | WorkerCommand::LoadPullRequestStackMemberCommits { .. }) => {
                self.stack_member_commits = Some(command);
            }
            command @ WorkerCommand::LoadPullRequestReview { .. } => self.review = Some(command),
            command @ WorkerCommand::LoadCheckRunLog { .. } => self.check_log = Some(command),
            command @ WorkerCommand::PrefetchCheckRunLogs { .. } => {
                self.warm = Some(command);
            }
            command @ WorkerCommand::PrefetchPullRequestStackMembers { .. } => {
                self.stack_warm = Some(command);
            }
            command @ WorkerCommand::Search { .. } => self.search = Some(command),
            WorkerCommand::Shutdown => self.shutdown = true,
        }
    }

    pub(super) fn pop(&mut self) -> Option<WorkerCommand> {
        self.operations
            .pop_front()
            .or_else(|| self.branches.take())
            .or_else(|| self.projects.take())
            .or_else(|| self.preview.take())
            .or_else(|| self.search.take())
            .or_else(|| self.repositories.take())
            .or_else(|| self.pull_request.take())
            .or_else(|| self.stack_member.take())
            .or_else(|| self.refresh.take())
            .or_else(|| self.check_log.take())
            .or_else(|| self.checks.take())
            .or_else(|| self.stack_tip_checks.take())
            .or_else(|| self.stack_member_checks.take())
            .or_else(|| self.conversation.take())
            .or_else(|| self.stack_member_conversation.take())
            .or_else(|| self.stack_member_commits.take())
            .or_else(|| self.review.take())
            .or_else(|| self.history.take())
            .or_else(|| self.prefetch.take())
            .or_else(|| self.warm.take())
            .or_else(|| self.stack_warm.take())
    }
}
