use super::*;

impl App {
    pub(super) fn open_stashes(&mut self, effects: &mut Vec<AppEffect>) {
        self.stash_generation = self.stash_generation.wrapping_add(1);
        self.modal = Some(Modal::Stashes {
            items: Vec::new(),
            selected: 0,
            query: TextBuffer::default(),
            loading: true,
        });
        effects.push(AppEffect::Git(Box::new(WorkerCommand::LoadStashes {
            generation: self.stash_generation,
        })));
    }

    pub(super) fn open_projects(&mut self, effects: &mut Vec<AppEffect>) {
        self.modal = Some(Modal::Projects {
            groups: self.project_groups.clone(),
            selected: 0,
            query: TextBuffer::default(),
            loading: self.project_groups.is_empty(),
        });
        self.request_recent_projects(effects);
    }

    pub(super) fn apply_current_worktrees(&mut self, groups: &[ProjectGroup]) {
        if let Some(group) = groups
            .iter()
            .find(|group| group.worktrees.iter().any(|tree| tree.current))
        {
            self.worktrees.clone_from(&group.worktrees);
        }
    }

    pub(super) fn request_worktrees(&mut self, effects: &mut Vec<AppEffect>) {
        self.worktree_generation = self.worktree_generation.wrapping_add(1);
        effects.push(AppEffect::Git(Box::new(WorkerCommand::LoadWorktrees {
            generation: self.worktree_generation,
        })));
    }

    pub(super) fn request_recent_projects(&mut self, effects: &mut Vec<AppEffect>) {
        self.project_generation = self.project_generation.wrapping_add(1);
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::LoadRecentProjects {
                generation: self.project_generation,
            },
        )));
    }

    pub(super) fn open_compare_branches(&mut self, effects: &mut Vec<AppEffect>) {
        self.modal = Some(Modal::CompareBranches {
            items: self.history_branches.clone(),
            selected: 0,
            query: TextBuffer::default(),
            loading: self.history_branches_loading,
        });
        if !self.history_branches_loaded && !self.history_branches_loading {
            self.request_history_branches(effects);
        }
    }

    pub(super) fn toggle_stage_selected(&mut self, effects: &mut Vec<AppEffect>) {
        if self.selected_change_section.is_some() {
            return;
        }
        let Some(change) = self.selected_change().cloned() else {
            return;
        };
        match change.area {
            ChangeArea::Unstaged => {
                self.queue_operation(GitOperation::Stage(vec![change.path]), effects);
            }
            ChangeArea::Conflict => self.modal = Some(Modal::Conflict { change }),
            ChangeArea::Staged => {
                self.queue_operation(GitOperation::Unstage(vec![change.path]), effects);
            }
        }
    }

    pub(super) fn unstage_selected(&mut self, effects: &mut Vec<AppEffect>) {
        if self.selected_change_section.is_some() {
            return;
        }
        let Some(change) = self.selected_change().cloned() else {
            return;
        };
        if change.area == ChangeArea::Staged {
            self.queue_operation(GitOperation::Unstage(vec![change.path]), effects);
        }
    }

    pub(super) fn confirm_discard(&mut self) {
        if !self.checked_change_paths.is_empty() {
            self.confirm_discard_checked();
            return;
        }
        if self.selected_change_section.is_some() {
            return;
        }
        let Some(change) = self.selected_change().cloned() else {
            return;
        };
        if change.area == ChangeArea::Conflict {
            self.modal = Some(Modal::Conflict { change });
            return;
        }
        self.modal = Some(Modal::Confirm {
            title: "Discard Change?".to_owned(),
            message: format!(
                "Permanently discard changes to `{}`? This cannot be undone.",
                change.display_path()
            ),
            action: ConfirmAction::Operate(GitOperation::Discard(vec![change])),
        });
    }

    pub(super) fn confirm_remove(&mut self) {
        if !self.checked_change_paths.is_empty() {
            self.confirm_remove_checked();
            return;
        }
        if self.selected_change_section.is_some() {
            return;
        }
        let Some(change) = self.selected_change().cloned() else {
            return;
        };
        self.modal = Some(Modal::Confirm {
            title: "Remove File?".to_owned(),
            message: format!(
                "Delete `{}` from the working tree and the index? This cannot be undone.",
                change.display_path()
            ),
            action: ConfirmAction::Operate(GitOperation::Remove(vec![change.path])),
        });
    }

    pub(super) fn confirm_discard_checked(&mut self) {
        let changes = self.checked_changes();
        if changes.is_empty() {
            return;
        }
        self.modal = Some(Modal::Confirm {
            title: "Revert Checked Files?".to_owned(),
            message: change_list_message(
                "Permanently discard changes to these files? This cannot be undone.",
                &changes,
            ),
            action: ConfirmAction::Operate(GitOperation::Discard(changes)),
        });
    }

    pub(super) fn confirm_remove_checked(&mut self) {
        let changes = self.checked_changes();
        if changes.is_empty() {
            return;
        }
        let mut paths: Vec<PathBuf> = Vec::new();
        for change in &changes {
            if !paths.contains(&change.path) {
                paths.push(change.path.clone());
            }
        }
        self.modal = Some(Modal::Confirm {
            title: "Remove Checked Files?".to_owned(),
            message: change_list_message(
                "Delete these files from the working tree and the index? This cannot be undone.",
                &changes,
            ),
            action: ConfirmAction::Operate(GitOperation::Remove(paths)),
        });
    }

    pub(super) fn confirm_discard_selected_file(&mut self) {
        if self.selected_change_section.is_some() {
            return;
        }
        let Some(change) = self.selected_change().cloned() else {
            return;
        };
        if change.area == ChangeArea::Conflict {
            self.modal = Some(Modal::Conflict { change });
            return;
        }
        self.modal = Some(Modal::Confirm {
            title: "Revert Selected File?".to_owned(),
            message: format!(
                "Permanently discard changes to `{}`? This cannot be undone.",
                change.display_path()
            ),
            action: ConfirmAction::Operate(GitOperation::Discard(vec![change])),
        });
    }

    pub(super) fn confirm_discard_area(
        &mut self,
        area: Option<ChangeArea>,
        title: &str,
        message: &str,
    ) {
        let changes = self
            .status
            .changes
            .iter()
            .filter(|change| change.area != ChangeArea::Conflict)
            .filter(|change| area.is_none_or(|wanted| change.area == wanted))
            .cloned()
            .collect::<Vec<_>>();
        if changes.is_empty() {
            return;
        }
        self.modal = Some(Modal::Confirm {
            title: title.to_owned(),
            message: change_list_message(message, &changes),
            action: ConfirmAction::Operate(GitOperation::Discard(changes)),
        });
    }

    pub(super) fn checked_changes(&self) -> Vec<Change> {
        self.status
            .changes
            .iter()
            .filter(|change| change.area != ChangeArea::Conflict)
            .filter(|change| self.checked_change_paths.contains(&change.path))
            .cloned()
            .collect()
    }

    pub(crate) fn scm_menu_items(&self) -> Vec<ScmMenuItem> {
        let mut items = vec![ScmMenuItem::StageAll, ScmMenuItem::UnstageAll];
        if self.checked_change_paths.is_empty() {
            if self.selected_change_section.is_none() && self.selected_change().is_some() {
                items.push(ScmMenuItem::DiscardSelected);
                items.push(ScmMenuItem::RemoveSelected);
            }
        } else {
            items.push(ScmMenuItem::DiscardChecked);
            items.push(ScmMenuItem::RemoveChecked);
        }
        if self
            .status
            .changes
            .iter()
            .any(|change| change.area == ChangeArea::Unstaged)
        {
            items.push(ScmMenuItem::DiscardUnstaged);
        }
        if self
            .status
            .changes
            .iter()
            .any(|change| change.area != ChangeArea::Conflict)
        {
            items.push(ScmMenuItem::DiscardAll);
        }
        items.extend([
            ScmMenuItem::CompareBranch,
            ScmMenuItem::ManageStashes,
            ScmMenuItem::StashAll,
            ScmMenuItem::StashIncludeUntracked,
            ScmMenuItem::StashStagedOnly,
        ]);
        items
    }

    pub(crate) fn scm_menu_label(&self, item: ScmMenuItem) -> String {
        match item {
            ScmMenuItem::DiscardChecked | ScmMenuItem::RemoveChecked => {
                let mut label = item.label().to_owned();
                label.push_str(" (");
                label.push_str(&self.checked_change_paths.len().to_string());
                label.push(')');
                label
            }
            other => other.label().to_owned(),
        }
    }

    pub(super) fn confirm_cherry_pick(&mut self) {
        let Some(commit) = self.selected_commit() else {
            return;
        };
        self.modal = Some(Modal::Confirm {
            title: "Cherry-pick Commit?".to_owned(),
            message: format!(
                "Apply {} — {} to the current branch?",
                commit.short_id, commit.subject
            ),
            action: ConfirmAction::Operate(GitOperation::CherryPick(commit.id.clone())),
        });
    }

    pub(super) fn confirm_revert(&mut self) {
        let Some(commit) = self.selected_commit() else {
            return;
        };
        self.modal = Some(Modal::Confirm {
            title: "Revert Commit?".to_owned(),
            message: format!(
                "Create a commit that reverts {} — {}?",
                commit.short_id, commit.subject
            ),
            action: ConfirmAction::Operate(GitOperation::Revert(commit.id.clone())),
        });
    }

    pub(super) fn prompt_branch_at_commit(&mut self) {
        let Some(commit) = self.selected_commit() else {
            return;
        };
        self.modal = Some(Modal::Prompt {
            title: format!("Create Branch at {}", commit.short_id),
            input: TextBuffer::default(),
            kind: PromptKind::CreateBranch {
                start: Some(commit.id.clone()),
            },
        });
    }

    pub(super) fn open_branches(&mut self, effects: &mut Vec<AppEffect>) {
        self.branch_generation = self.branch_generation.wrapping_add(1);
        let generation = self.branch_generation;
        self.modal = Some(Modal::Branches {
            items: Vec::new(),
            selected: 0,
            query: TextBuffer::default(),
            loading: true,
        });
        effects.push(AppEffect::Git(Box::new(WorkerCommand::LoadBranches {
            generation,
        })));
    }

    pub(super) fn open_history_branches(&mut self, effects: &mut Vec<AppEffect>) {
        self.modal = Some(Modal::HistoryBranches {
            items: self.history_branches.clone(),
            selected: self
                .history_branches
                .iter()
                .position(|branch| {
                    self.history_branch
                        .as_ref()
                        .map_or(branch.current, |selected| {
                            selected.reference == branch.reference
                        })
                })
                .unwrap_or_default(),
            query: TextBuffer::default(),
            loading: self.history_branches_loading,
        });
        if !self.history_branches_loaded && !self.history_branches_loading {
            self.request_history_branches(effects);
        }
    }

    pub(super) fn request_history_branches(&mut self, effects: &mut Vec<AppEffect>) {
        self.history_branch_generation = self.history_branch_generation.wrapping_add(1);
        self.history_branches_loading = true;
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::LoadHistoryBranches {
                generation: self.history_branch_generation,
            },
        )));
    }

    pub(super) fn select_history_branch(
        &mut self,
        branch: HistoryBranch,
        effects: &mut Vec<AppEffect>,
    ) {
        self.history_branch = (!branch.current).then_some(branch);
        self.history.clear();
        self.history_cursor = 0;
        self.reset_sidebar_scroll();
        self.history_complete = false;
        self.history_refresh_again = false;
        self.history_generation = self.history_generation.wrapping_add(1);
        self.history_loading = false;
        self.request_history(true, effects);
    }

    pub(super) fn open_pull_request_repositories(&mut self, effects: &mut Vec<AppEffect>) {
        let selected = self
            .pull_request_repository
            .as_ref()
            .and_then(|selected| {
                self.github_repositories
                    .iter()
                    .position(|repository| repository.url == selected.url)
            })
            .unwrap_or_default();
        let loading = self.github_repositories.is_empty();
        self.modal = Some(Modal::PullRequestRepositories {
            items: self.github_repositories.clone(),
            selected,
            query: TextBuffer::default(),
            loading,
        });
        if loading {
            self.repository_generation = self.repository_generation.wrapping_add(1);
            effects.push(AppEffect::Git(Box::new(
                WorkerCommand::LoadGitHubRepositories {
                    generation: self.repository_generation,
                    refresh: false,
                },
            )));
        }
    }

    pub(super) fn handle_pull_request_lookup_key(
        &mut self,
        key: KeyEvent,
        now: Instant,
    ) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        match key.code {
            KeyCode::Char('z') if key.modifiers == KeyModifiers::NONE => {
                self.toggle_sidebar(&mut effects);
            }
            KeyCode::Char('m') if key.modifiers == KeyModifiers::NONE => {
                effects.push(self.toggle_mouse_capture(now));
            }
            KeyCode::Esc => self.pull_request_lookup_active = false,
            KeyCode::Char('o') => {
                self.pull_request_lookup_active = false;
                self.open_pull_request_repositories(&mut effects);
            }
            KeyCode::Enter => {
                let value = self.pull_request_lookup.value.trim();
                match value.parse::<u64>() {
                    Ok(number) if number > 0 => {
                        self.pull_request_lookup_active = false;
                        self.request_pull_request_lookup(number, false, false, &mut effects);
                    }
                    _ => self.show_toast(
                        "Enter a positive numeric pull-request number".to_owned(),
                        ToastLevel::Error,
                        now,
                    ),
                }
            }
            KeyCode::Char(character)
                if character.is_ascii_digit()
                    && self.pull_request_lookup.value.len() < MAX_PULL_REQUEST_NUMBER_DIGITS
                    && !key.modifiers.intersects(
                        KeyModifiers::CONTROL
                            | KeyModifiers::ALT
                            | KeyModifiers::SUPER
                            | KeyModifiers::META
                            | KeyModifiers::HYPER,
                    ) =>
            {
                self.pull_request_lookup.insert(character);
            }
            KeyCode::Backspace => self.pull_request_lookup.backspace(),
            KeyCode::Delete => self.pull_request_lookup.delete(),
            KeyCode::Left => self.pull_request_lookup.move_left(),
            KeyCode::Right => self.pull_request_lookup.move_right(),
            KeyCode::Home => self.pull_request_lookup.home(),
            KeyCode::End => self.pull_request_lookup.end(),
            _ => {}
        }
        effects
    }
}
