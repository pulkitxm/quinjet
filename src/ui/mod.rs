use std::collections::{HashMap, HashSet};
use std::num::NonZeroU16;
use std::ops::Range;
use std::path::Path;

use ratatui::Frame;
use ratatui::buffer::{Buffer, CellDiffOption};
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget, Wrap};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::{
    App, ChangeRow, ChangeSection, CheckListRow, ContentFileHit, ContentReviewHit, ContentStepHit,
    DiffLayout, Focus, HelpHit, LinkHit, Modal, ModalAction, OpenTarget, PaletteCommand,
    PrActionItem, PrMenuItem, ProjectOpenMode, ProjectRow, PullRequestContentLink,
    PullRequestContentRow, PullRequestSection, PullRequestTreeEntry, RepositoryTabAction,
    RepositoryTabHit, ScmAction, ScmActionHit, ScmMenuItem, SideBySideRow, SidebarHit,
    SidebarHitArea, StackInspectorHit, StackInspectorHitArea, StackMemberSection, ToastLevel,
    UiGeometry, View,
};
use crate::convert::cells;
use crate::date_time::format_relative_timestamp;
use crate::file_icons;
use crate::git::diff::{DiffDocument, DiffLine, DiffLineKind, HighlightSpan, PullRequestDetails};
use crate::git::github::{
    CheckLogLine, CheckLogSeverity, CheckStep, ConversationEntry, ConversationKind,
    GitHubRepository, PullRequest, PullRequestCheckStatus, PullRequestFileStatus,
};
#[cfg(test)]
use crate::git::github::{PullRequestCheck, PullRequestFile, RecentPullRequest};
use crate::git::status::{Change, ChangeArea, ChangeStatus};
use crate::git::{Branch, HistoryBranch, ProjectGroup, Stash};
use crate::ssh::SshContext;
use crate::theme::{AppearanceChoice, Theme, ThemeName};

mod help_catalog;
use help_catalog::{HELP_ROWS, HelpRow};

mod content;
mod diff_render;
mod diff_scrolling;
mod diff_wrapping;
mod feedback;
mod help;
mod horizontal_scrollbar;
mod hover;
mod image_diff;
mod layout;
mod modal_branches;
mod modal_choices;
mod modal_conflict;
mod modal_list;
pub(crate) mod modal_ssh;
mod modal_stashes;
mod modals;
pub(crate) mod pickers;
mod project_picker;
mod prose;
mod pull_request_checks;
mod pull_request_conversation;
mod pull_request_details;
mod pull_request_files;
mod pull_request_overview;
mod pull_request_review;
mod pull_request_stack;
mod pull_request_stack_detail;
mod pull_request_stack_gate;
mod pull_request_stack_rows;
mod pull_request_stack_strip;
mod pull_request_viewport;
mod repository_tabs;
mod side_by_side;
mod sidebar_changes;
mod sidebar_history;
mod sidebar_pull_requests;
mod style;
mod unified_diff;

#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use content::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use diff_render::*;
use feedback::{draw_modal_hint, draw_toast, progress_bar};
pub(crate) use help::{draw_help, help_shortcut_count};
#[cfg(test)]
pub(crate) use help::{help_display_index, help_rows, help_shortcut_index_at};
pub(crate) use image_diff::draw;
pub(crate) use image_diff::picker::{image_picker_helper, initialize_image_picker};
pub(crate) use image_diff::preparation::image_preparation_ready;
use image_diff::{ImageDrawState, draw_image_line, image_side, selected_image_protocol};
use layout::draw_main_divider;
#[cfg(test)]
use layout::draw_text_selection;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use modal_branches::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use modal_choices::*;
use modal_conflict::draw_conflict;
pub(crate) use modal_list::ModalList;
use modal_ssh::draw_ssh_project_modal;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use modal_stashes::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use modals::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use pickers::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use project_picker::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use prose::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use pull_request_checks::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use pull_request_conversation::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use pull_request_details::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use pull_request_files::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use pull_request_overview::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use pull_request_review::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use repository_tabs::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use side_by_side::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use sidebar_changes::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use sidebar_history::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use sidebar_pull_requests::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use style::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use unified_diff::*;

#[cfg(test)]
#[expect(unused_results, reason = "test helpers discard unused return values")]
mod tests;
