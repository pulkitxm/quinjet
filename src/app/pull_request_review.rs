#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl App {
    pub(super) fn reset_pull_request_review(&mut self) {
        self.pull_request_review = PullRequestReviewSnapshot::default();
        self.pull_request_review_loading = false;
        self.pull_request_review_mutating = false;
        self.pull_request_review_error = None;
        self.pull_request_review_cursor = None;
        self.pull_request_review_line_threads.clear();
        self.pull_request_review_generation = self.pull_request_review_generation.wrapping_add(1);
    }

    pub(super) fn decorate_pull_request_review(&mut self) {
        if !self.review_document_active() {
            return;
        }
        let Some(path) = self.pull_request_single_file.as_ref() else {
            return;
        };
        self.document
            .lines
            .retain(|line| line.kind != DiffLineKind::Review);
        self.pull_request_review_line_threads.clear();
        let threads = self
            .pull_request_review
            .threads
            .iter()
            .filter(|thread| &thread.path == path)
            .cloned()
            .collect::<Vec<_>>();
        if threads.is_empty() {
            self.invalidate_diff_rows();
            return;
        }
        let source = std::mem::take(&mut self.document.lines);
        let mut lines = Vec::with_capacity(source.len().saturating_add(threads.len()));
        for line in source {
            let anchored = threads
                .iter()
                .filter(|thread| review_thread_matches_line(thread, &line))
                .collect::<Vec<_>>();
            lines.push(line);
            for thread in anchored {
                let start = lines.len();
                lines.extend(review_thread_lines(thread));
                for index in start..lines.len() {
                    drop(
                        self.pull_request_review_line_threads
                            .insert(index, thread.id.clone()),
                    );
                }
            }
        }
        self.document.lines = lines;
        self.invalidate_diff_rows();
    }

    pub(super) fn request_pull_request_review(
        &mut self,
        refresh: bool,
        effects: &mut Vec<AppEffect>,
    ) {
        if self.pull_request_review_loading
            || (!refresh && !self.pull_request_review.head_oid.is_empty())
        {
            return;
        }
        let Some(pull_request) = self.selected_pull_request().cloned() else {
            return;
        };
        self.pull_request_review_generation = self.pull_request_review_generation.wrapping_add(1);
        self.pull_request_review_loading = true;
        self.pull_request_review_mutating = false;
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::LoadPullRequestReview {
                generation: self.pull_request_review_generation,
                pull_request: Box::new(pull_request),
            },
        )));
    }

    pub(super) fn queue_pull_request_review_operation(
        &mut self,
        operation: PullRequestReviewOperation,
        effects: &mut Vec<AppEffect>,
    ) {
        if self.pull_request_review_loading {
            return;
        }
        let Some(pull_request) = self.selected_pull_request().cloned() else {
            return;
        };
        self.pull_request_review_generation = self.pull_request_review_generation.wrapping_add(1);
        self.pull_request_review_loading = true;
        self.pull_request_review_mutating = true;
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::OperatePullRequestReview {
                generation: self.pull_request_review_generation,
                pull_request: Box::new(pull_request),
                operation,
            },
        )));
    }

    pub(crate) fn review_surface_active(&self) -> bool {
        self.review_document_active() && self.focus == Focus::Content
    }

    fn review_document_active(&self) -> bool {
        self.view == View::PullRequests
            && (self.pull_request_section == PullRequestSection::Files
                || (self.pull_request_section == PullRequestSection::Stack
                    && self.stack_inspector.section == StackMemberSection::Files
                    && !self.stack_inspector.diff_open))
            && self.pull_request_file_view == PullRequestFileView::SingleFile
            && self.pull_request_single_file.is_some()
    }

    fn review_anchors(&self) -> Vec<(usize, PullRequestReviewAnchor)> {
        let Some(path) = self.pull_request_single_file.as_ref() else {
            return Vec::new();
        };
        self.document
            .lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| {
                let (number, side) = match line.kind {
                    DiffLineKind::Removed => (line.old_line, PullRequestReviewSide::Left),
                    DiffLineKind::Added | DiffLineKind::Context => {
                        (line.new_line, PullRequestReviewSide::Right)
                    }
                    _ => return None,
                };
                Some((
                    index,
                    PullRequestReviewAnchor {
                        path: path.clone(),
                        line: number?,
                        side,
                    },
                ))
            })
            .collect()
    }

    pub(super) fn move_review_cursor(&mut self, direction: isize) {
        let anchors = self.review_anchors();
        if anchors.is_empty() {
            self.pull_request_review_cursor = None;
            return;
        }
        let current = self
            .pull_request_review_cursor
            .as_ref()
            .and_then(|cursor| anchors.iter().position(|(_, anchor)| anchor == cursor));
        let next = match (current, direction.is_negative()) {
            (Some(index), true) => index.saturating_sub(1),
            (Some(index), false) => index.saturating_add(1).min(anchors.len() - 1),
            (None, true) => anchors.len() - 1,
            (None, false) => 0,
        };
        let Some((line_index, anchor)) = anchors.get(next) else {
            return;
        };
        self.pull_request_review_cursor = Some(anchor.clone());
        self.content_scroll = line_index.saturating_sub(2);
    }

    fn ensure_review_cursor(&mut self) -> Option<PullRequestReviewAnchor> {
        let valid = self
            .pull_request_review_cursor
            .as_ref()
            .is_some_and(|cursor| {
                self.review_anchors()
                    .iter()
                    .any(|(_, anchor)| anchor == cursor)
            });
        if !valid {
            self.pull_request_review_cursor = None;
            self.move_review_cursor(1);
        }
        self.pull_request_review_cursor.clone()
    }

    pub(super) fn selected_review_thread(&self) -> Option<&PullRequestReviewThread> {
        let cursor = self.pull_request_review_cursor.as_ref()?;
        self.pull_request_review.threads.iter().find(|thread| {
            thread.path == cursor.path
                && thread.side == cursor.side
                && thread.line.or(thread.original_line) == Some(cursor.line)
        })
    }

    pub(super) fn open_review_comment(&mut self, file: bool) {
        let target = if file {
            let Some(path) = self.pull_request_single_file.clone() else {
                return;
            };
            PullRequestReviewTarget::File(path)
        } else {
            let Some(anchor) = self.ensure_review_cursor() else {
                return;
            };
            PullRequestReviewTarget::Line(anchor)
        };
        self.modal = Some(Modal::PullRequestReviewComment {
            input: TextBuffer::default(),
            target,
        });
    }

    pub(super) fn open_review_reply(&mut self) {
        let Some(thread_id) = self
            .selected_review_thread()
            .filter(|thread| thread.viewer_can_reply)
            .map(|thread| thread.id.clone())
        else {
            return;
        };
        self.modal = Some(Modal::PullRequestReviewComment {
            input: TextBuffer::default(),
            target: PullRequestReviewTarget::Reply(thread_id),
        });
    }

    pub(super) fn open_review_thread_actions(&mut self, thread_id: &str) {
        let Some(thread) = self
            .pull_request_review
            .threads
            .iter()
            .find(|thread| thread.id == thread_id)
            .cloned()
        else {
            return;
        };
        self.pull_request_review_cursor =
            thread
                .line
                .or(thread.original_line)
                .map(|line| PullRequestReviewAnchor {
                    path: thread.path.clone(),
                    line,
                    side: thread.side,
                });
        let mut items = Vec::new();
        if thread.viewer_can_reply {
            items.push(PullRequestReviewThreadAction::Reply {
                thread_id: thread.id.clone(),
            });
        }
        if let Some(comment) = thread.comments.last() {
            items.push(PullRequestReviewThreadAction::CopyComment {
                body: comment.body.clone(),
            });
            if !comment.url.is_empty() {
                items.push(PullRequestReviewThreadAction::OpenComment {
                    url: comment.url.clone(),
                });
            }
            if comment.viewer_can_update {
                items.push(PullRequestReviewThreadAction::EditComment {
                    comment_id: comment.id.clone(),
                    body: comment.body.clone(),
                });
            }
            if comment.viewer_can_delete {
                items.push(PullRequestReviewThreadAction::DeleteComment {
                    comment_id: comment.id.clone(),
                });
            }
        }
        if thread.is_resolved && thread.viewer_can_unresolve {
            items.push(PullRequestReviewThreadAction::Reopen {
                thread_id: thread.id,
            });
        } else if !thread.is_resolved && thread.viewer_can_resolve {
            items.push(PullRequestReviewThreadAction::Resolve {
                thread_id: thread.id,
            });
        }
        if !items.is_empty() {
            self.modal = Some(Modal::PullRequestReviewThreadActions { items, selected: 0 });
        }
    }

    pub(super) fn handle_review_thread_action(
        &mut self,
        action: PullRequestReviewThreadAction,
        effects: &mut Vec<AppEffect>,
        now: Instant,
    ) {
        match action {
            PullRequestReviewThreadAction::Reply { thread_id } => {
                self.modal = Some(Modal::PullRequestReviewComment {
                    input: TextBuffer::default(),
                    target: PullRequestReviewTarget::Reply(thread_id),
                });
            }
            PullRequestReviewThreadAction::CopyComment { body } => {
                effects.push(AppEffect::Copy(body));
            }
            PullRequestReviewThreadAction::OpenComment { url } => {
                self.open_link(url, effects, now);
            }
            PullRequestReviewThreadAction::EditComment { comment_id, body } => {
                self.modal = Some(Modal::PullRequestReviewComment {
                    input: TextBuffer::new(body),
                    target: PullRequestReviewTarget::Edit { comment_id },
                });
            }
            PullRequestReviewThreadAction::DeleteComment { comment_id } => {
                self.modal = Some(Modal::Confirm {
                    title: "Delete Review Comment?".to_owned(),
                    message: "Permanently delete this review comment?".to_owned(),
                    action: ConfirmAction::PullRequestReview(
                        PullRequestReviewOperation::DeleteComment { comment_id },
                    ),
                });
            }
            PullRequestReviewThreadAction::Resolve { thread_id } => {
                self.queue_pull_request_review_operation(
                    PullRequestReviewOperation::Resolve { thread_id },
                    effects,
                );
            }
            PullRequestReviewThreadAction::Reopen { thread_id } => {
                self.queue_pull_request_review_operation(
                    PullRequestReviewOperation::Unresolve { thread_id },
                    effects,
                );
            }
        }
    }
}

fn review_thread_matches_line(
    thread: &PullRequestReviewThread,
    line: &crate::git::diff::DiffLine,
) -> bool {
    if thread.subject == PullRequestReviewThreadSubject::File {
        return line.kind == DiffLineKind::FileHeader;
    }
    let number = thread.line.or(thread.original_line);
    match thread.side {
        PullRequestReviewSide::Left => number == line.old_line,
        PullRequestReviewSide::Right | PullRequestReviewSide::Unknown => number == line.new_line,
    }
}

fn review_thread_lines(thread: &PullRequestReviewThread) -> Vec<crate::git::diff::DiffLine> {
    let state = if thread.is_resolved {
        "resolved"
    } else if thread.is_outdated {
        "outdated"
    } else {
        "open"
    };
    let mut lines = vec![crate::git::diff::DiffLine::review(format!(
        "review thread · {state}"
    ))];
    for comment in &thread.comments {
        for (index, body) in comment.body.lines().enumerate() {
            let prefix = if index == 0 {
                format!("@{}: ", comment.author)
            } else {
                "  ".to_owned()
            };
            lines.push(crate::git::diff::DiffLine::review(format!(
                "{prefix}{body}"
            )));
        }
    }
    lines
}
