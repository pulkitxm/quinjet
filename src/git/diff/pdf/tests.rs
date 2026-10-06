use lopdf::{Document, Stream, dictionary};

use super::*;
use crate::git::diff::MapBlobSource;

fn pdf(text: &str) -> Vec<u8> {
    let mut document = Document::with_version("1.5");
    let mut stream = Stream::new(
        dictionary! {},
        format!("BT\n({text}) Tj\nET\n").into_bytes(),
    );
    stream.compress().unwrap();
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
    attach_pdf_sources(&mut document, &source);
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
    let (lines, truncated) = source_lines(&source, Path::new("broken.PDF"), None).unwrap();
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
