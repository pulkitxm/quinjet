#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct AccountPicker {
    pub(crate) items: Vec<String>,
    pub(crate) selected: usize,
    pub(crate) current: Option<String>,
    pub(crate) message: Option<String>,
    pub(crate) ready: bool,
}

impl App {
    pub(super) fn receive_accounts(
        &mut self,
        generation: u64,
        result: Result<crate::git::accounts::Accounts, String>,
    ) {
        if generation != self.branch_generation {
            return;
        }
        let Some(Modal::Accounts(picker)) = self.modal.as_mut() else {
            return;
        };
        picker.ready = true;
        match result {
            Ok(accounts) => {
                picker.selected = accounts
                    .logins
                    .iter()
                    .position(|login| Some(login) == accounts.selected.as_ref())
                    .unwrap_or_default();
                picker.items = accounts.logins;
                picker.current = accounts.selected;
                picker.message = None;
            }
            Err(error) => picker.message = Some(error),
        }
    }

    pub(super) fn handle_account_key(
        &mut self,
        mut picker: AccountPicker,
        key: KeyEvent,
    ) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        match key.code {
            KeyCode::Esc => return effects,
            KeyCode::Up | KeyCode::Char('k') => {
                picker.selected = previous_list_index(picker.selected, picker.items.len());
            }
            KeyCode::Down | KeyCode::Char('j') => {
                picker.selected = next_list_index(picker.selected, picker.items.len());
            }
            KeyCode::Home => picker.selected = 0,
            KeyCode::End => picker.selected = picker.items.len().saturating_sub(1),
            KeyCode::Enter if picker.ready => {
                if let Some(login) = picker.items.get(picker.selected) {
                    self.queue_operation(GitOperation::SwitchAccount(login.clone()), &mut effects);
                    return effects;
                }
            }
            _ => {}
        }
        self.modal = Some(Modal::Accounts(picker));
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_accounts_cannot_switch_and_cancel_ignores_late_results() {
        let mut app = App::new("/tmp/repo", "repo");
        let now = Instant::now();
        app.execute_palette(PaletteCommand::SwitchAccount, &mut Vec::new(), now);
        let generation = app.branch_generation;
        for key in [KeyCode::Up, KeyCode::Down, KeyCode::Enter] {
            assert!(
                app.handle_key(KeyEvent::new(key, KeyModifiers::NONE), now)
                    .is_empty()
            );
        }
        app.receive_accounts(generation, Err("Account lookup failed".to_owned()));
        assert!(matches!(&app.modal, Some(Modal::Accounts(picker))
            if picker.ready && picker.message.as_deref() == Some("Account lookup failed")));
        assert!(
            app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now)
                .is_empty()
        );
        drop(app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), now));
        app.receive_accounts(generation, Err("Late failure".to_owned()));
        assert!(app.modal.is_none());
        assert!(app.toast.is_none());
    }

    #[test]
    fn stale_results_do_not_replace_the_new_account_list() {
        let mut app = App::new("/tmp/repo", "repo");
        let now = Instant::now();
        app.execute_palette(PaletteCommand::SwitchAccount, &mut Vec::new(), now);
        let generation = app.branch_generation;
        drop(app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), now));
        app.execute_palette(PaletteCommand::SwitchAccount, &mut Vec::new(), now);
        app.receive_accounts(generation, Err("Stale failure".to_owned()));
        assert!(matches!(&app.modal, Some(Modal::Accounts(picker)) if !picker.ready));
        app.receive_accounts(
            app.branch_generation,
            Ok(crate::git::accounts::Accounts {
                logins: Vec::new(),
                selected: None,
                name: String::new(),
                email: String::new(),
            }),
        );
        assert!(
            app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now)
                .is_empty()
        );
        assert!(matches!(&app.modal, Some(Modal::Accounts(picker)) if picker.ready));
    }
}
