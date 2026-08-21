#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(super) fn handle_modal_key(
        &mut self,
        modal: Modal,
        key: KeyEvent,
        now: Instant,
    ) -> Vec<AppEffect> {
        match modal {
            modal @ (Modal::Help { .. }
            | Modal::Commit { .. }
            | Modal::Prompt { .. }
            | Modal::Confirm { .. }
            | Modal::Conflict { .. }
            | Modal::Branches { .. }
            | Modal::HistoryBranches { .. }
            | Modal::CompareBranches { .. }
            | Modal::Stashes { .. }) => self.handle_form_modal_key(modal, key, now),
            modal => self.handle_picker_modal_key(modal, key, now),
        }
    }
}
