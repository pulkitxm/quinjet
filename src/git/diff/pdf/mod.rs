use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Result, ensure};

use super::{
    DiffBlobSource, DiffDocument, DiffLine, DiffLineKind, ImageSide, LoadedBlob, file_footer,
    header_identity, meta_line, parse_diff_with_highlighting,
};
use crate::git::github::run_bounded_command;

mod source;
#[cfg(test)]
mod tests;

use source::{SOURCE_LIMIT, decoded_source, escaped_source};

pub(super) fn attach_pdf_sources(
    document: &mut DiffDocument,
    source: &impl DiffBlobSource,
    expanded: bool,
    highlighting: bool,
) {
    let mut index = 0;
    while index < document.lines.len() {
        let Some(header) = document.lines.get(index) else {
            break;
        };
        if header.kind != DiffLineKind::FileHeader {
            index += 1;
            continue;
        }
        let Some(footer) = file_footer(&document.lines, index) else {
            break;
        };
        let (path, old_path, status) = header_identity(header);
        if !is_pdf_path(&path) && !old_path.as_deref().is_some_and(is_pdf_path) {
            index = footer + 1;
            continue;
        }
        let replacement = source_lines(
            source,
            &path,
            old_path.as_deref(),
            status,
            expanded,
            highlighting,
        );
        match replacement {
            Ok((lines, truncated)) => {
                update_counts(document, index, &lines);
                drop(document.lines.splice(index + 1..footer, lines));
                document.truncated |= truncated;
            }
            Err(error) => {
                document.lines.insert(
                    index + 1,
                    meta_line(
                        DiffLineKind::Meta,
                        &format!("PDF source unavailable: {error}"),
                    ),
                );
            }
        }
        index = file_footer(&document.lines, index).unwrap_or(index) + 1;
    }
}

fn is_pdf_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
}

fn source_lines(
    source: &impl DiffBlobSource,
    path: &Path,
    old_path: Option<&Path>,
    status: &str,
    expanded: bool,
    highlighting: bool,
) -> Result<(Vec<DiffLine>, bool)> {
    let previous = load_bytes(source.load(path, old_path, ImageSide::Previous))?;
    let current = load_bytes(source.load(path, old_path, ImageSide::New))?;
    ensure!(
        previous.is_some() || current.is_some(),
        "file content could not be read"
    );
    ensure!(
        previous.is_some() || matches!(status, "added" | "untracked"),
        "previous file content could not be read"
    );
    ensure!(
        current.is_some() || status == "deleted",
        "new file content could not be read"
    );
    let decoded = previous
        .as_deref()
        .map(decoded_source)
        .transpose()
        .and_then(|previous| {
            current
                .as_deref()
                .map(decoded_source)
                .transpose()
                .map(|current| (previous, current))
        });
    let (previous_source, current_source, label) = match decoded {
        Ok((old, new)) if old != new || previous == current => (
            old,
            new,
            "PDF source · normalized objects and decoded streams · escaped binary bytes",
        ),
        Ok(_) => (
            previous,
            current,
            "PDF source · stored bytes · only encoding or file layout changed · binary bytes use \\xNN",
        ),
        Err(_) => (
            previous,
            current,
            "PDF source · stored bytes · decoding unavailable · binary bytes use \\xNN",
        ),
    };
    let previous_text = escaped_source(previous_source.as_deref().unwrap_or_default())?;
    let current_text = escaped_source(current_source.as_deref().unwrap_or_default())?;
    let (diff_bytes, truncated) = source_patch(&previous_text, &current_text, expanded)?;
    let patch_text = String::from_utf8_lossy(&diff_bytes);
    let body_start = patch_text
        .find("\n@@")
        .map_or(patch_text.len(), |index| index + 1);
    let mut raw = b"diff --git a/source.pdf b/source.pdf\n".to_vec();
    raw.extend_from_slice(diff_bytes.get(body_start..).unwrap_or_default());
    let parsed =
        parse_diff_with_highlighting(&raw, "PDF source", Some(path), truncated, highlighting);
    let mut lines = vec![meta_line(DiffLineKind::Meta, label)];
    if body_start < diff_bytes.len() {
        lines.extend(parsed.lines.into_iter().filter(|line| {
            !matches!(
                line.kind,
                DiffLineKind::FileHeader | DiffLineKind::FileFooter
            )
        }));
    } else {
        lines.push(meta_line(DiffLineKind::Meta, "No PDF source differences"));
    }
    Ok((lines, truncated))
}

fn load_bytes(blob: LoadedBlob) -> Result<Option<Vec<u8>>> {
    match blob {
        LoadedBlob::Missing => Ok(None),
        LoadedBlob::Bytes(bytes) => Ok(Some(bytes)),
        LoadedBlob::TooLarge { size } => {
            anyhow::bail!("file has at least {size} bytes, above the 8 MiB limit")
        }
    }
}

fn source_patch(previous: &[u8], current: &[u8], expanded: bool) -> Result<(Vec<u8>, bool)> {
    let directory = tempfile::tempdir()?;
    let previous_path = directory.path().join("previous");
    let current_path = directory.path().join("current");
    fs::write(&previous_path, previous)?;
    fs::write(&current_path, current)?;
    let mut command = Command::new("git");
    let _ = command
        .args([
            "diff",
            "--no-index",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--text",
        ])
        .arg(if expanded {
            "--unified=1000000"
        } else {
            "--unified=3"
        })
        .arg("--")
        .arg(previous_path)
        .arg(current_path)
        .env("GIT_OPTIONAL_LOCKS", "0");
    let mut output = run_bounded_command(&mut command, SOURCE_LIMIT, 4096)?;
    ensure!(
        output.status.success() || output.status.code() == Some(1) || output.stdout_truncated,
        "could not compare PDF source"
    );
    if output.stdout_truncated {
        crate::git::support::truncate_to_complete_line(&mut output.stdout);
    }
    Ok((output.stdout, output.stdout_truncated))
}

fn update_counts(document: &mut DiffDocument, index: usize, lines: &[DiffLine]) {
    let Some(header) = document.lines.get_mut(index) else {
        return;
    };
    for (span_index, kind, prefix) in [
        (1, DiffLineKind::Added, '+'),
        (2, DiffLineKind::Removed, '-'),
    ] {
        if let Some(span) = header.spans.get_mut(span_index) {
            let count = lines.iter().filter(|line| line.kind == kind).count();
            span.text = format!("{prefix}{count}");
        }
    }
}
