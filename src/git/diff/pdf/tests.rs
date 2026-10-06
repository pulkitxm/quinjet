use lopdf::{Document, Stream, dictionary};

use super::*;
use crate::git::diff::MapBlobSource;

fn pdf(text: &str) -> Vec<u8> {
    let mut document = Document::with_version("1.5");
    let mut stream = Stream::new(
        dictionary! {},
        format!("BT\n({text}) Tj\nET\n").repeat(10).into_bytes(),
    );
    stream.compress().unwrap();
    assert!(stream.dict.has(b"Filter"));
    let _ = document.add_object(stream);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

#[test]
fn compressed_pdf_changes_show_content_commands() {
    let mut source = MapBlobSource::default();
    drop(
        source
            .previous
            .insert("resume.pdf".into(), pdf("Previous title")),
    );
    drop(
        source
            .current
            .insert("resume.pdf".into(), pdf("Current title")),
    );
    let mut document = parse_diff(
        b"diff --git a/resume.pdf b/resume.pdf\nBinary files a/resume.pdf and b/resume.pdf differ\n",
        "PDF change", None, false,
    );
    attach_pdf_sources(&mut document, &source, false);
    assert!(
        document
            .lines
            .iter()
            .any(|line| line.kind == DiffLineKind::Removed
                && line.text().contains("(Previous title) Tj"))
    );
    assert!(
        document
            .lines
            .iter()
            .any(|line| line.kind == DiffLineKind::Added
                && line.text().contains("(Current title) Tj"))
    );
    assert!(
        !document
            .lines
            .iter()
            .any(|line| line.text().starts_with("Binary files"))
    );
    assert!(!document.lines.iter().any(|line| line.image.is_some()));
}

#[test]
fn invalid_pdf_falls_back_to_exact_escaped_bytes() {
    let mut source = MapBlobSource::default();
    drop(source.current.insert(
        "broken.PDF".into(),
        b"%PDF-invalid\n\0\x1b\\x00\xff\n".to_vec(),
    ));
    let (lines, truncated) =
        source_lines(&source, Path::new("broken.PDF"), None, "added", false).unwrap();
    assert!(!truncated);
    assert!(
        lines
            .iter()
            .any(|line| line.text().contains("stored bytes"))
    );
    assert!(lines.iter().any(|line| line.kind == DiffLineKind::Added && line.text() == "\\x00\\x1b\\\\x00\\xff"));
}

#[test]
fn source_escape_is_bounded_and_breaks_long_binary_rows() {
    let escaped = escaped_source(&[0xff; 80]).unwrap();
    assert!(
        escaped
            .split(|byte| *byte == b'\n')
            .all(|line| line.len() <= 161)
    );
    let error = escaped_source(&vec![0xff; SOURCE_LIMIT]).unwrap_err();
    assert!(error.to_string().contains("8 MiB limit"));
}

#[test]
fn additions_deletions_and_renames_keep_the_correct_sides() {
    let mut source = MapBlobSource::default();
    drop(source.current.insert("added.pdf".into(), pdf("Added text")));
    drop(
        source
            .previous
            .insert("deleted.pdf".into(), pdf("Deleted text")),
    );
    drop(source.previous.insert("old.pdf".into(), pdf("Old name")));
    drop(source.current.insert("new.pdf".into(), pdf("New name")));
    for (path, old_path, added, removed) in [
        ("added.pdf", None, "Added text", ""),
        ("deleted.pdf", None, "", "Deleted text"),
        (
            "new.pdf",
            Some(Path::new("old.pdf")),
            "New name",
            "Old name",
        ),
    ] {
        let (lines, _) = source_lines(
            &source,
            Path::new(path),
            old_path,
            if added.is_empty() {
                "deleted"
            } else if removed.is_empty() {
                "added"
            } else {
                "renamed"
            },
            false,
        )
        .unwrap();
        assert_eq!(
            lines.iter().any(|line| line.kind == DiffLineKind::Added),
            !added.is_empty()
        );
        assert_eq!(
            lines.iter().any(|line| line.kind == DiffLineKind::Removed),
            !removed.is_empty()
        );
        for (kind, value) in [
            (DiffLineKind::Added, added),
            (DiffLineKind::Removed, removed),
        ] {
            if !value.is_empty() {
                assert!(
                    lines
                        .iter()
                        .any(|line| line.kind == kind && line.text().contains(value))
                );
            }
        }
    }
}

#[test]
fn expanded_source_keeps_context_outside_changed_hunks() {
    let previous = b"first\nsecond\nthird\nfourth\nfifth\nsixth\nseventh\nold\nlast\n";
    let current = b"first\nsecond\nthird\nfourth\nfifth\nsixth\nseventh\nnew\nlast\n";
    let (compact, _) = source_patch(previous, current, false).unwrap();
    let (expanded, _) = source_patch(previous, current, true).unwrap();
    assert!(!String::from_utf8_lossy(&compact).contains(" first"));
    assert!(String::from_utf8_lossy(&expanded).contains(" first"));
}

#[test]
fn encoding_only_changes_show_stored_bytes() {
    let bytes = pdf("Unchanged content");
    let mut modified = bytes.clone();
    modified.extend_from_slice(b"\n");
    let mut source = MapBlobSource::default();
    drop(source.previous.insert("document.pdf".into(), bytes));
    drop(source.current.insert("document.pdf".into(), modified));
    let (lines, _) =
        source_lines(&source, Path::new("document.pdf"), None, "modified", false).unwrap();
    assert!(
        lines
            .iter()
            .any(|line| line.text().contains("only encoding or file layout changed"))
    );
    assert!(lines.iter().any(|line| line.kind == DiffLineKind::Added));
}

#[test]
fn oversized_pdf_keeps_binary_notice_and_explains_the_limit() {
    let mut source = MapBlobSource::default();
    drop(
        source
            .current
            .insert("large.pdf".into(), vec![0; SOURCE_LIMIT + 1]),
    );
    let mut document = parse_diff(
        b"diff --git a/large.pdf b/large.pdf\nnew file mode 100644\nBinary files /dev/null and b/large.pdf differ\n",
        "Large PDF", None, false,
    );
    attach_pdf_sources(&mut document, &source, false);
    assert!(
        document
            .lines
            .iter()
            .any(|line| line.text().contains("above the 8 MiB limit"))
    );
    assert!(
        document
            .lines
            .iter()
            .any(|line| line.text().starts_with("Binary files"))
    );
}
