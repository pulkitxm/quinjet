#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
pub(super) fn progress_bar(percent: u16, width: usize) -> String {
    let filled = usize::from(percent.min(100)).saturating_mul(width) / 100;
    format!(
        "{}{}",
        "█".repeat(filled),
        "░".repeat(width.saturating_sub(filled))
    )
}

#[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
pub(super) fn draw_toast(frame: &mut Frame<'_>, message: &str, level: ToastLevel, theme: &Theme) {
    let width = (cells(message.width()) + 6)
        .min(frame.area().width.saturating_sub(4))
        .max(24);
    let height = ((cells(message.width()) / width.max(1)) + 3).min(7);
    let area = Rect::new(
        frame.area().right().saturating_sub(width + 2),
        frame.area().bottom().saturating_sub(height + 3),
        width,
        height,
    );
    let color = match level {
        ToastLevel::Info => theme.accent,
        ToastLevel::Success => theme.success,
        ToastLevel::Error => theme.error,
    };
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(message)
            .style(Style::default().fg(theme.text).bg(theme.panel_alt))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(color)),
            ),
        area,
    );
}

pub(super) fn draw_modal_hint(frame: &mut Frame<'_>, area: Rect, hint: &str, theme: &Theme) {
    frame.render_widget(
        Paragraph::new(hint)
            .alignment(Alignment::Right)
            .style(Style::default().fg(theme.muted).bg(theme.panel)),
        Rect::new(
            area.x + 2,
            area.bottom().saturating_sub(2),
            area.width.saturating_sub(4),
            1,
        ),
    );
}
