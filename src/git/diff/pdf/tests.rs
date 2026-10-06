use lopdf::{Document, Stream, dictionary};

use super::*;
use crate::git::diff::{MapBlobSource, attach_file_previews, parse_diff};
use crate::git::github::{GitHubRepository, PullRequest};
use crate::git::tests::TestRepository;
use crate::git::{LocalDiffRequest, Repository};

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
    attach_pdf_sources(&mut document, &source, false, true);
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
        source_lines(&source, Path::new("broken.PDF"), None, "added", false, true).unwrap();
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
            true,
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
    let (lines, _) = source_lines(
        &source,
        Path::new("document.pdf"),
        None,
        "modified",
        false,
        true,
    )
    .unwrap();
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
    attach_pdf_sources(&mut document, &source, false, true);
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

fn assert_plain_pdf_rows(plain: &DiffDocument, highlighted: &DiffDocument) {
    assert!(
        highlighted
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .any(|span| span.foreground.is_some())
    );
    assert!(
        plain
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .all(|span| span.foreground.is_none() && !span.bold && !span.italic)
    );
    let rows = |document: &DiffDocument| {
        document
            .lines
            .iter()
            .map(|line| (line.kind, line.old_line, line.new_line, line.text()))
            .collect::<Vec<_>>()
    };
    assert_eq!(rows(plain), rows(highlighted));
    assert_eq!(plain.title, highlighted.title);
    assert_eq!(plain.truncated, highlighted.truncated);
    assert_eq!(plain.commit_details, highlighted.commit_details);
    assert_eq!(plain.pull_request_details, highlighted.pull_request_details);
    assert!(plain.lines.iter().all(|line| line.image.is_none()));
}

#[test]
fn pdf_source_highlighting_can_be_disabled_without_changing_rows() {
    let previous = pdf("Previous title");
    let mut encoding_only = previous.clone();
    encoding_only.push(b'\n');
    for (old, new) in [
        (previous.clone(), pdf("Current title")),
        (previous, encoding_only),
        (
            b"%PDF-invalid\nprevious\0\xff\n".to_vec(),
            b"%PDF-invalid\ncurrent\0\xfe\n".to_vec(),
        ),
    ] {
        let mut source = MapBlobSource::default();
        drop(source.previous.insert("resume.pdf".into(), old));
        drop(source.current.insert("resume.pdf".into(), new));
        for expanded in [false, true] {
            let patch = b"diff --git a/resume.pdf b/resume.pdf\nBinary files a/resume.pdf and b/resume.pdf differ\n";
            let mut highlighted = parse_diff(patch, "PDF change", None, false);
            attach_file_previews(&mut highlighted, &source, expanded, true);
            let mut plain = parse_diff_with_highlighting(patch, "PDF change", None, false, false);
            attach_file_previews(&mut plain, &source, expanded, false);
            assert_plain_pdf_rows(&plain, &highlighted);
        }
    }
}

fn commit_pdf_fixture(repository: &Repository, text: &str) {
    fs::write(repository.root().join("resume.pdf"), pdf(text)).unwrap();
    drop(repository.checked(["add", "--", "resume.pdf"]).unwrap());
    drop(
        repository
            .checked([
                "-c",
                "user.name=Quinjet Test",
                "-c",
                "user.email=quinjet@example.com",
                "commit",
                "--message=PDF fixture",
            ])
            .unwrap(),
    );
}

fn assert_local_pdf_presentation(repository: &Repository, request: &LocalDiffRequest) {
    let path = Path::new("resume.pdf");
    let highlighted = repository
        .prepare_local_diff(request)
        .unwrap()
        .diff_file(path)
        .unwrap();
    let mut plain_repository = repository.clone_for_worker();
    plain_repository.set_diff_highlighting(false);
    let plain = plain_repository
        .prepare_local_diff(request)
        .unwrap()
        .diff_file(path)
        .unwrap();
    assert!(plain.lines.iter().any(|line| line.text().contains(") Tj")));
    assert_plain_pdf_rows(&plain, &highlighted);
}

#[test]
fn local_pdf_previews_follow_repository_highlighting() {
    let fixture = TestRepository::with_branch("main");
    let repository = fixture.repository();
    commit_pdf_fixture(&repository, "Previous title");
    drop(repository.checked(["branch", "pdf-base"]).unwrap());
    commit_pdf_fixture(&repository, "Current title");
    let commit = repository.history("HEAD", 0, 1).unwrap().remove(0);
    let branch = repository
        .history_branches()
        .unwrap()
        .into_iter()
        .find(|branch| branch.name == "pdf-base")
        .unwrap();
    fs::write(repository.root().join("resume.pdf"), pdf("Worktree title")).unwrap();
    let changes = repository.status().unwrap().changes;
    for expanded in [false, true] {
        for request in [
            LocalDiffRequest::Changes {
                changes: changes.clone(),
                version: 0,
                expanded,
            },
            LocalDiffRequest::Commit {
                commit: Box::new(commit.clone()),
                expanded,
            },
            LocalDiffRequest::Branch {
                branch: Box::new(branch.clone()),
                current: "main".to_owned(),
                current_oid: Some(commit.id.clone()),
                expanded,
            },
        ] {
            assert_local_pdf_presentation(&repository, &request);
        }
    }
    drop(
        repository
            .checked([
                "-c",
                "user.name=Quinjet Test",
                "-c",
                "user.email=quinjet@example.com",
                "stash",
                "push",
                "--message=PDF fixture",
            ])
            .unwrap(),
    );
    let stash = repository.stashes().unwrap().remove(0);
    for expanded in [false, true] {
        assert_local_pdf_presentation(
            &repository,
            &LocalDiffRequest::Stash {
                stash: Box::new(stash.clone()),
                expanded,
            },
        );
    }
}

#[test]
fn prepared_pull_request_pdf_previews_follow_repository_highlighting() {
    let fixture = TestRepository::with_branch("main");
    let mut repository = fixture.repository();
    commit_pdf_fixture(&repository, "Previous title");
    let base = repository.history("HEAD", 0, 1).unwrap().remove(0);
    commit_pdf_fixture(&repository, "Current title");
    let head = repository.history("HEAD", 0, 1).unwrap().remove(0);
    let pull_request = PullRequest {
        number: 7,
        base_oid: base.id,
        head_oid: head.id,
        base_repository: GitHubRepository {
            name_with_owner: "acme/widget".to_owned(),
            url: "https://invalid.example.test/acme/widget".to_owned(),
            remotes: Vec::new(),
        },
        changed_files: 1,
        ..PullRequest::default()
    };
    let path = Path::new("resume.pdf");
    let highlighted_workspace = repository
        .prepare_pull_request_diff(&pull_request, |_| {})
        .unwrap();
    let highlighted = highlighted_workspace.diff_file(path).unwrap();
    repository.set_diff_highlighting(false);
    let plain_workspace = repository
        .prepare_pull_request_diff(&pull_request, |_| {})
        .unwrap();
    let plain = plain_workspace.diff_file(path).unwrap();
    assert_plain_pdf_rows(&plain, &highlighted);
    assert_eq!(
        highlighted_workspace
            .diff_files(&[path.to_path_buf()])
            .unwrap(),
        vec![(path.to_path_buf(), highlighted)]
    );
    assert_eq!(
        plain_workspace.diff_files(&[path.to_path_buf()]).unwrap(),
        vec![(path.to_path_buf(), plain)]
    );
}
