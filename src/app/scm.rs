#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    #[expect(
        clippy::unreachable,
        reason = "the branch is impossible for the states that reach it"
    )]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the caller has no use for the value afterwards"
    )]
    pub(super) fn handle_scm_action(&mut self, action: ScmAction, effects: &mut Vec<AppEffect>) {
        match action {
            ScmAction::Stage(index) | ScmAction::Unstage(index) | ScmAction::Resolve(index) => {
                let Some(change) = self.status.changes.get(index).cloned() else {
                    return;
                };
                self.auxiliary_preview = None;
                if let Some(cursor) = self
                    .visible_change_indices()
                    .iter()
                    .position(|visible| *visible == index)
                {
                    self.selected_change_section = None;
                    self.change_cursor = cursor;
                }
                match action {
                    ScmAction::Stage(_) => {
                        self.queue_operation(GitOperation::Stage(vec![change.path]), effects);
                    }
                    ScmAction::Unstage(_) => {
                        self.queue_operation(GitOperation::Unstage(vec![change.path]), effects);
                    }
                    ScmAction::Resolve(_) => self.modal = Some(Modal::Conflict { change }),
                    _ => unreachable!(),
                }
            }
            ScmAction::StageSection(section) | ScmAction::UnstageSection(section) => {
                let paths = self
                    .status
                    .changes
                    .iter()
                    .filter(|change| section.matches(change))
                    .map(|change| change.path.clone())
                    .collect::<Vec<_>>();
                if paths.is_empty() {
                    return;
                }
                match action {
                    ScmAction::StageSection(_) => {
                        self.queue_operation(GitOperation::Stage(paths), effects);
                    }
                    ScmAction::UnstageSection(_) => {
                        self.queue_operation(GitOperation::Unstage(paths), effects);
                    }
                    _ => unreachable!(),
                }
            }
            ScmAction::ToggleCheck(index) => {
                let Some(change) = self.status.changes.get(index) else {
                    return;
                };
                if self.checked_change_paths.contains(&change.path) {
                    self.checked_change_paths
                        .retain(|path| path != &change.path);
                } else {
                    self.checked_change_paths.extend([change.path.clone()]);
                }
            }
            ScmAction::ToggleCheckSection(section) => self.toggle_section_check(section),
            ScmAction::RevertChecked => {
                self.scm_menu_open = false;
                self.confirm_discard_checked();
            }
            ScmAction::Primary => {
                self.scm_menu_open = false;
                if self.checked_change_paths.is_empty() {
                    self.modal = Some(Modal::Commit {
                        input: TextBuffer::default(),
                        amend: false,
                    });
                } else {
                    self.confirm_stash_selected();
                }
            }
            ScmAction::ToggleMenu => {
                self.scm_menu_open = !self.scm_menu_open;
                if self.scm_menu_open {
                    self.scm_menu_selected = 0;
                }
            }
            ScmAction::Menu(item) => {
                self.scm_menu_open = false;
                self.handle_scm_menu_item(item, effects);
            }
            ScmAction::PrPrimary => {
                self.pr_menu_open = false;
                if let Some(action) = self.pr_primary_action() {
                    self.handle_pr_primary(action, effects);
                }
            }
            ScmAction::PrToggleMenu => {
                let items = self.pr_menu_items();
                if items.is_empty() {
                    self.pr_menu_open = false;
                    return;
                }
                self.pr_menu_open = !self.pr_menu_open;
                if self.pr_menu_open {
                    self.pr_menu_selected = 0;
                }
            }
            ScmAction::PrMenu(item) => {
                self.pr_menu_open = false;
                self.handle_pr_menu_item(item, effects);
            }
            ScmAction::JumpToBottom => {
                self.set_focus(Focus::Content, effects);
                self.content_scroll = usize::MAX;
            }
        }
    }

    pub(super) fn handle_scm_menu_item(&mut self, item: ScmMenuItem, effects: &mut Vec<AppEffect>) {
        match item {
            ScmMenuItem::StageAll => {
                self.modal = Some(Modal::Confirm {
                    title: "Stage All?".to_owned(),
                    message: "Stage every change in the working tree?".to_owned(),
                    action: ConfirmAction::Operate(GitOperation::StageAll),
                });
            }
            ScmMenuItem::UnstageAll => {
                self.modal = Some(Modal::Confirm {
                    title: "Unstage All?".to_owned(),
                    message: "Unstage every staged change?".to_owned(),
                    action: ConfirmAction::Operate(GitOperation::UnstageAll),
                });
            }
            ScmMenuItem::DiscardChecked => self.confirm_discard_checked(),
            ScmMenuItem::RemoveChecked => self.confirm_remove_checked(),
            ScmMenuItem::DiscardSelected => self.confirm_discard_selected_file(),
            ScmMenuItem::RemoveSelected => self.confirm_remove(),
            ScmMenuItem::DiscardUnstaged => self.confirm_discard_area(
                Some(ChangeArea::Unstaged),
                "Revert Unstaged Changes?",
                "Permanently discard every unstaged change? This cannot be undone.",
            ),
            ScmMenuItem::DiscardAll => self.confirm_discard_area(
                None,
                "Revert All Changes?",
                "Permanently discard every change in the working tree and the index? This cannot be undone.",
            ),
            ScmMenuItem::CompareBranch => self.open_compare_branches(effects),
            ScmMenuItem::ManageStashes => self.open_stashes(effects),
            ScmMenuItem::StashAll => {
                self.modal = Some(Modal::Confirm {
                    title: "Stash All Changes?".to_owned(),
                    message: "Stash tracked working-tree changes?".to_owned(),
                    action: ConfirmAction::OpenPrompt {
                        title: "Stash Changes".to_owned(),
                        kind: PromptKind::StashPush {
                            include_untracked: false,
                            staged: false,
                            paths: Vec::new(),
                        },
                    },
                });
            }
            ScmMenuItem::StashIncludeUntracked => {
                self.modal = Some(Modal::Confirm {
                    title: "Stash Including Untracked?".to_owned(),
                    message: "Stash tracked and untracked working-tree changes?".to_owned(),
                    action: ConfirmAction::OpenPrompt {
                        title: "Stash Changes Including Untracked".to_owned(),
                        kind: PromptKind::StashPush {
                            include_untracked: true,
                            staged: false,
                            paths: Vec::new(),
                        },
                    },
                });
            }
            ScmMenuItem::StashStagedOnly => {
                self.modal = Some(Modal::Confirm {
                    title: "Stash Staged Only?".to_owned(),
                    message: "Stash only staged changes and leave the rest in place?".to_owned(),
                    action: ConfirmAction::OpenPrompt {
                        title: "Stash Staged Changes".to_owned(),
                        kind: PromptKind::StashPush {
                            include_untracked: false,
                            staged: true,
                            paths: Vec::new(),
                        },
                    },
                });
            }
        }
    }

    pub(super) fn confirm_stash_selected(&mut self) {
        let mut paths = self
            .checked_change_paths
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        if paths.is_empty() {
            return;
        }
        paths.sort();
        let include_untracked = self.status.changes.iter().any(|change| {
            paths.iter().any(|path| path == &change.path)
                && change.status == ChangeStatus::Untracked
        });
        let mut message = String::from("Stash these files?");
        for path in &paths {
            let label = self
                .status
                .changes
                .iter()
                .find(|change| &change.path == path)
                .map_or_else(|| path.display().to_string(), Change::display_path);
            message.push_str("\n  ");
            message.push_str(&label);
        }
        self.modal = Some(Modal::Confirm {
            title: "Stash Selected?".to_owned(),
            message,
            action: ConfirmAction::OpenPrompt {
                title: "Stash Selected Changes".to_owned(),
                kind: PromptKind::StashPush {
                    include_untracked,
                    staged: false,
                    paths,
                },
            },
        });
    }

    pub(super) fn prompt_stash(&mut self, include_untracked: bool, staged: bool) {
        let title = if staged {
            "Stash Staged Changes"
        } else if include_untracked {
            "Stash Changes Including Untracked"
        } else {
            "Stash Changes"
        };
        self.modal = Some(Modal::Prompt {
            title: title.to_owned(),
            input: TextBuffer::default(),
            kind: PromptKind::StashPush {
                include_untracked,
                staged,
                paths: Vec::new(),
            },
        });
    }

    pub(super) fn toggle_section_check(&mut self, section: ChangeSection) {
        let paths = self
            .status
            .changes
            .iter()
            .filter(|change| section.matches(change))
            .map(|change| change.path.clone())
            .collect::<Vec<_>>();
        if paths.is_empty() {
            return;
        }
        if paths
            .iter()
            .all(|path| self.checked_change_paths.contains(path))
        {
            self.checked_change_paths
                .retain(|path| !paths.contains(path));
        } else {
            self.checked_change_paths.extend(paths);
        }
    }

    pub(crate) fn section_check_label(&self, section: ChangeSection) -> &'static str {
        let mut total = 0_usize;
        let mut checked = 0_usize;
        for change in self
            .status
            .changes
            .iter()
            .filter(|change| section.matches(change))
        {
            total = total.saturating_add(1);
            if self.checked_change_paths.contains(&change.path) {
                checked = checked.saturating_add(1);
            }
        }
        if total == 0 || checked == 0 {
            "[ ]"
        } else if checked == total {
            "[x]"
        } else {
            "[-]"
        }
    }

    pub(crate) fn checked_change_count(&self) -> usize {
        self.checked_change_paths.len()
    }

    pub(super) fn toggle_checked_selected(&mut self) {
        if self.selected_change_section.is_some() {
            return;
        }
        let Some(path) = self.selected_change().map(|change| change.path.clone()) else {
            return;
        };
        if self.checked_change_paths.contains(&path) {
            self.checked_change_paths.retain(|checked| checked != &path);
        } else {
            self.checked_change_paths.extend([path]);
        }
    }

    pub(crate) fn primary_is_stash(&self) -> bool {
        !self.checked_change_paths.is_empty()
    }
}
