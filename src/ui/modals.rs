#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn draw_modal(frame: &mut Frame<'_>, app: &mut App, theme: &Theme) {
    if matches!(app.modal, Some(Modal::Help { .. })) {
        draw_help(frame, app, theme);
        return;
    }
    draw_modal_content(frame, app, theme);
}

#[expect(
    clippy::too_many_lines,
    reason = "the exhaustive dispatcher keeps every modal and its hit targets together"
)]
pub(super) fn draw_modal_content(frame: &mut Frame<'_>, app: &mut App, theme: &Theme) {
    let mut list_hits = Vec::new();
    let mut list_len = 0;
    let mut list_max_scroll = 0;
    let mut list = ModalList::new(
        &mut list_hits,
        &mut list_len,
        &mut list_max_scroll,
        app.modal_scroll,
        app.modal_free_scroll,
    );
    match app.modal.as_ref() {
        None | Some(Modal::Help { .. }) => {}
        Some(Modal::Commit { input, amend }) => {
            let (input, amend) = (input.clone(), *amend);
            draw_commit(
                frame,
                &mut app.geometry.modal_action_hits,
                &input,
                amend,
                theme,
            );
        }
        Some(Modal::PullRequestReviewComment { input, target }) => {
            let title = match target {
                crate::app::PullRequestReviewTarget::Line(_) => " Review line ",
                crate::app::PullRequestReviewTarget::File(_) => " Review file ",
                crate::app::PullRequestReviewTarget::Reply(_) => " Reply to review thread ",
                crate::app::PullRequestReviewTarget::Edit { .. } => " Edit review comment ",
            };
            draw_review_editor(
                frame,
                &mut app.geometry.modal_action_hits,
                title,
                input,
                None,
                theme,
            );
        }
        Some(Modal::PullRequestReviewThreadActions { items, selected }) => {
            draw_review_thread_actions(
                frame,
                &mut app.geometry.modal_action_hits,
                items,
                *selected,
                &mut list,
                theme,
            );
        }
        Some(Modal::PullRequestReviewSubmit { input, decision }) => {
            draw_review_editor(
                frame,
                &mut app.geometry.modal_action_hits,
                " Submit review ",
                input,
                Some(*decision),
                theme,
            );
        }
        Some(Modal::Prompt { title, input, .. }) => {
            draw_prompt(frame, title, input, theme);
        }
        Some(Modal::PullRequestActions {
            title,
            items,
            selected,
        }) => draw_pr_actions(
            frame,
            &mut app.geometry.modal_action_hits,
            title,
            items,
            *selected,
            &mut list,
            theme,
        ),
        Some(Modal::Confirm { title, message, .. }) => {
            let (title, message) = (title.clone(), message.clone());
            draw_confirm(
                frame,
                &mut app.geometry.modal_action_hits,
                &title,
                &message,
                theme,
            );
        }
        Some(Modal::Branches {
            items,
            selected,
            query,
            loading,
            ..
        }) => draw_branches(
            frame, items, *selected, query, *loading, app, &mut list, theme,
        ),
        Some(Modal::HistoryBranches {
            items,
            selected,
            query,
            loading,
        }) => draw_history_branches(frame, items, *selected, query, *loading, &mut list, theme),
        Some(Modal::CompareBranches {
            items,
            selected,
            query,
            loading,
        }) => draw_compare_branches(frame, items, *selected, query, *loading, &mut list, theme),
        Some(Modal::Stashes {
            items,
            selected,
            query,
            loading,
        }) => draw_stashes(frame, items, *selected, query, *loading, &mut list, theme),
        Some(modal @ Modal::Projects { .. }) => {
            draw_ssh_project_modal(
                frame,
                modal,
                &mut app.geometry.project_collapse_hits,
                app.ssh_context.as_ref(),
                app.project_machine_focus,
                &mut app.geometry.modal_action_hits,
                &mut list,
                theme,
            );
        }
        Some(Modal::PullRequestRepositories {
            items,
            selected,
            query,
            loading,
        }) => draw_pull_request_repositories(
            frame, items, *selected, query, *loading, &mut list, theme,
        ),
        Some(Modal::CommandPalette { query, selected }) => {
            let query = query.clone();
            let selected = *selected;
            draw_palette(frame, app, &query, selected, &mut list, theme);
        }
        Some(Modal::Themes { selected, .. }) => {
            draw_theme_picker(frame, *selected, app.theme_name, &mut list, theme);
        }
        Some(Modal::Appearances { selected, .. }) => {
            draw_appearance_picker(frame, *selected, app.appearance_choice, &mut list, theme);
        }
        Some(Modal::Conflict { change }) => {
            draw_conflict(frame, &mut app.geometry.modal_action_hits, change, theme);
        }
    }
    app.geometry.modal_list_hits = list_hits;
    app.geometry.modal_list_len = list_len;
    app.geometry.modal_list_max_scroll = list_max_scroll;
}

pub(super) fn draw_help(frame: &mut Frame<'_>, app: &mut App, theme: &Theme) {
    let (selected, mut scroll, hover) = match &app.modal {
        Some(Modal::Help {
            selected,
            scroll,
            hover,
        }) => (*selected, *scroll, *hover),
        _ => return,
    };
    let area = centered_rect(
        72,
        34.min(frame.area().height.saturating_sub(2)),
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(" Keyboard Shortcuts ", theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let list_height = inner.height.saturating_sub(1) as usize;
    let display_selected = help_display_index(selected);
    if app.modal_free_scroll {
        scroll = scroll.min(HELP_ROWS.len().saturating_sub(list_height));
    } else {
        ensure_offset(&mut scroll, display_selected, list_height, HELP_ROWS.len());
    }
    let mut hits = Vec::new();
    let end = (scroll + list_height).min(HELP_ROWS.len());
    for (y, (display, row)) in (inner.y..inner.y.saturating_add(cells(list_height)))
        .zip(HELP_ROWS.iter().enumerate().take(end).skip(scroll))
    {
        match row {
            HelpRow::Section(title) => {
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        (*title).to_owned(),
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    )))
                    .style(Style::default().bg(theme.panel_alt)),
                    Rect::new(inner.x, y, inner.width, 1),
                );
            }
            HelpRow::Spacer => {
                frame.render_widget(
                    Paragraph::new(" ").style(Style::default().bg(theme.panel)),
                    Rect::new(inner.x, y, inner.width, 1),
                );
            }
            HelpRow::Shortcut { keys, description } => {
                let Some(index) = help_shortcut_index_at(display) else {
                    continue;
                };
                let background = if index == selected {
                    theme.selected
                } else if hover == Some(index) {
                    theme.panel_alt
                } else {
                    theme.panel
                };
                frame.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(
                            format!("{keys:<22}"),
                            Style::default().fg(theme.modified).bg(background),
                        ),
                        Span::styled(
                            (*description).to_owned(),
                            Style::default().fg(theme.text).bg(background),
                        ),
                    ]))
                    .style(Style::default().bg(background)),
                    Rect::new(inner.x, y, inner.width, 1),
                );
                hits.push(HelpHit {
                    area: Rect::new(inner.x, y, inner.width, 1),
                    index,
                });
            }
        }
    }
    if let Some(Modal::Help { scroll: stored, .. }) = &mut app.modal {
        *stored = scroll;
    }
    app.geometry.help_hits = hits;
    draw_modal_hint(frame, area, "j/k select · Esc close", theme);
}

pub(super) fn draw_commit(
    frame: &mut Frame<'_>,
    hits: &mut Vec<(Rect, ModalAction)>,
    input: &crate::app::TextBuffer,
    amend: bool,
    theme: &Theme,
) {
    let width = frame.area().width.saturating_sub(12).min(76);
    let area = centered_rect(width, 12, frame.area());
    frame.render_widget(Clear, area);
    let block = modal_block(
        if amend {
            " Amend Commit "
        } else {
            " Commit Staged Changes "
        },
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let input_area = Rect::new(
        inner.x,
        inner.y + 1,
        inner.width,
        inner.height.saturating_sub(3),
    );
    frame.render_widget(
        Paragraph::new(input.value.as_str())
            .style(Style::default().fg(theme.text).bg(theme.panel_alt))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_focus)),
            ),
        input_area,
    );
    set_text_cursor(frame, input_area.inner(Margin::new(1, 1)), input, true);
    let buttons = Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1);
    let primary_label = if amend { "Amend" } else { "Commit" };
    let cancel_label = "Cancel";
    let mode_label = if amend { "Tab: New" } else { "Tab: Amend" };
    let cancel_width = u16::try_from(cancel_label.width().saturating_add(2))
        .unwrap_or(8)
        .min(buttons.width.saturating_sub(3));
    let arrow_width = u16::try_from(mode_label.width().saturating_add(2))
        .unwrap_or(12)
        .min(buttons.width.saturating_sub(cancel_width));
    let label_width = buttons
        .width
        .saturating_sub(cancel_width.saturating_add(arrow_width));
    let cancel_area = Rect::new(buttons.x, buttons.y, cancel_width, 1);
    let label_area = Rect::new(
        buttons.x.saturating_add(cancel_width),
        buttons.y,
        label_width,
        1,
    );
    let arrow_area = Rect::new(
        buttons
            .x
            .saturating_add(cancel_width.saturating_add(label_width)),
        buttons.y,
        arrow_width,
        1,
    );
    frame.render_widget(
        Paragraph::new("").style(Style::default().bg(theme.panel_alt)),
        buttons,
    );
    frame.render_widget(
        Paragraph::new(cancel_label)
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme.muted).bg(theme.panel_alt)),
        cancel_area,
    );
    frame.render_widget(
        Paragraph::new(primary_label)
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(theme.accent)
                    .bg(theme.panel_alt)
                    .add_modifier(Modifier::BOLD),
            ),
        label_area,
    );
    frame.render_widget(
        Paragraph::new(mode_label)
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme.modified).bg(theme.panel_alt)),
        arrow_area,
    );
    hits.push((cancel_area, ModalAction::CommitCancel));
    hits.push((label_area, ModalAction::CommitSubmit));
    hits.push((arrow_area, ModalAction::CommitToggleAmend));
}

pub(super) fn draw_prompt(
    frame: &mut Frame<'_>,
    title: &str,
    input: &crate::app::TextBuffer,
    theme: &Theme,
) {
    let area = centered_rect(
        frame.area().width.saturating_sub(14).min(68),
        7,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(&format!(" {title} "), theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let input_area = Rect::new(inner.x, inner.y + 1, inner.width, 1);
    frame.render_widget(
        Paragraph::new(input.value.as_str())
            .style(Style::default().fg(theme.text).bg(theme.panel_alt)),
        input_area,
    );
    set_text_cursor(frame, input_area, input, false);
    draw_modal_hint(frame, area, "Enter accept   Esc cancel", theme);
}

#[expect(
    clippy::too_many_arguments,
    reason = "the picker receives its modal state, hit targets, list geometry, and palette"
)]
fn draw_pr_actions(
    frame: &mut Frame<'_>,
    hits: &mut Vec<(Rect, ModalAction)>,
    title: &str,
    items: &[PrActionItem],
    selected: usize,
    list: &mut ModalList<'_>,
    theme: &Theme,
) {
    let width = items
        .iter()
        .map(|item| item.label().width())
        .max()
        .unwrap_or(24)
        .saturating_add(6);
    let height = items.len().saturating_add(3);
    let area = centered_rect(
        u16::try_from(width).unwrap_or(72).min(72),
        u16::try_from(height).unwrap_or(20).min(20),
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(&format!(" {title} "), theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let visible = inner.height.saturating_sub(1) as usize;
    let offset = list.offset(selected, visible, items.len());
    for (row, (index, item)) in items
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible)
        .enumerate()
    {
        let active = index == selected;
        let row = Rect::new(
            inner.x,
            inner.y.saturating_add(u16::try_from(row).unwrap_or(0)),
            inner.width,
            1,
        );
        frame.render_widget(
            Paragraph::new(format!("  {}", item.label())).style(if active {
                Style::default().fg(theme.text).bg(theme.selected)
            } else {
                Style::default().fg(theme.text).bg(theme.panel)
            }),
            row,
        );
        hits.push((row, ModalAction::PullRequestAction(index)));
        list.hit(row, index);
    }
    draw_modal_hint(frame, area, "j/k select   Enter open   Esc cancel", theme);
}

pub(super) fn draw_confirm(
    frame: &mut Frame<'_>,
    hits: &mut Vec<(Rect, ModalAction)>,
    title: &str,
    message: &str,
    theme: &Theme,
) {
    let line_count = message.lines().count().max(1);
    let height = u16::try_from(line_count.saturating_add(6))
        .unwrap_or(9)
        .clamp(9, frame.area().height.saturating_sub(4).max(9));
    let area = centered_rect(
        frame.area().width.saturating_sub(14).min(72),
        height,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = modal_block(&format!(" {title} "), theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(message)
            .style(Style::default().fg(theme.text))
            .wrap(Wrap { trim: false }),
        Rect::new(
            inner.x,
            inner.y + 1,
            inner.width,
            inner.height.saturating_sub(3),
        ),
    );
    let buttons = Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1);
    #[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
    let yes_width = buttons.width / 2;
    let no_width = buttons.width.saturating_sub(yes_width);
    let yes_area = Rect::new(buttons.x, buttons.y, yes_width, 1);
    let no_area = Rect::new(buttons.x.saturating_add(yes_width), buttons.y, no_width, 1);
    frame.render_widget(
        Paragraph::new("").style(Style::default().bg(theme.panel_alt)),
        buttons,
    );
    frame.render_widget(
        Paragraph::new("Yes").alignment(Alignment::Center).style(
            Style::default()
                .fg(theme.accent)
                .bg(theme.panel_alt)
                .add_modifier(Modifier::BOLD),
        ),
        yes_area,
    );
    frame.render_widget(
        Paragraph::new("No")
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme.muted).bg(theme.panel_alt)),
        no_area,
    );
    hits.push((yes_area, ModalAction::ConfirmYes));
    hits.push((no_area, ModalAction::ConfirmNo));
}
