use super::*;

pub(super) fn identity(command: &WorkerCommand) -> (&'static str, u64) {
    match command {
        WorkerCommand::Refresh { generation } => ("refresh", *generation),
        WorkerCommand::PrepareLocalDiff { generation, .. } => ("local-diff", *generation),
        WorkerCommand::LoadLocalDiffFile { generation, .. } => ("local-file", *generation),
        WorkerCommand::LoadHistory { generation, .. } => ("history", *generation),
        WorkerCommand::LoadGitHubRepositories { generation, .. } => ("repositories", *generation),
        WorkerCommand::LoadLocalGitHubRepository => ("local-repository", 0),
        WorkerCommand::LookupPullRequest { generation, .. } => ("lookup", *generation),
        WorkerCommand::LoadPullRequestStack { generation, .. } => ("stack", *generation),
        WorkerCommand::LoadPullRequestStackMember { generation, .. } => {
            ("stack-member", *generation)
        }
        WorkerCommand::LoadPullRequestStackMemberChecks { generation, .. } => {
            ("stack-member-checks", *generation)
        }
        WorkerCommand::LoadPullRequestStackTipChecks { generation, .. } => {
            ("stack-tip-checks", *generation)
        }
        WorkerCommand::LoadPullRequestStackMemberConversation { generation, .. } => {
            ("stack-member-conversation", *generation)
        }
        WorkerCommand::LoadPullRequestStackMemberCommits { generation, .. } => {
            ("stack-member-commits", *generation)
        }
        WorkerCommand::PreparePullRequest { generation, .. } => ("prepare-pr", *generation),
        WorkerCommand::PreparePullRequestStack { generation, .. } => ("prepare-stack", *generation),
        WorkerCommand::LoadPullRequestFile { generation, .. } => ("pr-file", *generation),
        WorkerCommand::LoadPullRequestFileBatch {
            workspace_generation,
            ..
        } => ("pr-batch", *workspace_generation),
        WorkerCommand::LoadPullRequestChecks { generation, .. } => ("checks", *generation),
        WorkerCommand::LoadPullRequestConversation { generation, .. } => {
            ("conversation", *generation)
        }
        WorkerCommand::LoadPullRequestReview { generation, .. } => ("review", *generation),
        WorkerCommand::OperatePullRequestReview { generation, .. } => {
            ("operate-review", *generation)
        }
        WorkerCommand::LoadCheckRunLog { generation, .. } => ("check-log", *generation),
        WorkerCommand::PrefetchCheckRunLogs { generation, .. } => ("warm", *generation),
        WorkerCommand::PrefetchPullRequestStackMembers { generation, .. } => {
            ("stack-warm", *generation)
        }
        WorkerCommand::LoadAccounts { generation } => ("accounts", *generation),
        WorkerCommand::LoadBranches { generation } => ("branches", *generation),
        WorkerCommand::LoadHistoryBranches { generation } => ("history-branches", *generation),
        WorkerCommand::LoadStashes { generation } => ("stashes", *generation),
        WorkerCommand::LoadWorktrees { generation } => ("worktrees", *generation),
        WorkerCommand::LoadRecentProjects { generation } => ("projects", *generation),
        WorkerCommand::Operate { id, .. } => ("operate", *id),
        WorkerCommand::OperatePullRequest { id, .. } => ("operate-pr", *id),
        WorkerCommand::Search { generation, .. } => ("search", *generation),
        WorkerCommand::LoadPullRequestCommits { generation, .. } => ("commits", *generation),
        WorkerCommand::Shutdown => ("shutdown", 0),
    }
}
