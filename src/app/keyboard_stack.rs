#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(super) fn handle_stack_inspector_key(
        &mut self,
        key: KeyEvent,
        now: Instant,
    ) -> Option<Vec<AppEffect>> {
        if self.view != View::PullRequests || self.pull_request_stack.is_none() {
            return None;
        }
        let mut effects = Vec::new();
        match key.code {
            KeyCode::Char('1') => {
                self.select_stack_member_section(StackMemberSection::Files, &mut effects);
            }
            KeyCode::Char('2') => {
                self.select_stack_member_section(StackMemberSection::Summary, &mut effects);
            }
            KeyCode::Char('3') => {
                self.select_stack_member_section(StackMemberSection::Conversation, &mut effects);
            }
            KeyCode::Char('4') => {
                self.select_stack_member_section(StackMemberSection::Checks, &mut effects);
            }
            KeyCode::Char('5') => {
                self.select_stack_member_section(StackMemberSection::Commits, &mut effects);
            }
            KeyCode::Char('d') => self.open_pull_request_stack_diff(&mut effects),
            KeyCode::Char('r') => self.open_stack_member_review(&mut effects),
            KeyCode::Char('p' | '[') => {
                let _ = self.move_pull_request_stack_cursor(-1, false, now);
            }
            KeyCode::Char('n' | ']') => {
                let _ = self.move_pull_request_stack_cursor(1, false, now);
            }
            KeyCode::Char('o') => self.open_selection_on_github(&mut effects, now),
            KeyCode::Char('t' | 'T') => {
                self.inspect_pull_request_stack_tip(now, &mut effects);
                self.set_focus(Focus::Content, &mut effects);
            }
            KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                let _ = self.move_pull_request_stack_cursor(-1, true, now);
            }
            KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                let _ = self.move_pull_request_stack_cursor(1, true, now);
            }
            KeyCode::Esc if self.close_pull_request_stack_diff(&mut effects) => {}
            KeyCode::Char('P' | 'F' | 'S' | 'z' | '/') => {}
            _ => return None,
        }
        Some(effects)
    }
}
