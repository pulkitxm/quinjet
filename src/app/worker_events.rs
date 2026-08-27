#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(crate) fn handle_worker_event(
        &mut self,
        event: WorkerEvent,
        now: Instant,
    ) -> Vec<AppEffect> {
        match event {
            event @ (WorkerEvent::PullRequestStackMember { .. }
            | WorkerEvent::PullRequestStackMemberChecks { .. }
            | WorkerEvent::PullRequestStackTipChecks { .. }
            | WorkerEvent::PullRequestStackMemberConversation { .. }
            | WorkerEvent::PullRequestStackMemberCommits { .. }) => {
                self.handle_stack_worker_event(event)
            }
            event @ (WorkerEvent::Status { .. }
            | WorkerEvent::LocalDiffIndex { .. }
            | WorkerEvent::LocalDiffFile { .. }
            | WorkerEvent::PullRequestIndex { .. }
            | WorkerEvent::PullRequestDiff { .. }
            | WorkerEvent::PullRequestDiffBatch { .. }
            | WorkerEvent::PullRequestChecks { .. }
            | WorkerEvent::CheckRunLog { .. }
            | WorkerEvent::PullRequestConversation { .. }
            | WorkerEvent::PullRequestReview { .. }) => {
                self.handle_content_worker_event(event, now)
            }
            event => self.handle_repository_worker_event(event, now),
        }
    }
}
