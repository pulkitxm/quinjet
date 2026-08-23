#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn handle_help_key(
    selected: &mut usize,
    hover: &mut Option<usize>,
    key: KeyEvent,
    count: usize,
) -> bool {
    match key.code {
        KeyCode::Esc | KeyCode::Char('?' | 'q') | KeyCode::Enter => return false,
        KeyCode::Up | KeyCode::Char('k') => {
            *selected = previous_list_index(*selected, count);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            *selected = next_list_index(*selected, count);
        }
        KeyCode::PageUp => {
            *selected = selected.saturating_sub(10);
        }
        KeyCode::PageDown => {
            *selected = (*selected + 10).min(count.saturating_sub(1));
        }
        KeyCode::Home | KeyCode::Char('g') => {
            *selected = 0;
        }
        KeyCode::End | KeyCode::Char('G') => {
            *selected = count.saturating_sub(1);
        }
        _ => return true,
    }
    *hover = None;
    true
}
