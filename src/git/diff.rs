use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    FileHeader,
    FileFooter,
    HunkHeader,
    Context,
    Added,
    Removed,
    Meta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub text: String,
    pub foreground: Option<(u8, u8, u8)>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub spans: Vec<HighlightSpan>,
}

impl DiffLine {
    pub fn text(&self) -> String {
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommitDetails {
    pub id: String,
    pub subject: String,
    pub author: String,
    pub author_email: String,
    pub authored_at: String,
    pub committer: String,
    pub committer_email: String,
    pub committed_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PullRequestDetails {
    pub number: u64,
    pub title: String,
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
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffDocument {
    pub title: String,
    pub lines: Vec<DiffLine>,
    pub truncated: bool,
    pub commit_details: Option<CommitDetails>,
    pub pull_request_details: Option<PullRequestDetails>,
}

impl DiffDocument {
    pub fn empty(title: impl Into<String>, message: impl Into<String>) -> Self {
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

    pub fn file_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|line| line.kind == DiffLineKind::FileHeader)
            .count()
    }

    pub fn addition_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|line| line.kind == DiffLineKind::Added)
            .count()
    }

    pub fn deletion_count(&self) -> usize {
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

/// Parse a unified diff and highlight code on the old and new sides independently.
/// Keeping two parser states avoids additions corrupting the old-file syntax state and
/// removals corrupting the new-file state.
pub fn parse_diff(
    raw: &[u8],
    title: impl Into<String>,
    path_hint: Option<&Path>,
    truncated: bool,
) -> DiffDocument {
    let title = title.into();
    if raw.is_empty() {
        return DiffDocument::empty(title, "No textual diff to display");
    }

    let assets = highlight_assets();
    let mut active_path = path_hint.map(Path::to_path_buf);
    let mut syntax = syntax_for_path(&assets.syntaxes, active_path.as_deref());
    let mut old_highlighter = HighlightLines::new(syntax, &assets.theme);
    let mut new_highlighter = HighlightLines::new(syntax, &assets.theme);
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
            syntax = syntax_for_path(&assets.syntaxes, active_path.as_deref());
            old_highlighter = HighlightLines::new(syntax, &assets.theme);
            new_highlighter = HighlightLines::new(syntax, &assets.theme);
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
            syntax = syntax_for_path(&assets.syntaxes, active_path.as_deref());
            old_highlighter = HighlightLines::new(syntax, &assets.theme);
            new_highlighter = HighlightLines::new(syntax, &assets.theme);
            old_line = None;
            new_line = None;
            continue;
        }

        // Ignore commit headers and other preamble. A commit summary is rendered in
        // its own pane; only rows belonging to an actual file enter the diff model.
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
            file_mut(&mut current_file, path_hint).new_path = new_path.clone();
            active_path =
                new_path.or_else(|| current_file.as_ref().and_then(|file| file.old_path.clone()));
            syntax = syntax_for_path(&assets.syntaxes, active_path.as_deref());
            old_highlighter = HighlightLines::new(syntax, &assets.theme);
            new_highlighter = HighlightLines::new(syntax, &assets.theme);
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
            let spans = highlight(&mut new_highlighter, content, &assets.syntaxes);
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
            let spans = highlight(&mut old_highlighter, content, &assets.syntaxes);
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
            let spans = highlight(&mut new_highlighter, content, &assets.syntaxes);
            // Advance the old parser too. Its spans are normally identical, while its
            // state can differ after a replacement block.
            let _ = old_highlighter.highlight_line(content, &assets.syntaxes);
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
    // GitHub's path sort follows Git's canonical, case-sensitive ordering of the
    // complete repository-relative path. Sorting the full path (rather than only the
    // basename) keeps nested files in the same order as GitHub's changed-files view.
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
    let old = patch_path(&header[..separator], "a/");
    let new = patch_path(&header[separator + 1..], "b/");
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
            Some(b'\\') => output.push(b'\\'),
            Some(b'"') => output.push(b'"'),
            Some(first @ b'0'..=b'7') => {
                let mut value = first - b'0';
                for _ in 0..2 {
                    let Some(next @ b'0'..=b'7') = bytes.peek().copied() else {
                        break;
                    };
                    bytes.next();
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

fn syntax_for_path<'a>(syntaxes: &'a SyntaxSet, path: Option<&Path>) -> &'a SyntaxReference {
    path.and_then(|path| syntaxes.find_syntax_for_file(path).ok().flatten())
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text())
}

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
                foreground: Some((style.foreground.r, style.foreground.g, style.foreground.b)),
                bold: style.font_style.contains(FontStyle::BOLD),
                italic: style.font_style.contains(FontStyle::ITALIC),
            })
            .collect(),
        Err(_) => vec![HighlightSpan::plain(line)],
    }
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
mod tests {
    use super::*;

    #[test]
    fn parses_hunks_and_tracks_line_numbers() {
        let raw = b"diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -10,3 +10,4 @@ fn main() {\n let value = 1;\n-old();\n+new();\n+more();\n end();\n";

        let document = parse_diff(raw, "main.rs", Some(Path::new("src/main.rs")), false);
        let content: Vec<_> = document
            .lines
            .iter()
            .filter(|line| {
                matches!(
                    line.kind,
                    DiffLineKind::Context | DiffLineKind::Added | DiffLineKind::Removed
                )
            })
            .collect();

        assert_eq!(content.len(), 5);
        assert_eq!(
            (content[0].old_line, content[0].new_line),
            (Some(10), Some(10))
        );
        assert_eq!((content[1].old_line, content[1].new_line), (Some(11), None));
        assert_eq!((content[2].old_line, content[2].new_line), (None, Some(11)));
        assert_eq!((content[3].old_line, content[3].new_line), (None, Some(12)));
        assert_eq!(
            (content[4].old_line, content[4].new_line),
            (Some(12), Some(13))
        );
    }

    #[test]
    fn returns_explanatory_line_for_empty_diff() {
        let document = parse_diff(b"", "empty", None, false);
        assert_eq!(document.lines[0].text(), "No textual diff to display");
    }

    #[test]
    fn highlights_typescript_and_hides_git_transport_headers() {
        let raw = b"diff --git a/widget.tsx b/widget.tsx\nindex aaaaaaa..bbbbbbb 100644\n--- a/widget.tsx\n+++ b/widget.tsx\n@@ -1 +1 @@\n-const oldValue: number = 1;\n+const newValue: number = 2;\n";
        let document = parse_diff(raw, "widget.tsx", Some(Path::new("widget.tsx")), false);

        assert_eq!(document.lines.len(), 5);
        assert_eq!(document.lines[0].kind, DiffLineKind::FileHeader);
        assert!(document.lines[0].text().starts_with("widget.tsx"));
        assert_eq!(document.lines[1].kind, DiffLineKind::HunkHeader);
        assert!(document.lines[3].spans.len() > 1);
        assert!(
            document.lines[3]
                .spans
                .iter()
                .filter_map(|span| span.foreground)
                .collect::<std::collections::HashSet<_>>()
                .len()
                > 1
        );
        assert_eq!(document.lines[4].kind, DiffLineKind::FileFooter);
    }

    #[test]
    fn groups_commit_patch_into_named_file_sections_and_drops_preamble() {
        let raw = b"commit abcdef\nAuthor: Ada\n\ndiff --git a/one.rs b/one.rs\n--- a/one.rs\n+++ b/one.rs\n@@ -1 +1 @@\n-old\n+new\ndiff --git a/docs/two.md b/docs/two.md\nnew file mode 100644\n--- /dev/null\n+++ b/docs/two.md\n@@ -0,0 +1 @@\n+hello\n";

        let document = parse_diff(raw, "commit", None, false);
        let headers: Vec<_> = document
            .lines
            .iter()
            .filter(|line| line.kind == DiffLineKind::FileHeader)
            .map(DiffLine::text)
            .collect();

        assert_eq!(document.file_count(), 2);
        assert_eq!(document.addition_count(), 2);
        assert_eq!(document.deletion_count(), 1);
        assert_eq!(headers[0], "docs/two.md  · added +1 -0");
        assert_eq!(headers[1], "one.rs +1 -1");
        assert!(!document.lines.iter().any(|line| {
            let text = line.text();
            text.starts_with("commit ")
                || text.starts_with("Author:")
                || text.starts_with("diff --git")
        }));
    }

    #[test]
    fn sorts_files_by_case_sensitive_full_repository_path() {
        let mut files = [
            "src/ui/mod.rs",
            "README.md",
            "src/app.rs",
            ".github/workflows/ci.yml",
            "Cargo.toml",
            ".github/ISSUE_TEMPLATE/bug.yml",
            "CODE_OF_CONDUCT.md",
            ".github/labeler.yml",
        ]
        .map(|path| FileBuilder::new(None, Some(PathBuf::from(path)), None));

        files.sort_by_cached_key(FileBuilder::sort_path);
        let paths: Vec<_> = files.iter().map(FileBuilder::sort_path).collect();

        assert_eq!(
            paths,
            vec![
                ".github/ISSUE_TEMPLATE/bug.yml",
                ".github/labeler.yml",
                ".github/workflows/ci.yml",
                "CODE_OF_CONDUCT.md",
                "Cargo.toml",
                "README.md",
                "src/app.rs",
                "src/ui/mod.rs",
            ]
        );
    }
}
