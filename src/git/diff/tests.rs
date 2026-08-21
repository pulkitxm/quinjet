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
fn lazy_index_keeps_all_headers_while_merging_one_loaded_file() {
    let index = DiffIndex {
        title: "Branch comparison".to_owned(),
        files: vec![
            DiffFileIndexEntry {
                path: PathBuf::from("src/first.rs"),
                old_path: None,
                status: "modified".to_owned(),
                counts: None,
            },
            DiffFileIndexEntry {
                path: PathBuf::from("src/second.rs"),
                old_path: None,
                status: "added".to_owned(),
                counts: None,
            },
        ],
        truncated: false,
        commit_details: None,
    };
    let mut loaded = HashMap::new();
    let skeleton = index.document(&loaded);
    assert_eq!(skeleton.file_count(), 2);
    assert!(skeleton.lines[0].text().contains("+··"));
    assert_eq!(
        skeleton
            .lines
            .iter()
            .filter(|line| line.text() == "Loading diff…")
            .count(),
        2
    );

    loaded.insert(
            PathBuf::from("src/first.rs"),
            parse_diff(
                b"diff --git a/src/first.rs b/src/first.rs\n--- a/src/first.rs\n+++ b/src/first.rs\n@@ -1 +1 @@\n-old();\n+new();\n",
                "first",
                Some(Path::new("src/first.rs")),
                false,
            ),
        );
    let document = index.document(&loaded);

    assert_eq!(document.file_count(), 2);
    assert_eq!(document.addition_count(), 1);
    assert_eq!(document.deletion_count(), 1);
    assert!(document.lines.iter().any(|line| line.text() == "new();"));
    assert!(
        document
            .lines
            .iter()
            .any(|line| line.text().contains("src/second.rs") && line.text().contains("+··"))
    );

    let collapsed = index.document_with_visibility(&loaded, |_| false);
    assert_eq!(collapsed.file_count(), 2);
    assert_eq!(collapsed.addition_count(), 0);
    assert!(!collapsed.lines.iter().any(|line| line.text() == "new();"));
    assert!(collapsed.lines[0].text().contains("+1"));
    assert!(
        collapsed
            .lines
            .iter()
            .any(|line| line.text() == "Diff loaded · expand this file to display it")
    );
    assert!(
        collapsed
            .lines
            .iter()
            .any(|line| line.text() == "Expand this file to load its diff")
    );
}

#[test]
fn reads_numstat_totals_for_plain_renamed_and_binary_paths() {
    let output = b"1\t1\tsrc/keep.rs\x001\t0\t\x00old/name.rs\x00new/name.rs\x00-\t-\tassets/logo.png\x004\t2\tpath\twith\ttabs.rs\x00";

    let counts = parse_numstat(output);

    assert_eq!(counts.len(), 4);
    assert_eq!(
        counts[Path::new("src/keep.rs")],
        DiffLineCounts {
            additions: 1,
            deletions: 1,
            binary: false,
        }
    );
    assert_eq!(
        counts[Path::new("new/name.rs")],
        DiffLineCounts {
            additions: 1,
            deletions: 0,
            binary: false,
        }
    );
    assert!(!counts.contains_key(Path::new("old/name.rs")));
    assert!(counts[Path::new("assets/logo.png")].binary);
    assert_eq!(
        counts[Path::new("path\twith\ttabs.rs")],
        DiffLineCounts {
            additions: 4,
            deletions: 2,
            binary: false,
        }
    );
}

#[test]
fn splits_a_batched_patch_into_one_section_per_file() {
    let patch = b"diff --git a/src/one.rs b/src/one.rs\n--- a/src/one.rs\n+++ b/src/one.rs\n@@ -1 +1 @@\n-old\n+new\ndiff --git a/old name.rs b/new name.rs\nsimilarity index 90%\nrename from old name.rs\nrename to new name.rs\ndiff --git a/gone.rs b/gone.rs\ndeleted file mode 100644\n--- a/gone.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n-bye\n";

    let sections = split_patch_by_file(patch);

    assert_eq!(sections.len(), 3);
    assert!(sections[0].matches(Path::new("src/one.rs")));
    assert!(
        sections[1].matches(Path::new("new name.rs")),
        "a rename is keyed by the post-image path even when it contains spaces"
    );
    assert!(sections[1].matches(Path::new("old name.rs")));
    assert!(sections[2].matches(Path::new("gone.rs")));
    assert!(!sections[0].matches(Path::new("gone.rs")));
    assert_eq!(
        String::from_utf8_lossy(sections[2].body),
        "diff --git a/gone.rs b/gone.rs\ndeleted file mode 100644\n--- a/gone.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n-bye\n"
    );

    let document = parse_diff(sections[0].body, "one", None, false);
    assert_eq!(document.file_count(), 1);
    assert_eq!(document.addition_count(), 1);
}

#[test]
fn indexed_counts_render_before_any_patch_is_loaded() {
    let index = DiffIndex {
        title: "Pull request".to_owned(),
        files: vec![
            DiffFileIndexEntry {
                path: PathBuf::from(".github/workflows/pages.yml"),
                old_path: None,
                status: "added".to_owned(),
                counts: Some(DiffLineCounts {
                    additions: 40,
                    deletions: 0,
                    binary: false,
                }),
            },
            DiffFileIndexEntry {
                path: PathBuf::from("README.md"),
                old_path: None,
                status: "modified".to_owned(),
                counts: Some(DiffLineCounts {
                    additions: 12,
                    deletions: 3,
                    binary: false,
                }),
            },
        ],
        truncated: false,
        commit_details: None,
    };

    let skeleton = index.document_with_visibility(&HashMap::new(), |_| false);
    let headers: Vec<_> = skeleton
        .lines
        .iter()
        .filter(|line| line.kind == DiffLineKind::FileHeader)
        .map(DiffLine::text)
        .collect();

    assert_eq!(
        headers,
        vec![
            ".github/workflows/pages.yml  · added +40 -0",
            "README.md  · modified +12 -3",
        ]
    );
    assert!(
        !skeleton
            .lines
            .iter()
            .any(|line| line.text().contains("+··"))
    );
}

#[test]
fn indexed_totals_do_not_depend_on_loaded_or_visible_patches() {
    let index = DiffIndex {
        title: "Commit".to_owned(),
        files: vec![
            DiffFileIndexEntry {
                path: PathBuf::from("one.rs"),
                old_path: None,
                status: "modified".to_owned(),
                counts: Some(DiffLineCounts {
                    additions: 12,
                    deletions: 2,
                    binary: false,
                }),
            },
            DiffFileIndexEntry {
                path: PathBuf::from("two.rs"),
                old_path: None,
                status: "modified".to_owned(),
                counts: Some(DiffLineCounts {
                    additions: 3,
                    deletions: 7,
                    binary: false,
                }),
            },
        ],
        truncated: false,
        commit_details: None,
    };

    assert_eq!(
        index.line_counts(),
        DiffLineCounts {
            additions: 15,
            deletions: 9,
            binary: false,
        }
    );
    assert_eq!(
        index
            .document_with_visibility(&HashMap::new(), |_| false)
            .addition_count(),
        0
    );
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
fn base16_syntax_colors_have_stable_semantic_roles() {
    let colors = [
        ((192, 197, 206), SyntaxColor::Text),
        ((101, 115, 126), SyntaxColor::Comment),
        ((191, 97, 106), SyntaxColor::Red),
        ((208, 135, 112), SyntaxColor::Orange),
        ((235, 203, 139), SyntaxColor::Yellow),
        ((163, 190, 140), SyntaxColor::Green),
        ((150, 181, 180), SyntaxColor::Cyan),
        ((143, 161, 179), SyntaxColor::Blue),
        ((180, 142, 173), SyntaxColor::Purple),
        ((171, 121, 103), SyntaxColor::Brown),
    ];
    for ((red, green, blue), expected) in colors {
        assert_eq!(
            syntax_color(syntect::highlighting::Color {
                r: red,
                g: green,
                b: blue,
                a: 255,
            }),
            expected
        );
    }
}

#[test]
fn skips_syntax_grammar_work_for_large_patches() {
    let mut raw = String::from(
        "diff --git a/generated.rs b/generated.rs\n--- a/generated.rs\n+++ b/generated.rs\n@@ -0,0 +1 @@\n",
    );
    while raw.len() <= MAX_SYNTAX_HIGHLIGHT_PATCH_BYTES {
        raw.push_str("+pub const GENERATED: usize = 1;\n");
    }

    let document = parse_diff(
        raw.as_bytes(),
        "generated.rs",
        Some(Path::new("generated.rs")),
        false,
    );
    let added = document
        .lines
        .iter()
        .find(|line| line.kind == DiffLineKind::Added)
        .unwrap();

    assert!(added.spans.iter().all(|span| span.foreground.is_none()));
}

#[test]
fn skips_syntax_grammar_work_for_very_long_lines() {
    let content = "x".repeat(MAX_SYNTAX_HIGHLIGHT_LINE_BYTES + 1);
    let raw = format!(
        "diff --git a/generated.rs b/generated.rs\n--- a/generated.rs\n+++ b/generated.rs\n@@ -0,0 +1 @@\n+{content}\n"
    );

    let document = parse_diff(
        raw.as_bytes(),
        "generated.rs",
        Some(Path::new("generated.rs")),
        false,
    );
    let added = document
        .lines
        .iter()
        .find(|line| line.kind == DiffLineKind::Added)
        .unwrap();

    assert!(added.spans.iter().all(|span| span.foreground.is_none()));
}

#[test]
fn preserves_space_indentation_and_expands_tabs_to_tab_stops() {
    let raw = b"diff --git a/widget.tsx b/widget.tsx\n--- a/widget.tsx\n+++ b/widget.tsx\n@@ -1,2 +1,2 @@\n-  oldValue,\n+\tnewValue,\n \t  nestedValue,\n";
    let document = parse_diff(raw, "widget.tsx", Some(Path::new("widget.tsx")), false);
    let content = document
        .lines
        .iter()
        .filter(|line| {
            matches!(
                line.kind,
                DiffLineKind::Context | DiffLineKind::Added | DiffLineKind::Removed
            )
        })
        .map(DiffLine::text)
        .collect::<Vec<_>>();

    assert_eq!(
        content,
        vec!["  oldValue,", "    newValue,", "      nestedValue,"]
    );
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
        text.starts_with("commit ") || text.starts_with("Author:") || text.starts_with("diff --git")
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
