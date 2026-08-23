#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(crate) fn set_repository_tabs(&mut self, tabs: Vec<TabInfo>) {
        self.repository_tabs = tabs;
        if self
            .repository_tab_drag
            .is_some_and(|drag| !self.repository_tabs.iter().any(|tab| tab.id == drag.id))
        {
            self.repository_tab_drag = None;
        }
        if self
            .repository_tab_menu
            .is_some_and(|menu| !self.repository_tabs.iter().any(|tab| tab.id == menu.id))
        {
            self.repository_tab_menu = None;
        }
    }

    pub(crate) fn set_tab_active(&mut self, active: bool, now: Instant) {
        if self.tab_active == active {
            return;
        }
        self.tab_active = active;
        self.schedule_pull_request_poll(now);
    }

    pub(super) fn handle_repository_tab_key(&mut self, key: KeyEvent) -> Option<Vec<AppEffect>> {
        if let Some(menu) = self.repository_tab_menu.as_mut() {
            let mut effects = Vec::new();
            match key.code {
                KeyCode::Esc => self.repository_tab_menu = None,
                KeyCode::Up | KeyCode::Char('k') => {
                    menu.selected =
                        previous_list_index(menu.selected, RepositoryTabAction::ALL.len());
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    menu.selected = next_list_index(menu.selected, RepositoryTabAction::ALL.len());
                }
                KeyCode::Enter => {
                    let id = menu.id;
                    let action = RepositoryTabAction::ALL.get(menu.selected).copied();
                    self.repository_tab_menu = None;
                    if let Some(action) = action {
                        self.handle_repository_tab_action(id, action, &mut effects);
                    }
                }
                _ => {}
            }
            return Some(effects);
        }
        if key.modifiers == KeyModifiers::ALT {
            return repository_tab_shortcut_index(key.code).map(|index| {
                self.repository_tabs
                    .get(index)
                    .map_or_else(Vec::new, |tab| {
                        vec![AppEffect::ActivateRepositoryTab(tab.id)]
                    })
            });
        }
        if !key.modifiers.contains(KeyModifiers::CONTROL) {
            return None;
        }
        if self.exit_locked() && key.code == KeyCode::Char('w') {
            return Some(Vec::new());
        }
        let reverse = key.modifiers.contains(KeyModifiers::SHIFT) || key.code == KeyCode::BackTab;
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => Some(
                self.neighbor_repository_tab(reverse)
                    .map_or_else(Vec::new, |id| vec![AppEffect::ActivateRepositoryTab(id)]),
            ),
            KeyCode::Char('w') => Some(
                self.active_repository_tab()
                    .map_or_else(Vec::new, |id| vec![AppEffect::CloseRepositoryTab(id)]),
            ),
            _ => None,
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "tab pointer handling shares drag, close, overflow, and menu state"
    )]
    pub(super) fn handle_repository_tab_mouse(
        &mut self,
        event: MouseEvent,
    ) -> Option<Vec<AppEffect>> {
        let point = (event.column, event.row).into();
        if self.exit_locked() && event.kind == MouseEventKind::Down(MouseButton::Right) {
            self.repository_tab_menu = None;
            return Some(Vec::new());
        }
        if event.kind == MouseEventKind::Down(MouseButton::Right) {
            if let Some(id) = self
                .geometry
                .repository_tab_hits
                .iter()
                .find(|hit| hit.area.contains(point))
                .map(|hit| hit.id)
            {
                self.repository_tab_drag = None;
                self.repository_tab_menu = Some(RepositoryTabMenu {
                    id,
                    column: event.column,
                    row: event.row,
                    selected: 0,
                });
                return Some(Vec::new());
            }
            if self.repository_tab_menu.take().is_some() {
                return Some(Vec::new());
            }
        }
        if let Some(menu) = self.repository_tab_menu.as_mut() {
            match event.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    let target = self
                        .geometry
                        .repository_tab_menu_hits
                        .iter()
                        .find(|(area, _)| area.contains(point))
                        .map(|(_, action)| *action);
                    let id = menu.id;
                    self.repository_tab_menu = None;
                    let mut effects = Vec::new();
                    if let Some(action) = target {
                        self.handle_repository_tab_action(id, action, &mut effects);
                    }
                    return Some(effects);
                }
                MouseEventKind::ScrollUp => {
                    menu.selected =
                        previous_list_index(menu.selected, RepositoryTabAction::ALL.len());
                    return Some(Vec::new());
                }
                MouseEventKind::ScrollDown => {
                    menu.selected = next_list_index(menu.selected, RepositoryTabAction::ALL.len());
                    return Some(Vec::new());
                }
                _ => return Some(Vec::new()),
            }
        }
        match event.kind {
            MouseEventKind::Down(MouseButton::Left)
                if self.geometry.repository_tab_previous.contains(point) =>
            {
                Some(
                    self.neighbor_repository_tab(true)
                        .map_or_else(Vec::new, |id| vec![AppEffect::ActivateRepositoryTab(id)]),
                )
            }
            MouseEventKind::Down(MouseButton::Left)
                if self.geometry.repository_tab_next.contains(point) =>
            {
                Some(
                    self.neighbor_repository_tab(false)
                        .map_or_else(Vec::new, |id| vec![AppEffect::ActivateRepositoryTab(id)]),
                )
            }
            MouseEventKind::Down(MouseButton::Left)
                if self.geometry.repository_tab_open.contains(point) =>
            {
                let mut effects = Vec::new();
                self.open_projects(ProjectOpenMode::NewTab, &mut effects);
                Some(effects)
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(id) = self
                    .geometry
                    .repository_tab_hits
                    .iter()
                    .find(|hit| hit.close.contains(point))
                    .map(|hit| hit.id)
                {
                    return Some(vec![AppEffect::CloseRepositoryTab(id)]);
                }
                let id = self
                    .geometry
                    .repository_tab_hits
                    .iter()
                    .find(|hit| hit.area.contains(point))
                    .map(|hit| hit.id)?;
                self.repository_tab_drag = Some(RepositoryTabDrag { id, target: None });
                Some(Vec::new())
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let dragged = self.repository_tab_drag?.id;
                let target = self
                    .geometry
                    .repository_tab_hits
                    .iter()
                    .find(|hit| hit.area.contains(point))
                    .map(|hit| hit.id);
                if let Some(drag) = self.repository_tab_drag.as_mut() {
                    drag.target = target.filter(|target| *target != dragged);
                }
                Some(Vec::new())
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let drag = self.repository_tab_drag.take()?;
                Some(drag.target.map_or_else(
                    || vec![AppEffect::ActivateRepositoryTab(drag.id)],
                    |target| {
                        vec![AppEffect::ReorderRepositoryTab {
                            source: drag.id,
                            target,
                        }]
                    },
                ))
            }
            _ => None,
        }
    }

    fn active_repository_tab(&self) -> Option<TabId> {
        self.repository_tabs
            .iter()
            .find(|tab| tab.active)
            .map(|tab| tab.id)
    }

    fn neighbor_repository_tab(&self, reverse: bool) -> Option<TabId> {
        let active = self.active_repository_tab()?;
        if reverse {
            self.repository_tabs
                .iter()
                .take_while(|tab| tab.id != active)
                .last()
                .or_else(|| self.repository_tabs.last())
                .map(|tab| tab.id)
        } else {
            self.repository_tabs
                .iter()
                .skip_while(|tab| tab.id != active)
                .nth(1)
                .or_else(|| self.repository_tabs.first())
                .map(|tab| tab.id)
        }
    }

    fn handle_repository_tab_action(
        &mut self,
        id: TabId,
        action: RepositoryTabAction,
        effects: &mut Vec<AppEffect>,
    ) {
        match action {
            RepositoryTabAction::OpenProject => {
                self.open_projects(ProjectOpenMode::NewTab, effects);
            }
            RepositoryTabAction::Close => {
                if !self.exit_locked() {
                    effects.push(AppEffect::CloseRepositoryTab(id));
                }
            }
            RepositoryTabAction::CloseOthers => {
                effects.push(AppEffect::CloseOtherRepositoryTabs(id));
            }
            RepositoryTabAction::CloseAll => {
                if !self.exit_locked() {
                    effects.push(AppEffect::CloseAllRepositoryTabs);
                }
            }
        }
    }
}

const fn repository_tab_shortcut_index(code: KeyCode) -> Option<usize> {
    match code {
        KeyCode::Char('1') => Some(0),
        KeyCode::Char('2') => Some(1),
        KeyCode::Char('3') => Some(2),
        KeyCode::Char('4') => Some(3),
        KeyCode::Char('5') => Some(4),
        KeyCode::Char('6') => Some(5),
        KeyCode::Char('7') => Some(6),
        KeyCode::Char('8') => Some(7),
        KeyCode::Char('9') => Some(8),
        _ => None,
    }
}
