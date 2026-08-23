use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::app::{App, ProjectOpenMode, ProjectRow, TextBuffer};
use crate::git::ProjectGroup;
use crate::ssh::{SshContext, SshProjectOpenMode, SshSwitch};
use crate::theme::Theme;

mod loader;
use loader::ProjectLoader;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum OnboardingAction {
    None,
    Open(PathBuf),
    SwitchSshMachine(SshSwitch),
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OnboardingPanel {
    Projects,
    Path,
}

pub(crate) struct Onboarding {
    launch_path: PathBuf,
    groups: Vec<ProjectGroup>,
    selected: usize,
    query: TextBuffer,
    collapsed: HashSet<PathBuf>,
    collapse_hits: Vec<(Rect, PathBuf)>,
    project_hits: Vec<(Rect, usize)>,
    project_len: usize,
    project_max_scroll: usize,
    project_scroll: usize,
    project_free_scroll: bool,
    panel: OnboardingPanel,
    path_input: String,
    error: Option<String>,
    ssh_context: Option<SshContext>,
    machine_hits: Vec<(Rect, usize)>,
    mode: ProjectOpenMode,
    project_loader: ProjectLoader,
}

impl Onboarding {
    pub(crate) fn new(
        launch_path: &Path,
        ssh_context: Option<SshContext>,
        mode: ProjectOpenMode,
    ) -> Self {
        let mut onboarding =
            Self::from_groups_with_mode(launch_path, Vec::new(), ssh_context, mode);
        onboarding.project_loader = ProjectLoader::start(launch_path);
        onboarding.collapsed = crate::state::load_collapsed_project_groups();
        onboarding.selected =
            App::first_project_worktree_index(&onboarding.groups, "", &onboarding.collapsed);
        onboarding
    }

    #[cfg(test)]
    fn from_groups(
        launch_path: &Path,
        groups: Vec<ProjectGroup>,
        ssh_context: Option<SshContext>,
    ) -> Self {
        Self::from_groups_with_mode(launch_path, groups, ssh_context, ProjectOpenMode::Initial)
    }

    fn from_groups_with_mode(
        launch_path: &Path,
        groups: Vec<ProjectGroup>,
        ssh_context: Option<SshContext>,
        mode: ProjectOpenMode,
    ) -> Self {
        let collapsed = HashSet::new();
        let selected = App::first_project_worktree_index(&groups, "", &collapsed);
        Self {
            launch_path: launch_path.to_path_buf(),
            groups,
            selected,
            query: TextBuffer::default(),
            collapsed,
            collapse_hits: Vec::new(),
            project_hits: Vec::new(),
            project_len: 0,
            project_max_scroll: 0,
            project_scroll: 0,
            project_free_scroll: false,
            panel: OnboardingPanel::Projects,
            path_input: String::new(),
            error: None,
            ssh_context,
            machine_hits: Vec::new(),
            mode,
            project_loader: ProjectLoader::ready(),
        }
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> OnboardingAction {
        self.error = None;
        self.project_free_scroll = false;
        match self.panel {
            OnboardingPanel::Projects => self.handle_projects_key(key),
            OnboardingPanel::Path => self.handle_path_key(key),
        }
    }

    pub(crate) fn handle_paste(&mut self, text: &str) {
        let sanitized = text
            .chars()
            .filter(|character| !character.is_control())
            .collect::<String>();
        match self.panel {
            OnboardingPanel::Projects => {
                self.query.insert_str(&sanitized);
                self.selected = 0;
                self.project_scroll = 0;
                self.project_free_scroll = false;
            }
            OnboardingPanel::Path => self.path_input.push_str(&sanitized),
        }
    }

    pub(crate) fn show_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }

    pub(crate) fn ssh_context(&self) -> Option<SshContext> {
        self.ssh_context.clone()
    }

    pub(crate) fn apply_ssh_probe(&mut self, accessibility: &[(String, bool)]) {
        let Some(context) = self.ssh_context.as_mut() else {
            return;
        };
        for machine in &mut context.machines {
            if let Some((_, accessible)) = accessibility
                .iter()
                .find(|(target, _)| target == &machine.target)
            {
                machine.accessible = *accessible;
            }
        }
        context.probing = false;
    }

    pub(crate) fn poll_projects(&mut self) -> bool {
        let Some(groups) = self.project_loader.poll() else {
            return false;
        };
        self.groups = groups;
        self.selected =
            App::first_project_worktree_index(&self.groups, &self.query.value, &self.collapsed);
        true
    }

    fn handle_projects_key(&mut self, key: KeyEvent) -> OnboardingAction {
        match key.code {
            KeyCode::Esc => return self.cancel_or_quit(),
            KeyCode::Char('e' | 'E') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                App::toggle_all_project_groups(&self.groups, &mut self.collapsed);
                self.selected = 0;
                #[cfg(not(test))]
                crate::state::record_collapsed_project_groups(&self.collapsed);
                return OnboardingAction::None;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let reverse =
                    key.code == KeyCode::BackTab || key.modifiers.contains(KeyModifiers::SHIFT);
                let index = self
                    .ssh_context
                    .as_ref()
                    .and_then(|context| context.adjacent_accessible_machine_index(reverse));
                return index.map_or(OnboardingAction::None, |index| self.switch_machine(index));
            }
            KeyCode::Char('o' | 'O') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.panel = OnboardingPanel::Path;
                return OnboardingAction::None;
            }
            _ => {}
        }
        let visible = App::filtered_project_rows(&self.groups, &self.query.value, &self.collapsed);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                OnboardingAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = self
                    .selected
                    .saturating_add(1)
                    .min(visible.len().saturating_sub(1));
                OnboardingAction::None
            }
            KeyCode::Enter if visible.is_empty() => {
                self.panel = OnboardingPanel::Path;
                OnboardingAction::None
            }
            KeyCode::Enter => match visible.get(self.selected) {
                Some(ProjectRow::Group(group_index)) => {
                    if let Some(group) = self.groups.get(*group_index) {
                        if self.collapsed.contains(&group.common_dir) {
                            self.collapsed
                                .retain(|candidate| candidate != &group.common_dir);
                        } else {
                            self.collapsed.extend([group.common_dir.clone()]);
                        }
                        #[cfg(not(test))]
                        crate::state::record_collapsed_project_groups(&self.collapsed);
                    }
                    OnboardingAction::None
                }
                Some(ProjectRow::Worktree {
                    group_index,
                    tree_index,
                }) => self
                    .groups
                    .get(*group_index)
                    .and_then(|group| group.worktrees.get(*tree_index))
                    .map_or(OnboardingAction::None, |tree| {
                        OnboardingAction::Open(tree.path.clone())
                    }),
                None => OnboardingAction::None,
            },
            KeyCode::Backspace => {
                self.query.backspace();
                self.selected = 0;
                OnboardingAction::None
            }
            KeyCode::Delete => {
                self.query.delete();
                self.selected = 0;
                OnboardingAction::None
            }
            KeyCode::Left => {
                self.query.move_left();
                OnboardingAction::None
            }
            KeyCode::Right => {
                self.query.move_right();
                OnboardingAction::None
            }
            KeyCode::Home => {
                self.query.home();
                OnboardingAction::None
            }
            KeyCode::End => {
                self.query.end();
                OnboardingAction::None
            }
            KeyCode::Char(character)
                if !key.modifiers.intersects(
                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                ) =>
            {
                self.query.insert(character);
                self.selected = 0;
                OnboardingAction::None
            }
            _ => OnboardingAction::None,
        }
    }

    fn handle_path_key(&mut self, key: KeyEvent) -> OnboardingAction {
        match key.code {
            KeyCode::Esc => {
                self.panel = OnboardingPanel::Projects;
                OnboardingAction::None
            }
            KeyCode::Enter => self
                .entered_path()
                .map_or(OnboardingAction::None, OnboardingAction::Open),
            KeyCode::Backspace => {
                let length = self
                    .path_input
                    .char_indices()
                    .next_back()
                    .map_or(0, |(index, _)| index);
                self.path_input.truncate(length);
                OnboardingAction::None
            }
            KeyCode::Char(character)
                if !key.modifiers.intersects(
                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                ) =>
            {
                self.path_input.push(character);
                OnboardingAction::None
            }
            _ => OnboardingAction::None,
        }
    }

    fn entered_path(&self) -> Option<PathBuf> {
        let entered = self.path_input.trim();
        if entered.is_empty() {
            return None;
        }
        let path = expand_home(entered);
        Some(if path.is_absolute() {
            path
        } else {
            self.launch_path.join(path)
        })
    }

    fn switch_machine(&mut self, index: usize) -> OnboardingAction {
        let mode = if self.mode == ProjectOpenMode::NewTab {
            SshProjectOpenMode::New
        } else {
            SshProjectOpenMode::Current
        };
        let Some(context) = self.ssh_context.as_mut() else {
            return OnboardingAction::None;
        };
        let Some(machine) = context.machines.get(index) else {
            return OnboardingAction::None;
        };
        if !machine.accessible || machine.target == context.current {
            return OnboardingAction::None;
        }
        if mode == SshProjectOpenMode::New
            && let Some(pending) = context.tabs.active_pending_for_machine(&context.current)
        {
            let _moved =
                context
                    .tabs
                    .move_pending(pending, machine.target.clone(), machine.folder.clone());
        }
        OnboardingAction::SwitchSshMachine(SshSwitch { index, mode })
    }

    fn cancel_or_quit(&mut self) -> OnboardingAction {
        if self.mode != ProjectOpenMode::NewTab {
            return OnboardingAction::Quit;
        }
        let Some(context) = self.ssh_context.as_mut() else {
            return OnboardingAction::Quit;
        };
        let Some(pending) = context.tabs.active_pending_for_machine(&context.current) else {
            return OnboardingAction::Quit;
        };
        drop(context.tabs.close(pending));
        let Some(active) = context.tabs.active_id() else {
            return OnboardingAction::Quit;
        };
        let Some(tab) = context.tabs.get(active) else {
            return OnboardingAction::Quit;
        };
        context
            .machines
            .iter()
            .position(|machine| machine.target == tab.machine && machine.accessible)
            .map_or(OnboardingAction::Quit, |index| {
                OnboardingAction::SwitchSshMachine(SshSwitch {
                    index,
                    mode: SshProjectOpenMode::Activate,
                })
            })
    }

    pub(crate) fn draw(&mut self, frame: &mut Frame<'_>, theme: &Theme) {
        frame.render_widget(
            Block::default().style(Style::default().bg(theme.background)),
            frame.area(),
        );
        self.machine_hits.clear();
        match self.panel {
            OnboardingPanel::Projects => {
                self.collapse_hits.clear();
                self.project_hits.clear();
                let mut list = crate::ui::ModalList::new(
                    &mut self.project_hits,
                    &mut self.project_len,
                    &mut self.project_max_scroll,
                    self.project_scroll,
                    self.project_free_scroll,
                );
                self.machine_hits = crate::ui::pickers::draw_projects(
                    frame,
                    &mut self.collapse_hits,
                    &self.groups,
                    self.selected,
                    &self.query,
                    &self.collapsed,
                    self.project_loader.is_loading(),
                    None,
                    self.mode,
                    self.ssh_context.as_ref(),
                    &mut list,
                    theme,
                );
            }
            OnboardingPanel::Path => self.draw_path(frame, theme),
        }
        self.draw_error(frame, theme);
    }

    fn draw_path(&self, frame: &mut Frame<'_>, theme: &Theme) {
        let area = centered_rect(
            frame.area().width.saturating_sub(10).min(88),
            7_u16.min(frame.area().height),
            frame.area(),
        );
        let block = Block::default()
            .title(" Open repository path ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.border_focus))
            .style(Style::default().bg(theme.panel));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let input_area = Rect::new(inner.x, inner.y, inner.width, 1);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" / ", Style::default().fg(theme.accent)),
                Span::styled(&self.path_input, Style::default().fg(theme.text)),
            ]))
            .style(Style::default().bg(theme.panel_alt)),
            input_area,
        );
        frame.render_widget(
            Paragraph::new("Enter open   Esc projects")
                .alignment(Alignment::Center)
                .style(Style::default().fg(theme.muted)),
            Rect::new(inner.x, area.bottom().saturating_sub(2), inner.width, 1),
        );
        let cursor_offset = u16::try_from(self.path_input.chars().count()).unwrap_or(u16::MAX);
        frame.set_cursor_position((
            input_area.x.saturating_add(3).saturating_add(cursor_offset),
            input_area.y,
        ));
    }

    fn draw_error(&self, frame: &mut Frame<'_>, theme: &Theme) {
        let Some(error) = self.error.as_ref() else {
            return;
        };
        frame.render_widget(
            Paragraph::new(error.as_str())
                .alignment(Alignment::Center)
                .style(Style::default().fg(theme.error)),
            Rect::new(
                frame.area().x,
                frame.area().bottom().saturating_sub(2),
                frame.area().width,
                1,
            ),
        );
    }
}

fn expand_home(value: &str) -> PathBuf {
    let Some(remainder) = value.strip_prefix("~/") else {
        if value == "~" {
            return home_path().unwrap_or_else(|| PathBuf::from(value));
        }
        return PathBuf::from(value);
    };
    home_path().map_or_else(|| PathBuf::from(value), |home| home.join(remainder))
}

fn home_path() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

#[expect(
    clippy::integer_division,
    reason = "terminal coordinates use whole cells"
)]
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width.min(area.width),
        height.min(area.height),
    )
}

mod mouse;
#[cfg(test)]
mod tests;
