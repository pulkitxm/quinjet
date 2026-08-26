#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    #[expect(
        clippy::too_many_lines,
        reason = "the exhaustive event match keeps each state transition together"
    )]
    pub(super) fn handle_stack_worker_event(&mut self, event: WorkerEvent) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        match event {
            WorkerEvent::PullRequestStackMember {
                identity,
                generation,
                result,
            } => {
                if self.stack_inspector.selected_identity.as_ref() != Some(&identity)
                    || self.stack_inspector.selected_generation != generation
                {
                    return effects;
                }
                self.stack_inspector.selected_loading = false;
                match result {
                    Ok(snapshot)
                        if snapshot.pull_request.number == identity.number
                            && snapshot
                                .pull_request
                                .base_repository
                                .url
                                .eq_ignore_ascii_case(&identity.repository_url)
                            && snapshot.pull_request.action_state.node_id
                                == identity.pull_request_node_id
                            && self.stack_inspector.selected_locator.as_ref().is_some_and(
                                |locator| {
                                    snapshot.pull_request.base_oid == locator.base_oid
                                        && snapshot.pull_request.head_oid == locator.head_oid
                                },
                            ) =>
                    {
                        self.stack_inspector.selected_pull_request = Some(snapshot.pull_request);
                        self.stack_inspector.selected_from_cache = snapshot.from_cache;
                        self.stack_inspector.selected_error = None;
                    }
                    Ok(_) => {
                        self.stack_inspector.selected_error =
                            Some("GitHub returned a different stack member".to_owned());
                    }
                    Err(error) => self.stack_inspector.selected_error = Some(error),
                }
                self.invalidate_stack_inspector_content_rows();
                if self.stack_inspector.selected_refresh_again {
                    self.stack_inspector.selected_refresh_again = false;
                    self.request_stack_member(true, &mut effects);
                }
            }
            WorkerEvent::PullRequestStackMemberChecks {
                identity,
                generation,
                result,
            } => {
                if self.stack_inspector.selected_identity.as_ref() != Some(&identity)
                    || self.stack_inspector.checks_generation != generation
                {
                    return effects;
                }
                self.stack_inspector.checks_loading = false;
                match result {
                    Ok(checks) => {
                        self.stack_inspector.checks = checks;
                        self.stack_inspector.checks_loaded = true;
                        self.stack_inspector.checks_error = None;
                    }
                    Err(error) => self.stack_inspector.checks_error = Some(error),
                }
                self.invalidate_stack_inspector_content_rows();
                if self.stack_inspector.checks_refresh_again {
                    self.stack_inspector.checks_refresh_again = false;
                    self.request_stack_member_checks(true, &mut effects);
                }
            }
            WorkerEvent::PullRequestStackTipChecks {
                identity,
                generation,
                result,
            } => {
                if self.stack_inspector.tip_identity.as_ref() != Some(&identity)
                    || self.stack_inspector.tip_checks_generation != generation
                {
                    return effects;
                }
                self.stack_inspector.tip_checks_loading = false;
                match result {
                    Ok(checks) => {
                        self.stack_inspector.tip_checks = checks;
                        self.stack_inspector.tip_checks_loaded = true;
                        self.stack_inspector.tip_checks_error = None;
                    }
                    Err(error) => self.stack_inspector.tip_checks_error = Some(error),
                }
                if self.stack_inspector.selected_identity == self.stack_inspector.tip_identity {
                    self.invalidate_stack_inspector_content_rows();
                }
                if self.stack_inspector.tip_checks_refresh_again {
                    self.stack_inspector.tip_checks_refresh_again = false;
                    self.request_stack_tip_checks(true, &mut effects);
                }
            }
            WorkerEvent::PullRequestStackMemberConversation {
                identity,
                generation,
                result,
            } => {
                if self.stack_inspector.selected_identity.as_ref() != Some(&identity)
                    || self.stack_inspector.conversation_generation != generation
                {
                    return effects;
                }
                self.stack_inspector.conversation_loading = false;
                match result {
                    Ok(conversation) => {
                        self.stack_inspector.conversation = conversation;
                        self.stack_inspector.conversation_loaded = true;
                        self.stack_inspector.conversation_error = None;
                    }
                    Err(error) => self.stack_inspector.conversation_error = Some(error),
                }
                self.invalidate_stack_inspector_content_rows();
                if self.stack_inspector.conversation_refresh_again {
                    self.stack_inspector.conversation_refresh_again = false;
                    self.request_stack_member_conversation(true, &mut effects);
                }
            }
            WorkerEvent::PullRequestStackMemberCommits {
                identity,
                generation,
                result,
            } => {
                if self.stack_inspector.selected_identity.as_ref() != Some(&identity)
                    || self.stack_inspector.commits_generation != generation
                {
                    return effects;
                }
                self.stack_inspector.commits_loading = false;
                match result {
                    Ok(commits) => {
                        self.stack_inspector.commits = commits;
                        self.stack_inspector.commits_loaded = true;
                        self.stack_inspector.commits_error = None;
                    }
                    Err(error) => self.stack_inspector.commits_error = Some(error),
                }
                self.invalidate_stack_inspector_content_rows();
            }
            _ => {}
        }
        effects
    }
}
