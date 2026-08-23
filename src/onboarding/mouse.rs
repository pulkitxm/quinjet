use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use super::{App, Onboarding, OnboardingAction, OnboardingPanel, ProjectRow};

impl Onboarding {
    pub(crate) fn handle_mouse(&mut self, mouse: MouseEvent) -> OnboardingAction {
        if self.panel != OnboardingPanel::Projects {
            return OnboardingAction::None;
        }
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.project_scroll = self.project_scroll.saturating_sub(2);
                self.project_free_scroll = true;
                return OnboardingAction::None;
            }
            MouseEventKind::ScrollDown => {
                self.project_scroll = self
                    .project_scroll
                    .saturating_add(2)
                    .min(self.project_max_scroll);
                self.project_free_scroll = true;
                return OnboardingAction::None;
            }
            MouseEventKind::Moved => {
                if let Some(index) = self
                    .project_hits
                    .iter()
                    .find(|(area, _)| area.contains((mouse.column, mouse.row).into()))
                    .map(|(_, index)| *index)
                {
                    self.selected = index;
                }
                return OnboardingAction::None;
            }
            MouseEventKind::Down(MouseButton::Left) => {}
            _ => return OnboardingAction::None,
        }
        if let Some(index) = self
            .machine_hits
            .iter()
            .find(|(area, _)| area.contains((mouse.column, mouse.row).into()))
            .map(|(_, index)| *index)
        {
            self.machine_selected = Some(index);
            return self
                .ssh_context
                .as_ref()
                .map_or(OnboardingAction::None, |context| {
                    context
                        .machines
                        .get(index)
                        .map_or(OnboardingAction::None, |machine| {
                            if machine.accessible && machine.target != context.current {
                                OnboardingAction::SwitchSshMachine(index)
                            } else {
                                OnboardingAction::None
                            }
                        })
                });
        }
        if let Some(common_dir) = self
            .collapse_hits
            .iter()
            .find(|(area, _)| area.contains((mouse.column, mouse.row).into()))
            .map(|(_, common_dir)| common_dir.clone())
        {
            if self.collapsed.contains(&common_dir) {
                self.collapsed.retain(|candidate| candidate != &common_dir);
            } else {
                self.collapsed.extend([common_dir.clone()]);
            }
            #[cfg(not(test))]
            crate::state::record_collapsed_project_groups(&self.collapsed);
            let visible =
                App::filtered_project_rows(&self.groups, &self.query.value, &self.collapsed);
            self.selected = visible
                .iter()
                .position(|row| {
                    matches!(row, ProjectRow::Group(group_index) if self.groups.get(*group_index).is_some_and(|group| group.common_dir == common_dir))
                })
                .unwrap_or_else(|| self.selected.min(visible.len().saturating_sub(1)));
            return OnboardingAction::None;
        }
        let Some(index) = self
            .project_hits
            .iter()
            .find(|(area, _)| area.contains((mouse.column, mouse.row).into()))
            .map(|(_, index)| *index)
        else {
            return OnboardingAction::None;
        };
        self.selected = index;
        self.project_free_scroll = false;
        self.handle_projects_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
    }
}
