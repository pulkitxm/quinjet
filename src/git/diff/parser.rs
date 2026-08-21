#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

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
pub(super) struct FileBuilder {
    old_path: Option<PathBuf>,
    new_path: Option<PathBuf>,
    status: Option<&'static str>,
    lines: Vec<DiffLine>,
    additions: usize,
    deletions: usize,
    binary: bool,
}

impl FileBuilder {
    pub(super) fn new(
        old_path: Option<PathBuf>,
        new_path: Option<PathBuf>,
        path_hint: Option<&Path>,
    ) -> Self {
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

    pub(super) fn sort_path(&self) -> String {
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
