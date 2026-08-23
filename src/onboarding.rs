use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::app::{App, ProjectOpenMode, TextBuffer};
use crate::git::ProjectGroup;
use crate::ssh::SshContext;
use crate::theme::Theme;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum OnboardingAction {
    None,
    Open(PathBuf),
    SwitchSshMachine(usize),
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
    panel: OnboardingPanel,
    path_input: String,
    error: Option<String>,
    ssh_context: Option<SshContext>,
    machine_selected: Option<usize>,
    machine_hits: Vec<(Rect, usize)>,
    mode: ProjectOpenMode,
}

impl Onboarding {
    pub(crate) fn new(
        launch_path: &Path,
        ssh_context: Option<SshContext>,
        mode: ProjectOpenMode,
    ) -> Self {
        Self::from_groups_with_mode(
            launch_path,
            crate::state::load_recent_projects(launch_path),
            ssh_context,
            mode,
        )
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
        Self {
            launch_path: launch_path.to_path_buf(),
            groups,
            selected: 0,
            query: TextBuffer::default(),
            collapsed: HashSet::new(),
            collapse_hits: Vec::new(),
            panel: OnboardingPanel::Projects,
            path_input: String::new(),
            error: None,
            ssh_context,
            machine_selected: None,
            machine_hits: Vec::new(),
            mode,
        }
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> OnboardingAction {
        self.error = None;
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
                self.machine_selected = None;
                self.query.insert_str(&sanitized);
                self.selected = 0;
            }
            OnboardingPanel::Path => self.path_input.push_str(&sanitized),
        }
    }

    pub(crate) fn handle_mouse(&mut self, mouse: MouseEvent) -> OnboardingAction {
        if self.panel != OnboardingPanel::Projects
            || mouse.kind != MouseEventKind::Down(MouseButton::Left)
        {
            return OnboardingAction::None;
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
        let Some(common_dir) = self
            .collapse_hits
            .iter()
            .find(|(area, _)| area.contains((mouse.column, mouse.row).into()))
            .map(|(_, common_dir)| common_dir.clone())
        else {
            return OnboardingAction::None;
        };
        if self.collapsed.contains(&common_dir) {
            self.collapsed.retain(|candidate| candidate != &common_dir);
        } else {
            self.collapsed.extend([common_dir]);
        }
        let visible = App::filtered_project_rows(&self.groups, &self.query.value, &self.collapsed);
        self.selected = self.selected.min(visible.len().saturating_sub(1));
        OnboardingAction::None
    }

    pub(crate) fn show_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }

    fn handle_projects_key(&mut self, key: KeyEvent) -> OnboardingAction {
        match key.code {
            KeyCode::Esc => return OnboardingAction::Quit,
            KeyCode::Char('e' | 'E') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                App::toggle_all_project_groups(&self.groups, &mut self.collapsed);
                self.selected = 0;
                return OnboardingAction::None;
            }
            KeyCode::Tab if self.ssh_context.is_some() => {
                self.machine_selected = self.machine_selected.map_or_else(
                    || {
                        self.ssh_context.as_ref().and_then(|context| {
                            context
                                .machines
                                .iter()
                                .position(|machine| machine.target == context.current)
                        })
                    },
                    |_| None,
                );
                return OnboardingAction::None;
            }
            KeyCode::Char('o' | 'O') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.panel = OnboardingPanel::Path;
                return OnboardingAction::None;
            }
            _ => {}
        }
        if let Some(action) = self.handle_focused_machine_key(key) {
            return action;
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
            KeyCode::Enter => visible
                .get(self.selected)
                .and_then(|(group_index, tree_index)| {
                    self.groups
                        .get(*group_index)
                        .and_then(|group| group.worktrees.get(*tree_index))
                })
                .map_or(OnboardingAction::None, |tree| {
                    OnboardingAction::Open(tree.path.clone())
                }),
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

    fn handle_focused_machine_key(&mut self, key: KeyEvent) -> Option<OnboardingAction> {
        let selected = self.machine_selected?;
        let Some(context) = self.ssh_context.as_ref() else {
            self.machine_selected = None;
            return None;
        };
        match key.code {
            KeyCode::Left | KeyCode::Up | KeyCode::Char('h' | 'k') => {
                self.machine_selected =
                    crate::ssh::previous_accessible_machine_index(&context.machines, selected)
                        .or(Some(selected));
                Some(OnboardingAction::None)
            }
            KeyCode::Right | KeyCode::Down | KeyCode::Char('j' | 'l') => {
                self.machine_selected =
                    crate::ssh::next_accessible_machine_index(&context.machines, selected)
                        .or(Some(selected));
                Some(OnboardingAction::None)
            }
            KeyCode::Enter => Some(context.machines.get(selected).map_or(
                OnboardingAction::None,
                |machine| {
                    if machine.accessible && machine.target != context.current {
                        OnboardingAction::SwitchSshMachine(selected)
                    } else {
                        OnboardingAction::None
                    }
                },
            )),
            _ => {
                self.machine_selected = None;
                None
            }
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

    pub(crate) fn draw(&mut self, frame: &mut Frame<'_>, theme: &Theme) {
        frame.render_widget(
            Block::default().style(Style::default().bg(theme.background)),
            frame.area(),
        );
        self.machine_hits.clear();
        match self.panel {
            OnboardingPanel::Projects => {
                self.collapse_hits.clear();
                self.machine_hits = crate::ui::pickers::draw_projects(
                    frame,
                    &mut self.collapse_hits,
                    &self.groups,
                    self.selected,
                    &self.query,
                    &self.collapsed,
                    false,
                    None,
                    self.mode,
                    self.ssh_context.as_ref(),
                    self.machine_selected,
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

#[cfg(test)]
mod tests;
