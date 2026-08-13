use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
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
    #[cfg(test)]
    pub fn text(&self) -> String {
        self.spans.iter().map(|span| span.text.as_str()).collect()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffDocument {
    pub title: String,
    pub lines: Vec<DiffLine>,
    pub truncated: bool,
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
        }
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
    let mut lines = Vec::new();

    for raw_line in String::from_utf8_lossy(raw).lines() {
        if let Some(path) = raw_line.strip_prefix("+++ b/") {
            active_path = Some(PathBuf::from(path));
            syntax = syntax_for_path(&assets.syntaxes, active_path.as_deref());
            old_highlighter = HighlightLines::new(syntax, &assets.theme);
            new_highlighter = HighlightLines::new(syntax, &assets.theme);
            continue;
        }

        // File headers are Git transport metadata rather than source. The selected
        // filename already appears in the pane title, so keep the preview code-only.
        if raw_line.starts_with("diff --git ")
            || raw_line.starts_with("index ")
            || raw_line.starts_with("--- ")
            || raw_line.starts_with("new file mode ")
            || raw_line.starts_with("deleted file mode ")
            || raw_line.starts_with("similarity index ")
            || raw_line.starts_with("rename from ")
            || raw_line.starts_with("rename to ")
        {
            continue;
        }

        if raw_line.starts_with("@@") {
            let (old_start, new_start) = parse_hunk_starts(raw_line);
            old_line = old_start;
            new_line = new_start;
            lines.push(meta_line(DiffLineKind::HunkHeader, raw_line));
            continue;
        }

        if let Some(content) = raw_line.strip_prefix('+') {
            let number = new_line;
            new_line = new_line.map(|line| line + 1);
            lines.push(DiffLine {
                kind: DiffLineKind::Added,
                old_line: None,
                new_line: number,
                spans: highlight(&mut new_highlighter, content, &assets.syntaxes),
            });
        } else if let Some(content) = raw_line.strip_prefix('-') {
            let number = old_line;
            old_line = old_line.map(|line| line + 1);
            lines.push(DiffLine {
                kind: DiffLineKind::Removed,
                old_line: number,
                new_line: None,
                spans: highlight(&mut old_highlighter, content, &assets.syntaxes),
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
            lines.push(DiffLine {
                kind: DiffLineKind::Context,
                old_line: old_number,
                new_line: new_number,
                spans,
            });
        } else {
            lines.push(meta_line(DiffLineKind::Meta, raw_line));
        }
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
    }
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
}
