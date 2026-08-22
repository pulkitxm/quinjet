#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    #[expect(
        clippy::too_many_lines,
        reason = "the exhaustive modal match keeps each state transition together"
    )]
    pub(super) fn handle_picker_modal_key(
        &mut self,
        mut modal: Modal,
        key: KeyEvent,
        now: Instant,
    ) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        match &mut modal {
            Modal::Projects {
                groups,
                selected,
                query,
                collapsed,
                loading,
                opening,
                mode,
            } => {
                if key.code == KeyCode::Esc {
                    return effects;
                }
                if matches!(key.code, KeyCode::Char('e' | 'E'))
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    Self::toggle_all_project_groups(groups, collapsed);
                    *selected = 0;
                    self.modal = Some(modal);
                    return effects;
                }
                if key.code == KeyCode::Tab
                    && let Some(context) = self.ssh_context.as_ref()
                {
                    let selected = context
                        .machines
                        .iter()
                        .position(|machine| machine.target == context.current)
                        .unwrap_or_default();
                    self.modal = Some(Modal::SshMachines {
                        items: context.machines.clone(),
                        selected,
                        current: context.current.clone(),
                        parent: Box::new(modal.clone()),
                    });
                    return effects;
                }
                let visible = Self::filtered_project_rows(groups, &query.value, collapsed);
                let selected_tree = visible
                    .get(*selected)
                    .and_then(|(group_index, tree_index)| {
                        groups
                            .get(*group_index)
                            .and_then(|group| group.worktrees.get(*tree_index))
                            .cloned()
                    });
                let selected_group = visible
                    .get(*selected)
                    .and_then(|(group_index, _)| groups.get(*group_index).cloned());
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        *selected = previous_list_index(*selected, visible.len());
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *selected = next_list_index(*selected, visible.len());
                    }
                    KeyCode::Enter if !*loading && opening.is_none() => {
                        if let Some(tree) = selected_tree.filter(|tree| !tree.current) {
                            *opening = Some(tree.path.clone());
                            effects.push(match mode {
                                ProjectOpenMode::Initial | ProjectOpenMode::CurrentTab => {
                                    AppEffect::SwitchRepository(tree.path)
                                }
                                ProjectOpenMode::NewTab => AppEffect::OpenRepositoryTab(tree.path),
                            });
                            self.modal = Some(modal);
                        }
                        return effects;
                    }
                    KeyCode::Delete if !*loading => {
                        if let Some(group) = selected_group
                            .filter(|group| !group.worktrees.iter().any(|tree| tree.current))
                        {
                            crate::state::forget_recent_project(&group.common_dir);
                            self.request_recent_projects(&mut effects);
                        }
                        self.modal = Some(modal);
                        return effects;
                    }
                    _ => {
                        edit_text(query, key, false);
                        *selected = 0;
                    }
                }
                self.modal = Some(modal);
            }
            Modal::SshMachines {
                items,
                selected,
                current,
                parent,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.modal = Some(*parent.clone());
                        return effects;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        *selected = previous_list_index(*selected, items.len());
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *selected = next_list_index(*selected, items.len());
                    }
                    KeyCode::Enter => {
                        if let Some(machine) = items.get(*selected)
                            && machine.accessible
                            && machine.target != *current
                        {
                            effects.push(AppEffect::SwitchSshMachine(*selected));
                            return effects;
                        }
                    }
                    _ => {}
                }
                self.modal = Some(modal);
            }
            Modal::PullRequestRepositories {
                items,
                selected,
                query,
                loading,
            } => {
                if key.code == KeyCode::Esc {
                    return effects;
                }
                let visible = Self::filtered_github_repositories(items, &query.value);
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        *selected = previous_list_index(*selected, visible.len());
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *selected = next_list_index(*selected, visible.len());
                    }
                    KeyCode::Enter if !*loading => {
                        if let Some(repository) = visible
                            .get(*selected)
                            .and_then(|index| items.get(*index))
                            .cloned()
                        {
                            self.pull_request_repository = Some(repository);
                            if let Some(number) = self.pull_request_exact_number.or_else(|| {
                                self.pull_request_lookup.value.trim().parse::<u64>().ok()
                            }) {
                                self.request_pull_request_lookup(
                                    number,
                                    false,
                                    false,
                                    &mut effects,
                                );
                            }
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
            Modal::CommandPalette { query, selected } => {
                if key.code == KeyCode::Esc {
                    return effects;
                }
                let commands = self.palette_commands(&query.value);
                match key.code {
                    KeyCode::Up => *selected = previous_list_index(*selected, commands.len()),
                    KeyCode::Down => *selected = next_list_index(*selected, commands.len()),
                    KeyCode::Enter => {
                        if let Some(command) = commands.get(*selected).copied() {
                            self.execute_palette(command, &mut effects, now);
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
            Modal::Themes { selected, original } => {
                match key.code {
                    KeyCode::Esc => {
                        self.apply_theme(*original);
                        return effects;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        *selected = previous_list_index(*selected, ThemeName::ALL.len());
                        if let Some(name) = ThemeName::ALL.get(*selected).copied() {
                            self.apply_theme(name);
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *selected = next_list_index(*selected, ThemeName::ALL.len());
                        if let Some(name) = ThemeName::ALL.get(*selected).copied() {
                            self.apply_theme(name);
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(name) = ThemeName::ALL.get(*selected).copied() {
                            self.apply_theme(name);
                            self.show_toast(
                                format!("Theme changed to {}", name.label()),
                                ToastLevel::Success,
                                now,
                            );
                        }
                        return effects;
                    }
                    _ => {}
                }
                self.modal = Some(modal);
            }
            Modal::Appearances {
                selected,
                original_choice,
                original_appearance,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.appearance_choice = *original_choice;
                        self.appearance = *original_appearance;
                        self.theme = Theme::new(self.theme_name, self.appearance);
                        return effects;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        *selected = previous_list_index(*selected, AppearanceChoice::ALL.len());
                        if let Some(choice) = AppearanceChoice::ALL.get(*selected).copied() {
                            self.set_theme_selection(self.theme_name, choice);
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *selected = next_list_index(*selected, AppearanceChoice::ALL.len());
                        if let Some(choice) = AppearanceChoice::ALL.get(*selected).copied() {
                            self.set_theme_selection(self.theme_name, choice);
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(choice) = AppearanceChoice::ALL.get(*selected).copied() {
                            self.set_theme_selection(self.theme_name, choice);
                            self.show_toast(
                                format!("Appearance changed to {}", choice.label()),
                                ToastLevel::Success,
                                now,
                            );
                        }
                        return effects;
                    }
                    _ => {}
                }
                self.modal = Some(modal);
            }
            _ => {}
        }
        effects
    }
}
