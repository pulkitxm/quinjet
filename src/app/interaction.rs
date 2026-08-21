use super::*;

impl App {
    /// Reporting the mouse to the application is what stops a terminal from
    /// selecting text with it. Releasing it hands selection and copying back to
    /// the terminal, which is the only place either can happen.
    pub(super) fn toggle_mouse_capture(&mut self, now: Instant) -> AppEffect {
        self.mouse_capture = !self.mouse_capture;
        self.mouse_capture_preference = self.mouse_capture;
        self.show_toast(
            if self.mouse_capture {
                "Mouse on · quinjet handles clicks and the wheel".to_owned()
            } else {
                "Mouse off · select and copy with the terminal, m to restore".to_owned()
            },
            ToastLevel::Info,
            now,
        );
        AppEffect::SetMouseCapture(self.mouse_capture)
    }

    pub(crate) const fn configure_mouse_capture(&mut self, enabled: bool) {
        self.mouse_capture = enabled;
        self.mouse_capture_preference = enabled;
    }

    pub(super) fn set_focus(&mut self, focus: Focus, effects: &mut Vec<AppEffect>) {
        self.focus = focus;
        let enabled = self.mouse_capture_preference;
        if self.mouse_capture != enabled {
            self.mouse_capture = enabled;
            effects.push(AppEffect::SetMouseCapture(enabled));
        }
    }

    pub(super) fn start_text_selection(&mut self, column: u16, row: u16) {
        let Some(pane) = self.text_selection_pane(column, row) else {
            return;
        };
        self.text_selection = Some(TextSelection {
            pane,
            anchor: (column, row),
            head: (column, row),
        });
    }

    pub(super) fn update_text_selection(&mut self, column: u16, row: u16) {
        let Some(selection) = self.text_selection.as_mut() else {
            return;
        };
        selection.head = (
            column.clamp(selection.pane.x, selection.pane.right().saturating_sub(1)),
            row.clamp(selection.pane.y, selection.pane.bottom().saturating_sub(1)),
        );
    }

    pub(super) const fn scroll_horizontal(&mut self, right: bool) {
        self.horizontal_scroll = if right {
            self.horizontal_scroll.saturating_add(3)
        } else {
            self.horizontal_scroll.saturating_sub(3)
        };
        self.text_selection = None;
    }

    pub(super) fn text_selection_pane(&self, column: u16, row: u16) -> Option<Rect> {
        let point = (column, row).into();
        if !self.geometry.content.contains(point) {
            return None;
        }
        let Some(divider) = self.geometry.diff_divider else {
            return Some(self.geometry.content);
        };
        if column < divider.x {
            return Some(Rect::new(
                self.geometry.content.x,
                self.geometry.content.y,
                divider.x.saturating_sub(self.geometry.content.x),
                self.geometry.content.height,
            ));
        }
        if column >= divider.right() {
            return Some(Rect::new(
                divider.right(),
                self.geometry.content.y,
                self.geometry
                    .content
                    .right()
                    .saturating_sub(divider.right()),
                self.geometry.content.height,
            ));
        }
        None
    }

    pub(super) fn selected_text(&self) -> String {
        let Some(selection) = self.text_selection else {
            return String::new();
        };
        let ((start_x, start_y), (end_x, end_y)) = selection.ordered_endpoints();
        let mut selected = String::new();
        for row in start_y..=end_y {
            let first = if row == start_y {
                start_x
            } else {
                selection.pane.x
            };
            let last = if row == end_y {
                end_x
            } else {
                selection.pane.right().saturating_sub(1)
            };
            let mut line = String::new();
            if let Some(cells) = self.rendered_cells.get(row as usize) {
                for column in first..=last {
                    if let Some(symbol) = cells.get(column as usize) {
                        line.push(*symbol);
                    }
                }
            }
            let trimmed = line.trim_end();
            selected.push_str(trimmed);
            if row != end_y {
                selected.push('\n');
            }
        }
        selected
    }

    /// A check's own link is the run it describes; anywhere else in the pull
    /// request view the pull request itself is what the reader is looking at.
    pub(super) fn github_url_for_selection(&self) -> Option<&str> {
        if self.view != View::PullRequests {
            return None;
        }
        let check = self
            .selected_pull_request_check()
            .map(|check| check.link.as_str())
            .filter(|link| !link.is_empty());
        check.or_else(|| {
            self.pull_request
                .as_ref()
                .map(|pull_request| pull_request.url.as_str())
                .filter(|url| !url.is_empty())
        })
    }

    pub(super) fn open_selection_on_github(&mut self, effects: &mut Vec<AppEffect>, now: Instant) {
        let target = match self.view {
            View::Changes => self.branch_open_target(&self.status.branch.head),
            View::History => self
                .selected_commit()
                .and_then(|commit| self.commit_open_target(&commit.id)),
            View::PullRequests => self
                .github_url_for_selection()
                .map(|url| OpenTarget::Browser(url.to_owned())),
        };
        match target {
            Some(OpenTarget::Browser(url)) => {
                self.show_toast(format!("Opening {url}"), ToastLevel::Info, now);
                effects.push(AppEffect::Open(OpenTarget::Browser(url)));
            }
            None => self.show_toast(
                "Nothing to open on GitHub for this selection".to_owned(),
                ToastLevel::Error,
                now,
            ),
        }
    }
}
