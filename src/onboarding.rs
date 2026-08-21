use std::env;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph};

use crate::git::ProjectGroup;
use crate::theme::Theme;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum OnboardingAction {
    None,
    Open(PathBuf),
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OnboardingPanel {
    Home,
    Projects,
    Path,
}

#[derive(Debug)]
struct ProjectEntry {
    project: String,
    branch: String,
    path: PathBuf,
}

pub(crate) struct Onboarding {
    launch_path: PathBuf,
    projects: Vec<ProjectEntry>,
    selected: usize,
    panel: OnboardingPanel,
    path_input: String,
    error: Option<String>,
}

impl Onboarding {
    pub(crate) fn new(launch_path: &Path) -> Self {
        Self::from_groups(launch_path, crate::state::load_recent_projects(launch_path))
    }

    fn from_groups(launch_path: &Path, groups: Vec<ProjectGroup>) -> Self {
        let projects = groups
            .into_iter()
            .flat_map(|group| {
                group
                    .worktrees
                    .into_iter()
                    .map(move |worktree| ProjectEntry {
                        project: group.name.clone(),
                        branch: worktree.branch_label(),
                        path: worktree.path,
                    })
            })
            .collect();
        Self {
            launch_path: launch_path.to_path_buf(),
            projects,
            selected: 0,
            panel: OnboardingPanel::Home,
            path_input: String::new(),
            error: None,
        }
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> OnboardingAction {
        self.error = None;
        match self.panel {
            OnboardingPanel::Home => self.handle_home_key(key),
            OnboardingPanel::Projects => self.handle_projects_key(key),
            OnboardingPanel::Path => self.handle_path_key(key),
        }
    }

    pub(crate) fn handle_paste(&mut self, text: &str) {
        if self.panel == OnboardingPanel::Path {
            self.path_input
                .extend(text.chars().filter(|character| !character.is_control()));
        }
    }

    pub(crate) fn show_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }

    const fn handle_home_key(&mut self, key: KeyEvent) -> OnboardingAction {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => OnboardingAction::Quit,
            KeyCode::Char('o' | 'O') => {
                self.panel = OnboardingPanel::Path;
                OnboardingAction::None
            }
            KeyCode::Enter if self.projects.is_empty() => {
                self.panel = OnboardingPanel::Path;
                OnboardingAction::None
            }
            KeyCode::Char('w' | 'W' | 'n' | 'N') | KeyCode::Enter => {
                self.panel = OnboardingPanel::Projects;
                OnboardingAction::None
            }
            _ => OnboardingAction::None,
        }
    }

    fn handle_projects_key(&mut self, key: KeyEvent) -> OnboardingAction {
        match key.code {
            KeyCode::Esc => {
                self.panel = OnboardingPanel::Home;
                OnboardingAction::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                OnboardingAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = self
                    .selected
                    .saturating_add(1)
                    .min(self.projects.len().saturating_sub(1));
                OnboardingAction::None
            }
            KeyCode::Enter => self
                .projects
                .get(self.selected)
                .map_or(OnboardingAction::None, |project| {
                    OnboardingAction::Open(project.path.clone())
                }),
            _ => OnboardingAction::None,
        }
    }

    fn handle_path_key(&mut self, key: KeyEvent) -> OnboardingAction {
        match key.code {
            KeyCode::Esc => {
                self.panel = OnboardingPanel::Home;
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

    pub(crate) fn draw(&self, frame: &mut Frame<'_>, theme: &Theme) {
        frame.render_widget(
            Block::default().style(Style::default().bg(theme.background)),
            frame.area(),
        );
        if frame.area().width < 52 || frame.area().height < 16 {
            Self::draw_small(frame, theme);
            return;
        }
        let width = frame.area().width.saturating_sub(8).min(76);
        let height = frame.area().height.saturating_sub(4).min(22);
        let area = centered_rect(width, height, frame.area());
        let [header, body, footer] = Layout::vertical([
            Constraint::Length(9),
            Constraint::Min(9),
            Constraint::Length(3),
        ])
        .areas(area);
        self.draw_header(frame, header, theme);
        match self.panel {
            OnboardingPanel::Home => self.draw_home(frame, body, theme),
            OnboardingPanel::Projects => self.draw_projects(frame, body, theme),
            OnboardingPanel::Path => self.draw_path(frame, body, theme),
        }
        self.draw_footer(frame, footer, theme);
    }

    fn draw_header(&self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        let prompt = match self.panel {
            OnboardingPanel::Home => "Open a project to get started",
            OnboardingPanel::Projects => "Choose a project or worktree",
            OnboardingPanel::Path => "Enter a repository path",
        };
        let logo = Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD);
        frame.render_widget(
            Paragraph::new(Text::from(vec![
                Line::from(Span::styled(
                    " ██████  ██    ██ ██ ███    ██      ██ ███████ ████████",
                    logo,
                )),
                Line::from(Span::styled(
                    "██    ██ ██    ██ ██ ████   ██      ██ ██         ██   ",
                    logo,
                )),
                Line::from(Span::styled(
                    "██    ██ ██    ██ ██ ██ ██  ██      ██ █████      ██   ",
                    logo,
                )),
                Line::from(Span::styled(
                    "██ ▄▄ ██ ██    ██ ██ ██  ██ ██ ██   ██ ██         ██   ",
                    logo,
                )),
                Line::from(Span::styled(
                    " ██████   ██████  ██ ██   ████  █████  ███████    ██   ",
                    logo,
                )),
                Line::from(Span::styled(
                    "keyboard-first Git workspace",
                    Style::default().fg(theme.muted),
                )),
                Line::from(""),
                Line::from(Span::styled(prompt, Style::default().fg(theme.text))),
            ]))
            .alignment(Alignment::Center),
            area,
        );
    }

    fn draw_home(&self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        let recent = if self.projects.is_empty() {
            "No recent projects yet".to_owned()
        } else if self.projects.len() == 1 {
            "1 recent worktree".to_owned()
        } else {
            format!("{} recent worktrees", self.projects.len())
        };
        let card_area = centered_rect(62_u16.min(area.width), 9_u16.min(area.height), area);
        let block = Block::default()
            .title(Span::styled(
                " Get started ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.border_focus))
            .style(Style::default().bg(theme.panel));
        let inner = block.inner(card_area);
        frame.render_widget(block, card_area);
        let rows = vec![
            Line::from(""),
            menu_line("Projects and worktrees", "W / N", inner.width, theme),
            menu_line("Open a repository path", "O", inner.width, theme),
            menu_line("Quit", "Q", inner.width, theme),
            Line::from(""),
            Line::from(Span::styled(recent, Style::default().fg(theme.muted))),
        ];
        frame.render_widget(
            Paragraph::new(rows)
                .alignment(Alignment::Center)
                .style(Style::default().bg(theme.panel)),
            inner,
        );
    }

    fn draw_projects(&self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        if self.projects.is_empty() {
            let card_area = centered_rect(62_u16.min(area.width), 7_u16.min(area.height), area);
            frame.render_widget(
                Paragraph::new(Text::from(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "No recent projects",
                        Style::default().fg(theme.text),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Press Esc, then O to enter a repository path.",
                        Style::default().fg(theme.muted),
                    )),
                ]))
                .alignment(Alignment::Center)
                .block(panel_block(" Recent projects ", theme)),
                card_area,
            );
            return;
        }
        let items = self
            .projects
            .iter()
            .map(|project| {
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(" {} ", project.project),
                        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{}  ", project.branch),
                        Style::default().fg(theme.accent),
                    ),
                    Span::styled(
                        project.path.display().to_string(),
                        Style::default().fg(theme.muted),
                    ),
                ]))
            })
            .collect::<Vec<_>>();
        let mut state = ListState::default().with_selected(Some(self.selected));
        let card_area = centered_rect(70_u16.min(area.width), area.height, area);
        frame.render_stateful_widget(
            List::new(items)
                .block(panel_block(" Recent projects ", theme))
                .highlight_style(
                    Style::default()
                        .bg(theme.selected)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("›"),
            card_area,
            &mut state,
        );
    }

    fn draw_path(&self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        let input = Paragraph::new(Line::from(vec![
            Span::styled(" > ", Style::default().fg(theme.accent)),
            Span::styled(&self.path_input, Style::default().fg(theme.text)),
        ]))
        .block(
            Block::default()
                .title(" Repository path ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(theme.border_focus)),
        );
        let input_area = centered_rect(62_u16.min(area.width), 3, area);
        frame.render_widget(input, input_area);
        let cursor_offset = u16::try_from(self.path_input.chars().count()).unwrap_or(u16::MAX);
        frame.set_cursor_position((
            input_area.x.saturating_add(4).saturating_add(cursor_offset),
            input_area.y.saturating_add(1),
        ));
    }

    fn draw_footer(&self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        let hint = match self.panel {
            OnboardingPanel::Home => "",
            OnboardingPanel::Projects => "↑/↓ or j/k move   Enter open   Esc back",
            OnboardingPanel::Path => "Enter open   Esc back",
        };
        let mut lines = vec![Line::from(Span::styled(
            hint,
            Style::default().fg(theme.muted),
        ))];
        if let Some(error) = self.error.as_ref() {
            lines.push(Line::from(Span::styled(
                error,
                Style::default().fg(theme.error),
            )));
        }
        frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
    }

    fn draw_small(frame: &mut Frame<'_>, theme: &Theme) {
        frame.render_widget(
            Paragraph::new(Text::from(vec![
                Line::from(Span::styled(
                    "Quinjet",
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Open a project to get started",
                    Style::default().fg(theme.text),
                )),
                Line::from(Span::styled(
                    "W projects   O path   Q quit",
                    Style::default().fg(theme.muted),
                )),
            ]))
            .alignment(Alignment::Center),
            centered_rect(44, 6, frame.area()),
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

fn menu_line<'a>(label: &'a str, key: &'a str, width: u16, theme: &Theme) -> Line<'a> {
    let label_width = usize::from(width.saturating_sub(11));
    Line::from(vec![
        Span::styled("  │  ", Style::default().fg(theme.accent)),
        Span::styled(
            format!("{label:<label_width$}"),
            Style::default().fg(theme.text),
        ),
        Span::styled(
            format!("{key:>6}"),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn panel_block<'a>(title: &'a str, theme: &Theme) -> Block<'a> {
    Block::default()
        .title(Span::styled(title, Style::default().fg(theme.accent)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.panel))
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
