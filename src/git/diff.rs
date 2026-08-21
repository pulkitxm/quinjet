use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Serialize;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use unicode_width::UnicodeWidthChar;

use crate::theme::SyntaxColor;

const TAB_WIDTH: usize = 4;
const MAX_SYNTAX_HIGHLIGHT_PATCH_BYTES: usize = 512 * 1024;
const MAX_SYNTAX_HIGHLIGHT_LINE_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DiffLineKind {
    FileHeader,
    FileFooter,
    HunkHeader,
    Context,
    Added,
    Removed,
    Meta,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HighlightSpan {
    pub text: String,
    pub foreground: Option<SyntaxColor>,
    pub bold: bool,
    pub italic: bool,
}

impl HighlightSpan {
    fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            foreground: None,
            bold: false,
            italic: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffLine {
    pub kind: DiffLineKind,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub spans: Vec<HighlightSpan>,
}

impl DiffLine {
    pub(crate) fn text(&self) -> String {
        if self.kind == DiffLineKind::FileHeader {
            self.spans
                .iter()
                .map(|span| span.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            self.spans.iter().map(|span| span.text.as_str()).collect()
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommitDetails {
    pub id: String,
    pub subject: String,
    pub author: String,
    pub author_email: String,
    pub authored_at: String,
    pub committer: String,
    pub committer_email: String,
    pub committed_at: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffLineCounts {
    pub additions: usize,
    pub deletions: usize,
    pub binary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffFileIndexEntry {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub status: String,
    /// Exact per-file totals read from `git diff --numstat` while the index is
    /// built. Known counts let a file header render its real `+n -n` before the
    /// patch for that file has been produced.
    pub counts: Option<DiffLineCounts>,
}

impl DiffFileIndexEntry {
    pub(crate) const fn new(path: PathBuf, old_path: Option<PathBuf>, status: String) -> Self {
        Self {
            path,
            old_path,
            status,
            counts: None,
        }
    }

    fn label(&self) -> String {
        let mut label = self.path.display().to_string();
        if let Some(old_path) = self.old_path.as_ref().filter(|old| *old != &self.path) {
            label.push_str("  · renamed from ");
            label.push_str(&old_path.display().to_string());
        } else if !self.status.is_empty() {
            label.push_str("  · ");
            label.push_str(&self.status);
        }
        if self.counts.is_some_and(|counts| counts.binary) {
            label.push_str("  · binary");
        }
        label
    }

    fn count_spans(&self) -> (String, String) {
        self.counts.map_or_else(
            || ("+··".to_owned(), "-··".to_owned()),
            |counts| {
                (
                    format!("+{}", counts.additions),
                    format!("-{}", counts.deletions),
                )
            },
        )
    }
}

/// Parse `git diff --numstat -z` output into per-path totals. Renames emit an
/// empty path field followed by the pre-image and post-image records, so the
/// scanner has to consume those two extra records instead of assuming one.
pub(crate) fn parse_numstat(output: &[u8]) -> HashMap<PathBuf, DiffLineCounts> {
    let records: Vec<&[u8]> = output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect();
    let mut counts = HashMap::new();
    let mut cursor = 0;
    while cursor < records.len() {
        let Some(record) = records.get(cursor).copied() else {
            break;
        };
        cursor += 1;
        let mut fields = record.splitn(3, |byte| *byte == b'\t');
        let (Some(additions), Some(deletions), Some(path)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let binary = additions == b"-" || deletions == b"-";
        let entry = DiffLineCounts {
            additions: parse_count(additions),
            deletions: parse_count(deletions),
            binary,
        };
        if path.is_empty() {
            let Some(new_path) = records.get(cursor + 1) else {
                break;
            };
            cursor += 2;
            let _ = counts.insert(record_path(new_path), entry);
        } else {
            let _ = counts.insert(record_path(path), entry);
        }
    }
    counts
}

fn parse_count(field: &[u8]) -> usize {
    std::str::from_utf8(field)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default()
}

fn record_path(record: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(record).into_owned())
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffIndex {
    pub title: String,
    pub files: Vec<DiffFileIndexEntry>,
    pub truncated: bool,
    pub commit_details: Option<CommitDetails>,
}

impl DiffIndex {
    pub(crate) fn line_counts(&self) -> DiffLineCounts {
        self.files.iter().filter_map(|file| file.counts).fold(
            DiffLineCounts::default(),
            |total, counts| DiffLineCounts {
                additions: total.additions.saturating_add(counts.additions),
                deletions: total.deletions.saturating_add(counts.deletions),
                binary: total.binary || counts.binary,
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn document(&self, loaded: &HashMap<PathBuf, DiffDocument>) -> DiffDocument {
        self.document_with_visibility(loaded, |_| true)
    }

    pub(crate) fn document_with_visibility(
        &self,
        loaded: &HashMap<PathBuf, DiffDocument>,
        mut visible: impl FnMut(&Path) -> bool,
    ) -> DiffDocument {
        if self.files.is_empty() {
            let mut document = DiffDocument::empty(&self.title, "No file changes to display");
            document.commit_details.clone_from(&self.commit_details);
            return document;
        }

        let mut lines = Vec::with_capacity(self.files.len().saturating_mul(3));
        let mut truncated = self.truncated;
        for file in &self.files {
            let loaded_document = loaded.get(&file.path);
            let show_body = visible(&file.path);
            truncated |= loaded_document.is_some_and(|document| document.truncated);
            let loaded_header = loaded_document.and_then(|document| {
                document
                    .lines
                    .iter()
                    .find(|line| line.kind == DiffLineKind::FileHeader)
            });
            if show_body && let Some(document) = loaded_document.filter(|_| loaded_header.is_some())
            {
                let mut file_lines = document.lines.clone();
                if let Some(label) = file_lines
                    .iter_mut()
                    .find(|line| line.kind == DiffLineKind::FileHeader)
                    .and_then(|header| header.spans.first_mut())
                {
                    label.text = file.label();
                }
                lines.extend(file_lines);
                continue;
            }

            let mut header = index_file_header(file);
            if let Some(loaded_header) = loaded_header {
                for span_index in 1..=2 {
                    if let (Some(target), Some(source)) = (
                        header.spans.get_mut(span_index),
                        loaded_header.spans.get(span_index),
                    ) {
                        target.text.clone_from(&source.text);
                    }
                }
            }
            lines.push(header);
            if show_body {
                if let Some(document) = loaded_document {
                    lines.extend(document.lines.clone());
                } else {
                    lines.push(meta_line(DiffLineKind::Meta, "Loading diff…"));
                }
            } else {
                lines.push(meta_line(
                    DiffLineKind::Meta,
                    if loaded_document.is_some() {
                        "Diff loaded · expand this file to display it"
                    } else {
                        "Expand this file to load its diff"
                    },
                ));
            }
            lines.push(meta_line(DiffLineKind::FileFooter, ""));
        }

        DiffDocument {
            title: self.title.clone(),
            lines,
            truncated,
            commit_details: self.commit_details.clone(),
            pull_request_details: None,
        }
    }
}

fn index_file_header(file: &DiffFileIndexEntry) -> DiffLine {
    let (additions, deletions) = file.count_spans();
    DiffLine {
        kind: DiffLineKind::FileHeader,
        old_line: None,
        new_line: None,
        spans: vec![
            HighlightSpan::plain(file.label()),
            HighlightSpan::plain(additions),
            HighlightSpan::plain(deletions),
        ],
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestDetails {
    pub number: u64,
    pub title: String,
    pub description: String,
    pub author: String,
    pub state: String,
    pub is_draft: bool,
    pub updated_at: String,
    pub url: String,
    pub base_repository: String,
    pub base_ref: String,
    pub base_remotes: Vec<String>,
    pub head_repository: Option<String>,
    pub head_ref: String,
    pub head_remotes: Vec<String>,
    pub is_cross_repository: bool,
    pub changed_files: usize,
    pub additions: usize,
    pub deletions: usize,
    pub selected_file: Option<String>,
    pub selected_file_additions: usize,
    pub selected_file_deletions: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffDocument {
    pub title: String,
    pub lines: Vec<DiffLine>,
    pub truncated: bool,
    pub commit_details: Option<CommitDetails>,
    pub pull_request_details: Option<PullRequestDetails>,
}

impl DiffDocument {
    pub(crate) fn empty(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            lines: vec![DiffLine {
                kind: DiffLineKind::Meta,
                old_line: None,
                new_line: None,
                spans: vec![HighlightSpan::plain(message)],
            }],
            truncated: false,
            commit_details: None,
            pull_request_details: None,
        }
    }

    pub(crate) fn file_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|line| line.kind == DiffLineKind::FileHeader)
            .count()
    }

    pub(crate) fn addition_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|line| line.kind == DiffLineKind::Added)
            .count()
    }

    pub(crate) fn deletion_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|line| line.kind == DiffLineKind::Removed)
            .count()
    }
}

struct HighlightAssets {
    syntaxes: SyntaxSet,
    theme: Theme,
}

static HIGHLIGHT_ASSETS: OnceLock<HighlightAssets> = OnceLock::new();

fn highlight_assets() -> &'static HighlightAssets {
    HIGHLIGHT_ASSETS.get_or_init(|| {
        let syntaxes = two_face::syntax::extra_newlines();
        let themes = ThemeSet::load_defaults();
        let theme = themes
            .themes
            .get("base16-ocean.dark")
            .or_else(|| themes.themes.values().next())
            .cloned()
            .unwrap_or_default();
        HighlightAssets { syntaxes, theme }
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "the draw pass reads better as one top-to-bottom pass"
)]
/// Parse a unified diff and highlight code on the old and new sides independently.
/// Keeping two parser states avoids additions corrupting the old-file syntax state and
/// removals corrupting the new-file state.
pub(crate) fn parse_diff(
    raw: &[u8],
    title: impl Into<String>,
    path_hint: Option<&Path>,
    truncated: bool,
) -> DiffDocument {
    let title = title.into();
    if raw.is_empty() {
        return DiffDocument::empty(title, "No textual diff to display");
    }

    let assets = (raw.len() <= MAX_SYNTAX_HIGHLIGHT_PATCH_BYTES).then(highlight_assets);
    let mut active_path = path_hint.map(Path::to_path_buf);
    let mut old_highlighter = highlighter_for_path(assets, active_path.as_deref());
    let mut new_highlighter = highlighter_for_path(assets, active_path.as_deref());
    let mut old_line = None;
    let mut new_line = None;
    let mut current_file: Option<FileBuilder> = None;
    let mut files = Vec::new();
    let mut lines = Vec::new();

    for raw_line in String::from_utf8_lossy(raw).lines() {
        if let Some(header) = raw_line.strip_prefix("diff --git ") {
            if let Some(file) = current_file.take() {
                files.push(file);
            }
            let (old_path, new_path) = diff_header_paths(header);
            current_file = Some(FileBuilder::new(old_path, new_path, path_hint));
            active_path = current_file.as_ref().and_then(FileBuilder::syntax_path);
            old_highlighter = highlighter_for_path(assets, active_path.as_deref());
            new_highlighter = highlighter_for_path(assets, active_path.as_deref());
            old_line = None;
            new_line = None;
            continue;
        }
        if let Some(path) = raw_line
            .strip_prefix("diff --cc ")
            .or_else(|| raw_line.strip_prefix("diff --combined "))
        {
            if let Some(file) = current_file.take() {
                files.push(file);
            }
            let path = Some(PathBuf::from(decode_git_path(path)));
            current_file = Some(FileBuilder::new(path.clone(), path, path_hint));
            active_path = current_file.as_ref().and_then(FileBuilder::syntax_path);
            old_highlighter = highlighter_for_path(assets, active_path.as_deref());
            new_highlighter = highlighter_for_path(assets, active_path.as_deref());
            old_line = None;
            new_line = None;
            continue;
        }

        if current_file.is_none() {
            continue;
        }

        if raw_line.starts_with("index ") || raw_line.starts_with("similarity index ") {
            continue;
        }
        if raw_line.starts_with("new file mode ") {
            file_mut(&mut current_file, path_hint).status = Some("added");
            continue;
        }
        if raw_line.starts_with("deleted file mode ") {
            file_mut(&mut current_file, path_hint).status = Some("deleted");
            continue;
        }
        if let Some(path) = raw_line.strip_prefix("rename from ") {
            let file = file_mut(&mut current_file, path_hint);
            file.old_path = Some(PathBuf::from(decode_git_path(path)));
            file.status = Some("renamed");
            continue;
        }
        if let Some(path) = raw_line.strip_prefix("rename to ") {
            let file = file_mut(&mut current_file, path_hint);
            file.new_path = Some(PathBuf::from(decode_git_path(path)));
            file.status = Some("renamed");
            continue;
        }
        if let Some(path) = raw_line.strip_prefix("--- ") {
            file_mut(&mut current_file, path_hint).old_path = patch_path(path, "a/");
            continue;
        }
        if let Some(path) = raw_line.strip_prefix("+++ ") {
            let new_path = patch_path(path, "b/");
            file_mut(&mut current_file, path_hint)
                .new_path
                .clone_from(&new_path);
            active_path =
                new_path.or_else(|| current_file.as_ref().and_then(|file| file.old_path.clone()));
            old_highlighter = highlighter_for_path(assets, active_path.as_deref());
            new_highlighter = highlighter_for_path(assets, active_path.as_deref());
            continue;
        }
        if raw_line.starts_with("old mode ") || raw_line.starts_with("new mode ") {
            file_mut(&mut current_file, path_hint)
                .lines
                .push(meta_line(DiffLineKind::Meta, raw_line));
            continue;
        }
        if raw_line.starts_with("Binary files ") || raw_line == "GIT binary patch" {
            let file = file_mut(&mut current_file, path_hint);
            file.binary = true;
            file.lines.push(meta_line(DiffLineKind::Meta, raw_line));
            continue;
        }
        if current_file.as_ref().is_some_and(|file| file.binary) {
            file_mut(&mut current_file, path_hint)
                .lines
                .push(meta_line(DiffLineKind::Meta, raw_line));
            continue;
        }

        if raw_line.starts_with("@@") {
            let (old_start, new_start) = parse_hunk_starts(raw_line);
            old_line = old_start;
            new_line = new_start;
            file_mut(&mut current_file, path_hint)
                .lines
                .push(meta_line(DiffLineKind::HunkHeader, raw_line));
            continue;
        }

        if let Some(content) = raw_line.strip_prefix('+') {
            let number = new_line;
            new_line = new_line.map(|line| line + 1);
            let content = expand_tabs(content);
            let spans = highlight_optional(&mut new_highlighter, &content, assets);
            let file = file_mut(&mut current_file, path_hint);
            file.additions += 1;
            file.lines.push(DiffLine {
                kind: DiffLineKind::Added,
                old_line: None,
                new_line: number,
                spans,
            });
        } else if let Some(content) = raw_line.strip_prefix('-') {
            let number = old_line;
            old_line = old_line.map(|line| line + 1);
            let content = expand_tabs(content);
            let spans = highlight_optional(&mut old_highlighter, &content, assets);
            let file = file_mut(&mut current_file, path_hint);
            file.deletions += 1;
            file.lines.push(DiffLine {
                kind: DiffLineKind::Removed,
                old_line: number,
                new_line: None,
                spans,
            });
        } else if let Some(content) = raw_line.strip_prefix(' ') {
            let old_number = old_line;
            let new_number = new_line;
            old_line = old_line.map(|line| line + 1);
            new_line = new_line.map(|line| line + 1);
            let content = expand_tabs(content);
            let spans = highlight_optional(&mut new_highlighter, &content, assets);
            advance_highlighter(&mut old_highlighter, &content, assets);
            file_mut(&mut current_file, path_hint).lines.push(DiffLine {
                kind: DiffLineKind::Context,
                old_line: old_number,
                new_line: new_number,
                spans,
            });
        } else if !raw_line.is_empty() {
            file_mut(&mut current_file, path_hint)
                .lines
                .push(meta_line(DiffLineKind::Meta, raw_line));
        }
    }

    if let Some(file) = current_file.take() {
        files.push(file);
    }
    files.sort_by_cached_key(FileBuilder::sort_path);
    for file in files {
        flush_file(file, &mut lines);
    }
    if lines.is_empty() {
        return DiffDocument::empty(title, "No file changes to display");
    }
    if truncated {
        lines.push(meta_line(
            DiffLineKind::Meta,
            "… diff truncated to keep Quinjet responsive …",
        ));
    }

    DiffDocument {
        title,
        lines,
        truncated,
        commit_details: None,
        pull_request_details: None,
    }
}

#[expect(
    clippy::similar_names,
    reason = "the names follow the Git vocabulary they model"
)]
/// Cut a multi-file patch at its `diff --git` boundaries and key each section by
/// the paths in that header. One Git invocation can then answer for many files
/// while each file still parses and renders as its own document.
pub(crate) fn split_patch_by_file(patch: &[u8]) -> Vec<PatchSection<'_>> {
    let mut starts = Vec::new();
    let mut offset = 0;
    while offset < patch.len() {
        let end = patch
            .get(offset..)
            .unwrap_or_default()
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(patch.len(), |index| offset + index + 1);
        let line = patch.get(offset..end).unwrap_or_default();
        if line.starts_with(b"diff --git ")
            || line.starts_with(b"diff --cc ")
            || line.starts_with(b"diff --combined ")
        {
            starts.push(offset);
        }
        offset = end;
    }

    starts
        .iter()
        .enumerate()
        .map(|(index, start)| {
            let end = starts.get(index + 1).copied().unwrap_or(patch.len());
            let body = patch.get(*start..end).unwrap_or_default();
            let header = body.split(|byte| *byte == b'\n').next().unwrap_or_default();
            let header = String::from_utf8_lossy(header);
            let (old_path, new_path) = header.strip_prefix("diff --git ").map_or_else(
                || {
                    let path = header
                        .strip_prefix("diff --cc ")
                        .or_else(|| header.strip_prefix("diff --combined "))
                        .map(|path| PathBuf::from(decode_git_path(path.trim_end())));
                    (path.clone(), path)
                },
                diff_header_paths,
            );
            PatchSection {
                old_path,
                new_path,
                body,
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PatchSection<'a> {
    pub old_path: Option<PathBuf>,
    pub new_path: Option<PathBuf>,
    pub body: &'a [u8],
}

impl PatchSection<'_> {
    pub(crate) fn matches(&self, path: &Path) -> bool {
        self.new_path.as_deref() == Some(path) || self.old_path.as_deref() == Some(path)
    }
}

#[derive(Default)]
struct FileBuilder {
    old_path: Option<PathBuf>,
    new_path: Option<PathBuf>,
    status: Option<&'static str>,
    lines: Vec<DiffLine>,
    additions: usize,
    deletions: usize,
    binary: bool,
}

impl FileBuilder {
    fn new(old_path: Option<PathBuf>, new_path: Option<PathBuf>, path_hint: Option<&Path>) -> Self {
        let fallback = path_hint.map(Path::to_path_buf);
        Self {
            old_path: old_path.or_else(|| fallback.clone()),
            new_path: new_path.or(fallback),
            ..Self::default()
        }
    }

    fn syntax_path(&self) -> Option<PathBuf> {
        self.new_path.clone().or_else(|| self.old_path.clone())
    }

    fn sort_path(&self) -> String {
        self.new_path
            .as_ref()
            .or(self.old_path.as_ref())
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default()
    }

    fn display_path(&self) -> String {
        match (&self.old_path, &self.new_path) {
            (Some(old), Some(new)) if old != new => {
                format!("{} → {}", old.display(), new.display())
            }
            (_, Some(path)) | (Some(path), None) => path.display().to_string(),
            (None, None) => "Changed file".to_owned(),
        }
    }
}

fn file_mut<'a>(
    current: &'a mut Option<FileBuilder>,
    path_hint: Option<&Path>,
) -> &'a mut FileBuilder {
    current.get_or_insert_with(|| FileBuilder::new(None, None, path_hint))
}

fn flush_file(mut file: FileBuilder, output: &mut Vec<DiffLine>) {
    if file.lines.is_empty() {
        file.lines.push(meta_line(
            DiffLineKind::Meta,
            if file.status == Some("renamed") {
                "File renamed without content changes"
            } else {
                "No textual changes to display"
            },
        ));
    }

    let status = file
        .status
        .map(|status| format!("  · {status}"))
        .unwrap_or_default();
    output.push(DiffLine {
        kind: DiffLineKind::FileHeader,
        old_line: None,
        new_line: None,
        spans: vec![
            HighlightSpan::plain(format!("{}{}", file.display_path(), status)),
            HighlightSpan::plain(format!("+{}", file.additions)),
            HighlightSpan::plain(format!("-{}", file.deletions)),
        ],
    });
    output.append(&mut file.lines);
    output.push(meta_line(DiffLineKind::FileFooter, ""));
}

fn diff_header_paths(header: &str) -> (Option<PathBuf>, Option<PathBuf>) {
    let Some(separator) = header.rfind(" b/").or_else(|| header.rfind(" \"b/")) else {
        return (None, None);
    };
    let old = patch_path(header.get(..separator).unwrap_or_default(), "a/");
    let new = patch_path(header.get(separator + 1..).unwrap_or_default(), "b/");
    (old, new)
}

fn patch_path(value: &str, prefix: &str) -> Option<PathBuf> {
    let value = value.trim_end_matches('\t');
    if value == "/dev/null" {
        return None;
    }
    let decoded = decode_git_path(value);
    let path = decoded.strip_prefix(prefix).unwrap_or(&decoded);
    Some(PathBuf::from(path))
}

fn decode_git_path(value: &str) -> String {
    let Some(quoted) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        return value.to_owned();
    };
    let mut output = Vec::with_capacity(quoted.len());
    let mut bytes = quoted.bytes().peekable();
    while let Some(byte) = bytes.next() {
        if byte != b'\\' {
            output.push(byte);
            continue;
        }
        match bytes.next() {
            Some(b'n') => output.push(b'\n'),
            Some(b'r') => output.push(b'\r'),
            Some(b't') => output.push(b'\t'),
            Some(b'"') => output.push(b'"'),
            Some(first @ b'0'..=b'7') => {
                let mut value = first - b'0';
                for _ in 0..2 {
                    let Some(next @ b'0'..=b'7') = bytes.peek().copied() else {
                        break;
                    };
                    let _ = bytes.next();
                    value = value.saturating_mul(8).saturating_add(next - b'0');
                }
                output.push(value);
            }
            Some(other) => output.push(other),
            None => output.push(b'\\'),
        }
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn highlighter_for_path<'a>(
    assets: Option<&'a HighlightAssets>,
    path: Option<&Path>,
) -> Option<HighlightLines<'a>> {
    let assets = assets?;
    Some(HighlightLines::new(
        syntax_for_path(&assets.syntaxes, path),
        &assets.theme,
    ))
}

fn highlight_optional<'a>(
    highlighter: &mut Option<HighlightLines<'a>>,
    line: &str,
    assets: Option<&'a HighlightAssets>,
) -> Vec<HighlightSpan> {
    if line.len() > MAX_SYNTAX_HIGHLIGHT_LINE_BYTES {
        *highlighter = None;
        return vec![HighlightSpan::plain(line)];
    }
    match (highlighter.as_mut(), assets) {
        (Some(highlighter), Some(assets)) => highlight(highlighter, line, &assets.syntaxes),
        _ => vec![HighlightSpan::plain(line)],
    }
}

fn advance_highlighter<'a>(
    highlighter: &mut Option<HighlightLines<'a>>,
    line: &str,
    assets: Option<&'a HighlightAssets>,
) {
    if line.len() > MAX_SYNTAX_HIGHLIGHT_LINE_BYTES {
        *highlighter = None;
        return;
    }
    if let (Some(highlighter), Some(assets)) = (highlighter.as_mut(), assets) {
        drop(highlighter.highlight_line(line, &assets.syntaxes));
    }
}

fn syntax_for_path<'a>(syntaxes: &'a SyntaxSet, path: Option<&Path>) -> &'a SyntaxReference {
    path.and_then(|path| syntaxes.find_syntax_for_file(path).ok().flatten())
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text())
}

#[expect(
    clippy::option_if_let_else,
    reason = "the branch is one arm of a longer chain that map_or_else cannot express"
)]
fn highlight(
    highlighter: &mut HighlightLines<'_>,
    line: &str,
    syntaxes: &SyntaxSet,
) -> Vec<HighlightSpan> {
    match highlighter.highlight_line(line, syntaxes) {
        Ok(ranges) => ranges
            .into_iter()
            .map(|(style, text)| HighlightSpan {
                text: text.to_owned(),
                foreground: Some(syntax_color(style.foreground)),
                bold: style.font_style.contains(FontStyle::BOLD),
                italic: style.font_style.contains(FontStyle::ITALIC),
            })
            .collect(),
        Err(_) => vec![HighlightSpan::plain(line)],
    }
}

const fn syntax_color(color: syntect::highlighting::Color) -> SyntaxColor {
    match (color.r, color.g, color.b) {
        (101, 115, 126) => SyntaxColor::Comment,
        (191, 97, 106) => SyntaxColor::Red,
        (208, 135, 112) => SyntaxColor::Orange,
        (235, 203, 139) => SyntaxColor::Yellow,
        (163, 190, 140) => SyntaxColor::Green,
        (150, 181, 180) => SyntaxColor::Cyan,
        (143, 161, 179) => SyntaxColor::Blue,
        (180, 142, 173) => SyntaxColor::Purple,
        (171, 121, 103) => SyntaxColor::Brown,
        _ => SyntaxColor::Text,
    }
}

fn expand_tabs(line: &str) -> Cow<'_, str> {
    if !line.contains('\t') {
        return Cow::Borrowed(line);
    }

    let mut expanded = String::with_capacity(line.len());
    let mut column = 0;
    for character in line.chars() {
        if character == '\t' {
            let spaces = TAB_WIDTH - column % TAB_WIDTH;
            expanded.extend(std::iter::repeat_n(' ', spaces));
            column += spaces;
        } else {
            expanded.push(character);
            column += UnicodeWidthChar::width(character).unwrap_or_default();
        }
    }
    Cow::Owned(expanded)
}

fn meta_line(kind: DiffLineKind, text: &str) -> DiffLine {
    DiffLine {
        kind,
        old_line: None,
        new_line: None,
        spans: vec![HighlightSpan::plain(text)],
    }
}

fn parse_hunk_starts(line: &str) -> (Option<usize>, Option<usize>) {
    let mut fields = line.split_ascii_whitespace();
    let _marker = fields.next();
    let old = fields
        .next()
        .and_then(|field| parse_range_start(field, '-'));
    let new = fields
        .next()
        .and_then(|field| parse_range_start(field, '+'));
    (old, new)
}

fn parse_range_start(field: &str, prefix: char) -> Option<usize> {
    field.strip_prefix(prefix)?.split(',').next()?.parse().ok()
}

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests;
