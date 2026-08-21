use super::*;

impl App {
    pub(super) fn handle_form_modal_key(
        &mut self,
        mut modal: Modal,
        key: KeyEvent,
        now: Instant,
    ) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        match &mut modal {
            Modal::Help {
                selected, hover, ..
            } => match key.code {
                KeyCode::Esc | KeyCode::Char('?' | 'q') | KeyCode::Enter => {}
                KeyCode::Up | KeyCode::Char('k') => {
                    *selected = previous_list_index(*selected, crate::ui::help_shortcut_count());
                    *hover = None;
                    self.modal = Some(modal);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    *selected = next_list_index(*selected, crate::ui::help_shortcut_count());
                    *hover = None;
                    self.modal = Some(modal);
                }
                KeyCode::PageUp => {
                    *selected = selected.saturating_sub(10);
                    *hover = None;
                    self.modal = Some(modal);
                }
                KeyCode::PageDown => {
                    let count = crate::ui::help_shortcut_count();
                    *selected = (*selected + 10).min(count.saturating_sub(1));
                    *hover = None;
                    self.modal = Some(modal);
                }
                KeyCode::Home | KeyCode::Char('g') => {
                    *selected = 0;
                    *hover = None;
                    self.modal = Some(modal);
                }
                KeyCode::End | KeyCode::Char('G') => {
                    *selected = crate::ui::help_shortcut_count().saturating_sub(1);
                    *hover = None;
                    self.modal = Some(modal);
                }
                _ => self.modal = Some(modal),
            },
            Modal::Commit { input, amend } => {
                if key.code == KeyCode::Esc {
                    return effects;
                }
                if key.code == KeyCode::Enter
                    && key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                {
                    let message = input.value.trim().to_owned();
                    if message.is_empty() {
                        self.modal = Some(modal);
                        return effects;
                    }
                    if *amend {
                        self.modal = Some(Modal::Confirm {
                            title: "Amend Commit?".to_owned(),
                            message: "Replace the previous commit with this message?".to_owned(),
                            action: ConfirmAction::Operate(GitOperation::Commit {
                                message,
                                amend: true,
                            }),
                        });
                    } else {
                        self.queue_operation(
                            GitOperation::Commit {
                                message,
                                amend: false,
                            },
                            &mut effects,
                        );
                    }
                    return effects;
                }
                edit_text(input, key, true);
                self.modal = Some(modal);
            }
            Modal::Prompt { input, kind, .. } => {
                if key.code == KeyCode::Esc {
                    if let PromptKind::Filter { previous } = kind {
                        self.filter.clone_from(previous);
                        self.normalize_selection();
                        self.schedule_preview(now);
                    }
                    return effects;
                }
                if key.code == KeyCode::Enter {
                    match kind {
                        PromptKind::Filter { .. } => {
                            self.filter.clone_from(&input.value);
                            self.normalize_selection();
                            self.schedule_preview(now);
                        }
                        PromptKind::CreateBranch { start } => {
                            self.queue_operation(
                                GitOperation::CreateBranch {
                                    name: input.value.trim().to_owned(),
                                    start: start.clone(),
                                },
                                &mut effects,
                            );
                        }
                        PromptKind::RenameBranch { old } => {
                            self.queue_operation(
                                GitOperation::RenameBranch {
                                    old: old.clone(),
                                    new: input.value.trim().to_owned(),
                                },
                                &mut effects,
                            );
                        }
                        PromptKind::StashPush {
                            include_untracked,
                            staged,
                            paths,
                        } => {
                            self.queue_operation(
                                GitOperation::StashPush {
                                    message: input.value.trim().to_owned(),
                                    include_untracked: *include_untracked,
                                    staged: *staged,
                                    paths: paths.clone(),
                                },
                                &mut effects,
                            );
                            self.checked_change_paths.clear();
                        }
                    }
                    return effects;
                }
                edit_text(input, key, false);
                if matches!(kind, PromptKind::Filter { .. }) {
                    self.filter.clone_from(&input.value);
                    self.normalize_selection();
                    self.schedule_preview(now);
                }
                self.modal = Some(modal);
            }
            Modal::Confirm { action, .. } => match key.code {
                KeyCode::Enter | KeyCode::Char('y' | 'Y') => match action {
                    ConfirmAction::Operate(operation) => {
                        self.queue_operation(operation.clone(), &mut effects);
                    }
                    ConfirmAction::OpenPrompt { title, kind } => {
                        self.modal = Some(Modal::Prompt {
                            title: title.clone(),
                            input: TextBuffer::default(),
                            kind: kind.clone(),
                        });
                        return effects;
                    }
                    ConfirmAction::PullRequest {
                        pull_request,
                        operation,
                    } => {
                        self.queue_pull_request_operation(
                            *pull_request.clone(),
                            operation.clone(),
                            &mut effects,
                        );
                    }
                },
                KeyCode::Esc | KeyCode::Char('n' | 'N') => {}
                _ => self.modal = Some(modal),
            },
            Modal::Conflict { change } => match key.code {
                KeyCode::Char('o') => self.queue_operation(
                    GitOperation::ResolveConflict {
                        path: change.path.clone(),
                        choice: ConflictChoice::Ours,
                    },
                    &mut effects,
                ),
                KeyCode::Char('t') => self.queue_operation(
                    GitOperation::ResolveConflict {
                        path: change.path.clone(),
                        choice: ConflictChoice::Theirs,
                    },
                    &mut effects,
                ),
                KeyCode::Char('s') | KeyCode::Enter => self
                    .queue_operation(GitOperation::Stage(vec![change.path.clone()]), &mut effects),
                KeyCode::Esc => {}
                _ => self.modal = Some(modal),
            },
            Modal::Branches {
                items,
                selected,
                query,
                loading,
                ..
            } => {
                if key.code == KeyCode::Esc {
                    return effects;
                }
                let visible = Self::filtered_branches(items, &query.value);
                match key.code {
                    KeyCode::Up => *selected = previous_list_index(*selected, visible.len()),
                    KeyCode::Down => *selected = next_list_index(*selected, visible.len()),
                    KeyCode::Enter if !*loading => {
                        if let Some(branch) =
                            visible.get(*selected).and_then(|index| items.get(*index))
                        {
                            if branch.current {
                                self.show_toast(
                                    format!("Already on {}", branch.name),
                                    ToastLevel::Info,
                                    now,
                                );
                            } else {
                                self.queue_operation(
                                    GitOperation::Checkout(branch.name.clone()),
                                    &mut effects,
                                );
                            }
                        }
                        return effects;
                    }
                    KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.modal = Some(Modal::Prompt {
                            title: "Create Branch".to_owned(),
                            input: TextBuffer::default(),
                            kind: PromptKind::CreateBranch { start: None },
                        });
                        return effects;
                    }
                    KeyCode::F(2) | KeyCode::Char('r')
                        if !*loading
                            && (key.code == KeyCode::F(2)
                                || key.modifiers.contains(KeyModifiers::CONTROL)) =>
                    {
                        if let Some(branch) =
                            visible.get(*selected).and_then(|index| items.get(*index))
                        {
                            self.modal = Some(Modal::Prompt {
                                title: "Rename Local Branch".to_owned(),
                                input: TextBuffer::new(branch.name.clone()),
                                kind: PromptKind::RenameBranch {
                                    old: branch.name.clone(),
                                },
                            });
                        }
                        return effects;
                    }
                    KeyCode::Delete if !*loading => {
                        if let Some(branch) =
                            visible.get(*selected).and_then(|index| items.get(*index))
                        {
                            if branch.current {
                                self.show_toast(
                                    "Cannot delete the current branch".to_owned(),
                                    ToastLevel::Error,
                                    now,
                                );
                            } else {
                                self.modal = Some(Modal::Confirm {
                                    title: "Delete Branch?".to_owned(),
                                    message: format!(
                                        "Delete local branch `{}`? Git will refuse if it is not merged.",
                                        branch.name
                                    ),
                                    action: ConfirmAction::Operate(GitOperation::DeleteBranch(
                                        branch.name.clone(),
                                    )),
                                });
                            }
                            return effects;
                        }
                    }
                    _ => {
                        edit_text(query, key, false);
                        *selected = 0;
                    }
                }
                self.modal = Some(modal);
            }
            Modal::HistoryBranches {
                items,
                selected,
                query,
                loading,
            } => {
                if key.code == KeyCode::Esc {
                    return effects;
                }
                let visible = Self::filtered_history_branches(items, &query.value);
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        *selected = previous_list_index(*selected, visible.len());
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *selected = next_list_index(*selected, visible.len());
                    }
                    KeyCode::Enter if !*loading => {
                        if let Some(branch) = visible
                            .get(*selected)
                            .and_then(|index| items.get(*index))
                            .cloned()
                        {
                            self.select_history_branch(branch, &mut effects);
                        }
                        return effects;
                    }
                    _ => {
                        edit_text(query, key, false);
                        *selected = 0;
                    }
                }
                self.modal = Some(modal);
            }
            Modal::CompareBranches {
                items,
                selected,
                query,
                loading,
            } => {
                if key.code == KeyCode::Esc {
                    return effects;
                }
                let visible = Self::filtered_history_branches(items, &query.value)
                    .into_iter()
                    .filter(|index| items.get(*index).is_some_and(|item| !item.current))
                    .collect::<Vec<_>>();
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        *selected = previous_list_index(*selected, visible.len());
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *selected = next_list_index(*selected, visible.len());
                    }
                    KeyCode::Enter if !*loading => {
                        if let Some(branch) = visible
                            .get(*selected)
                            .and_then(|index| items.get(*index))
                            .cloned()
                        {
                            self.auxiliary_preview = Some(AuxiliaryPreview::Branch(branch));
                            self.set_focus(Focus::Content, &mut effects);
                            self.content_scroll = 0;
                            self.request_preview(&mut effects);
                        }
                        return effects;
                    }
                    _ => {
                        edit_text(query, key, false);
                        *selected = 0;
                    }
                }
                self.modal = Some(modal);
            }
            Modal::Stashes {
                items,
                selected,
                query,
                loading,
            } => {
                if key.code == KeyCode::Esc {
                    return effects;
                }
                let visible = Self::filtered_stashes(items, &query.value);
                let selected_stash = visible
                    .get(*selected)
                    .and_then(|index| items.get(*index))
                    .cloned();
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        *selected = previous_list_index(*selected, visible.len());
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *selected = next_list_index(*selected, visible.len());
                    }
                    KeyCode::Enter if !*loading => {
                        if let Some(stash) = selected_stash {
                            self.auxiliary_preview = Some(AuxiliaryPreview::Stash(stash));
                            self.set_focus(Focus::Content, &mut effects);
                            self.content_scroll = 0;
                            self.request_preview(&mut effects);
                        }
                        return effects;
                    }
                    KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.prompt_stash(false, false);
                        return effects;
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.prompt_stash(true, false);
                        return effects;
                    }
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.prompt_stash(false, true);
                        return effects;
                    }
                    KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::ALT) => {
                        if let Some(stash) = selected_stash {
                            self.queue_operation(
                                GitOperation::StashApply(stash.reference),
                                &mut effects,
                            );
                        }
                        return effects;
                    }
                    KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::ALT) => {
                        if let Some(stash) = selected_stash {
                            self.queue_operation(
                                GitOperation::StashPop(Some(stash.reference)),
                                &mut effects,
                            );
                        }
                        return effects;
                    }
                    KeyCode::Delete if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if !items.is_empty() {
                            self.modal = Some(Modal::Confirm {
                                title: "Drop All Stashes?".to_owned(),
                                message: "Permanently delete every stash? This cannot be undone."
                                    .to_owned(),
                                action: ConfirmAction::Operate(GitOperation::StashClear),
                            });
                        }
                        return effects;
                    }
                    KeyCode::Delete if !*loading => {
                        if let Some(stash) = selected_stash {
                            self.modal = Some(Modal::Confirm {
                                title: "Drop Stash?".to_owned(),
                                message: format!(
                                    "Permanently delete {} — {}?",
                                    stash.reference, stash.message
                                ),
                                action: ConfirmAction::Operate(GitOperation::StashDrop(
                                    stash.reference,
                                )),
                            });
                        }
                        return effects;
                    }
                    _ => {
                        edit_text(query, key, false);
                        *selected = 0;
                    }
                }
                self.modal = Some(modal);
            }
            _ => {}
        }
        effects
    }
}
