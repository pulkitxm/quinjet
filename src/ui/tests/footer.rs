use super::*;

#[test]
fn footer_shows_repository_account_at_the_right_even_during_operations() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut app = App::new("/tmp/repo", "repo");
    app.status.branch.head = "a-very-long-branch-name-that-must-not-cover-the-account".to_owned();
    let theme = Theme::default();
    let mut terminal = Terminal::new(TestBackend::new(80, 2)).unwrap();
    for (account, busy, expected) in [
        (Some("personal"), None, "@personal"),
        (
            Some("company"),
            Some("Switching repository account"),
            "@company",
        ),
        (None, None, "Account unset"),
    ] {
        app.status.account = account.map(str::to_owned);
        app.busy = busy.map(str::to_owned);
        let mut links = Vec::new();
        terminal
            .draw(|frame| {
                draw_footer(
                    frame,
                    frame.area(),
                    &app,
                    &theme,
                    &mut links,
                    &mut Vec::new(),
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let row: String = (0..80).map(|x| buffer[(x, 1)].symbol()).collect();
        assert!(row.ends_with(&format!("{expected} ")), "{row}");
        assert!(!row.contains("Ctrl+G"), "{row}");
        let account_start = 80 - cells(format!(" {expected} ").width());
        assert!(links.iter().all(|link| link.area.right() <= account_start));
    }
}
