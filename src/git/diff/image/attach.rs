use std::path::{Path, PathBuf};

use super::decode::{decode_image, skipped_too_large};
use super::detect::looks_like_image;
use super::source::{ImageBlobSource, LoadedBlob};
use super::{ImagePreview, ImageSide};
use crate::git::diff::{DiffDocument, DiffLine, DiffLineKind, HighlightSpan};

pub(crate) fn attach_image_previews(document: &mut DiffDocument, source: &impl ImageBlobSource) {
    let mut index = 0;
    while index < document.lines.len() {
        let Some(line) = document.lines.get(index) else {
            break;
        };
        if line.kind != DiffLineKind::FileHeader {
            index += 1;
            continue;
        }
        let header = index;
        let Some(footer) = file_footer(&document.lines, header) else {
            break;
        };
        let (path, old_path, status) = header_identity(line);
        let body_start = header.saturating_add(1);
        let body = document.lines.get(body_start..footer).unwrap_or_default();
        let binary = body.iter().any(is_binary_meta);
        if !should_preview(&path, old_path.as_deref(), binary) {
            index = footer.saturating_add(1);
            continue;
        }
        let replacement = preview_lines(source, &path, old_path.as_deref(), status, body);
        if replacement.is_empty() {
            index = footer.saturating_add(1);
            continue;
        }
        drop(document.lines.splice(body_start..footer, replacement));
        index = file_footer(&document.lines, header)
            .unwrap_or(header)
            .saturating_add(1);
    }
}

fn should_preview(path: &Path, old_path: Option<&Path>, binary: bool) -> bool {
    looks_like_image(path, None)
        || old_path.is_some_and(|path| looks_like_image(path, None))
        || binary
}

fn preview_lines(
    source: &impl ImageBlobSource,
    path: &Path,
    old_path: Option<&Path>,
    status: &str,
    body: &[DiffLine],
) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    for side in sides_for(status, body) {
        match source.load(path, old_path, side) {
            LoadedBlob::Missing => {}
            LoadedBlob::TooLarge { size } => {
                if looks_like_image(path, None)
                    || old_path.is_some_and(|old| looks_like_image(old, None))
                {
                    lines.extend(to_lines(skipped_too_large(path, side, size)));
                }
            }
            LoadedBlob::Bytes(bytes) => {
                if !looks_like_image(path, Some(&bytes))
                    && !old_path.is_some_and(|path| looks_like_image(path, Some(&bytes)))
                {
                    continue;
                }
                lines.extend(to_lines(decode_image(&bytes, path, side)));
            }
        }
    }
    lines
}

fn to_lines(previews: Vec<ImagePreview>) -> Vec<DiffLine> {
    previews
        .into_iter()
        .map(|preview| {
            let caption = preview.caption();
            DiffLine {
                kind: DiffLineKind::Image,
                old_line: None,
                new_line: None,
                spans: vec![HighlightSpan::plain(caption)],
                image: Some(preview),
            }
        })
        .collect()
}

fn sides_for(status: &str, body: &[DiffLine]) -> Vec<ImageSide> {
    if let Some(sides) = sides_from_binary(body) {
        return sides;
    }
    if status == "renamed" && body.iter().any(is_rename_only) {
        return vec![ImageSide::New];
    }
    match status {
        "added" | "untracked" | "copied" => vec![ImageSide::New],
        "deleted" => vec![ImageSide::Previous],
        _ => vec![ImageSide::Previous, ImageSide::New],
    }
}

fn sides_from_binary(body: &[DiffLine]) -> Option<Vec<ImageSide>> {
    let text = body.iter().find(|line| is_binary_meta(line))?.text();
    let lower = text.to_ascii_lowercase();
    if lower.contains("/dev/null and") {
        return Some(vec![ImageSide::New]);
    }
    if lower.contains("and /dev/null") {
        return Some(vec![ImageSide::Previous]);
    }
    if lower.contains("binary files") {
        return Some(vec![ImageSide::Previous, ImageSide::New]);
    }
    None
}

fn is_binary_meta(line: &DiffLine) -> bool {
    let text = line.text();
    text.starts_with("Binary files ") || text == "GIT binary patch"
}

fn is_rename_only(line: &DiffLine) -> bool {
    line.text() == "File renamed without content changes"
}

fn file_footer(lines: &[DiffLine], header: usize) -> Option<usize> {
    lines
        .iter()
        .enumerate()
        .skip(header.saturating_add(1))
        .find_map(|(index, line)| (line.kind == DiffLineKind::FileFooter).then_some(index))
}

fn header_identity(line: &DiffLine) -> (PathBuf, Option<PathBuf>, &str) {
    let label = line
        .spans
        .first()
        .map(|span| span.text.as_str())
        .unwrap_or_default();
    let (path_part, status) = label
        .split_once("  · ")
        .map_or((label, ""), |(path, rest)| {
            (path, rest.split("  · ").next().unwrap_or(rest))
        });
    if let Some((old, new)) = path_part.split_once(" → ") {
        return (PathBuf::from(new), Some(PathBuf::from(old)), status);
    }
    if let Some(old) = status.strip_prefix("renamed from ") {
        return (
            PathBuf::from(path_part),
            Some(PathBuf::from(old)),
            "renamed",
        );
    }
    (PathBuf::from(path_part), None, status)
}
