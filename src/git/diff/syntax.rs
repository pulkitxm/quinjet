#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) struct HighlightAssets {
    syntaxes: SyntaxSet,
    theme: Theme,
}

static HIGHLIGHT_ASSETS: OnceLock<HighlightAssets> = OnceLock::new();

pub(super) fn highlight_assets() -> &'static HighlightAssets {
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

pub(super) fn highlighter_for_path<'a>(
    assets: Option<&'a HighlightAssets>,
    path: Option<&Path>,
) -> Option<HighlightLines<'a>> {
    let assets = assets?;
    Some(HighlightLines::new(
        syntax_for_path(&assets.syntaxes, path),
        &assets.theme,
    ))
}

pub(super) fn highlight_optional<'a>(
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

pub(super) fn advance_highlighter<'a>(
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

pub(super) const fn syntax_color(color: syntect::highlighting::Color) -> SyntaxColor {
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

pub(super) fn expand_tabs(line: &str) -> Cow<'_, str> {
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

pub(super) fn meta_line(kind: DiffLineKind, text: &str) -> DiffLine {
    DiffLine {
        kind,
        old_line: None,
        new_line: None,
        spans: vec![HighlightSpan::plain(text)],
    }
}

pub(super) fn parse_hunk_starts(line: &str) -> (Option<usize>, Option<usize>) {
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
