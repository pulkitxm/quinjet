#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(super) fn queue_operation(
        &mut self,
        operation: GitOperation,
        effects: &mut Vec<AppEffect>,
    ) {
        if self.busy.is_some() {
            return;
        }
        self.operation_id = self.operation_id.wrapping_add(1);
        self.busy = Some(operation.label().to_owned());
        self.operation_frame = 0;
        match &operation {
            GitOperation::Remove(paths) => {
                self.checked_change_paths
                    .retain(|path| !paths.contains(path));
            }
            GitOperation::Discard(changes) => {
                self.checked_change_paths
                    .retain(|path| !changes.iter().any(|change| &change.path == path));
            }
            _ => {}
        }
        effects.push(AppEffect::Git(Box::new(WorkerCommand::Operate {
            id: self.operation_id,
            operation,
        })));
    }

    pub(super) fn queue_pull_request_operation(
        &mut self,
        pull_request: PullRequest,
        operation: PullRequestOperation,
        effects: &mut Vec<AppEffect>,
    ) {
        if self.busy.is_some() {
            return;
        }
        if let PullRequestOperation::Merge { method, .. } = &operation {
            self.preferred_merge_method = *method;
        }
        self.operation_id = self.operation_id.wrapping_add(1);
        self.busy = Some(operation.label().to_owned());
        self.operation_frame = 0;
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::OperatePullRequest {
                id: self.operation_id,
                pull_request: Box::new(pull_request),
                operation,
            },
        )));
    }

    pub(super) fn refresh_loaded_pull_request(&mut self, effects: &mut Vec<AppEffect>) {
        let Some(number) = self.pull_request_exact_number.or_else(|| {
            self.pull_request
                .as_ref()
                .map(|pull_request| pull_request.number)
        }) else {
            return;
        };
        self.request_pull_request_lookup(number, true, true, effects);
    }

    pub(crate) fn pr_primary_action(&self) -> Option<PrPrimaryAction> {
        let pull_request = self.selected_pull_request()?;
        Some(match pull_request.state.as_str() {
            "MERGED" => PrPrimaryAction::OpenInBrowser,
            "CLOSED" => PrPrimaryAction::Reopen,
            _ => PrPrimaryAction::Merge(self.preferred_merge_method),
        })
    }

    pub(crate) fn pr_menu_items(&self) -> Vec<PrMenuItem> {
        let Some(pull_request) = self.selected_pull_request() else {
            return Vec::new();
        };
        match pull_request.state.as_str() {
            "MERGED" => Vec::new(),
            "CLOSED" => vec![PrMenuItem::OpenInBrowser],
            _ => {
                let mut items = PullRequestMergeMethod::ALL
                    .into_iter()
                    .filter(|method| *method != self.preferred_merge_method)
                    .map(PrMenuItem::Merge)
                    .collect::<Vec<_>>();
                items.push(PrMenuItem::Close);
                items.push(PrMenuItem::OpenInBrowser);
                items
            }
        }
    }

    pub(super) fn handle_pr_primary(
        &mut self,
        action: PrPrimaryAction,
        effects: &mut Vec<AppEffect>,
    ) {
        match action {
            PrPrimaryAction::OpenInBrowser => self.open_selected_pull_request_in_browser(effects),
            PrPrimaryAction::Merge(method) => {
                self.confirm_pull_request_operation(PullRequestOperation::Merge {
                    method,
                    delete_branch: false,
                });
            }
            PrPrimaryAction::Reopen => {
                self.confirm_pull_request_operation(PullRequestOperation::Reopen);
            }
        }
    }

    pub(super) fn handle_pr_menu_item(&mut self, item: PrMenuItem, effects: &mut Vec<AppEffect>) {
        match item {
            PrMenuItem::OpenInBrowser => self.open_selected_pull_request_in_browser(effects),
            PrMenuItem::Merge(method) => {
                self.preferred_merge_method = method;
                self.confirm_pull_request_operation(PullRequestOperation::Merge {
                    method,
                    delete_branch: false,
                });
            }
            PrMenuItem::Close => {
                self.confirm_pull_request_operation(PullRequestOperation::Close);
            }
        }
    }

    pub(super) fn confirm_pull_request_operation(&mut self, operation: PullRequestOperation) {
        let Some(pull_request) = self.selected_pull_request().cloned() else {
            return;
        };
        self.modal = Some(Modal::Confirm {
            title: operation.confirm_title(),
            message: operation.confirm_message(&pull_request),
            action: ConfirmAction::PullRequest {
                pull_request: Box::new(pull_request),
                operation,
            },
        });
    }
}
