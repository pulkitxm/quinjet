use super::*;

fn change(path: &str, original_path: Option<&str>, area: ChangeArea) -> Change {
    Change {
        path: PathBuf::from(path),
        original_path: original_path.map(PathBuf::from),
        area,
        status: original_path.map_or(ChangeStatus::Modified, |_| ChangeStatus::Renamed),
    }
}

fn repository(name_with_owner: &str, url: &str) -> GitHubRepository {
    GitHubRepository {
        name_with_owner: name_with_owner.to_owned(),
        url: url.to_owned(),
        remotes: Vec::new(),
    }
}

#[test]
fn previous_list_index_wraps_and_saturates() {
    let cases = [
        ((0, 0), 0),
        ((1, 0), 0),
        ((usize::MAX, 0), usize::MAX - 1),
        ((0, 1), 0),
        ((1, 1), 0),
        ((0, 2), 1),
        ((1, 2), 0),
        ((2, 2), 1),
        ((3, 5), 2),
        ((4, 5), 3),
        ((5, 5), 4),
        ((usize::MAX, usize::MAX), usize::MAX - 1),
    ];

    for ((selected, length), expected) in cases {
        assert_eq!(previous_list_index(selected, length), expected);
    }
}

#[test]
fn next_list_index_wraps_and_saturates() {
    let cases = [
        ((0, 0), 0),
        ((1, 0), 0),
        ((usize::MAX, 0), 0),
        ((0, 1), 0),
        ((1, 1), 0),
        ((0, 2), 1),
        ((1, 2), 0),
        ((2, 2), 0),
        ((3, 5), 4),
        ((4, 5), 0),
        ((5, 5), 0),
        ((usize::MAX, usize::MAX), 0),
    ];

    for ((selected, length), expected) in cases {
        assert_eq!(next_list_index(selected, length), expected);
    }
}

#[test]
fn estimated_patch_bytes_handles_known_unknown_and_saturated_counts() {
    let cases = [
        (None, PULL_REQUEST_PATCH_FALLBACK_ESTIMATE),
        (
            Some(DiffLineCounts {
                additions: 0,
                deletions: 0,
                binary: false,
            }),
            4_096,
        ),
        (
            Some(DiffLineCounts {
                additions: 1,
                deletions: 0,
                binary: false,
            }),
            4_176,
        ),
        (
            Some(DiffLineCounts {
                additions: 0,
                deletions: 1,
                binary: true,
            }),
            4_176,
        ),
        (
            Some(DiffLineCounts {
                additions: 10,
                deletions: 5,
                binary: false,
            }),
            5_296,
        ),
        (
            Some(DiffLineCounts {
                additions: usize::MAX,
                deletions: 0,
                binary: false,
            }),
            usize::MAX,
        ),
        (
            Some(DiffLineCounts {
                additions: 0,
                deletions: usize::MAX,
                binary: false,
            }),
            usize::MAX,
        ),
        (
            Some(DiffLineCounts {
                additions: usize::MAX,
                deletions: usize::MAX,
                binary: true,
            }),
            usize::MAX,
        ),
    ];

    for (counts, expected) in cases {
        assert_eq!(estimated_patch_bytes(counts), expected);
    }
}

#[test]
fn pull_request_file_status_labels_are_exhaustive() {
    let cases = [
        (PullRequestFileStatus::Added, "added"),
        (PullRequestFileStatus::Modified, "modified"),
        (PullRequestFileStatus::Deleted, "deleted"),
        (PullRequestFileStatus::Renamed, "renamed"),
        (PullRequestFileStatus::Copied, "copied"),
        (PullRequestFileStatus::TypeChanged, "type changed"),
        (PullRequestFileStatus::Unmerged, "unmerged"),
        (PullRequestFileStatus::Unknown, "changed"),
    ];

    for (status, expected) in cases {
        assert_eq!(pull_request_file_status_label(status), expected);
    }
}

#[test]
fn previous_character_observes_utf8_boundaries() {
    let value = "aé中🙂z";
    let cases = [
        (0, None),
        (1, Some((0, 'a'))),
        (2, None),
        (3, Some((1, 'é'))),
        (4, None),
        (5, None),
        (6, Some((3, '中'))),
        (7, None),
        (8, None),
        (9, None),
        (10, Some((6, '🙂'))),
        (11, Some((10, 'z'))),
        (12, None),
    ];

    for (cursor, expected) in cases {
        assert_eq!(previous_character(value, cursor), expected);
    }
}

#[test]
fn next_character_observes_utf8_boundaries() {
    let value = "aé中🙂z";
    let cases = [
        (0, Some((1, 'a'))),
        (1, Some((3, 'é'))),
        (2, None),
        (3, Some((6, '中'))),
        (4, None),
        (5, None),
        (6, Some((10, '🙂'))),
        (7, None),
        (8, None),
        (9, None),
        (10, Some((11, 'z'))),
        (11, None),
        (12, None),
    ];

    for (cursor, expected) in cases {
        assert_eq!(next_character(value, cursor), expected);
    }
}

#[test]
fn word_character_classification_covers_scripts_and_separators() {
    let cases = [
        ('a', true),
        ('Z', true),
        ('0', true),
        ('_', true),
        ('é', true),
        ('中', true),
        ('١', true),
        ('β', true),
        ('-', false),
        (' ', false),
        ('/', false),
        ('.', false),
        ('\n', false),
        ('́', false),
        ('🙂', false),
    ];

    for (character, expected) in cases {
        assert_eq!(is_word_character(character), expected);
    }
}

#[test]
fn url_path_encoding_preserves_only_path_safe_bytes() {
    let cases = [
        ("", ""),
        ("abcXYZ019-._~/", "abcXYZ019-._~/"),
        ("feature/topic", "feature/topic"),
        ("feature with space", "feature%20with%20space"),
        ("query?value#part", "query%3Fvalue%23part"),
        ("name:value", "name%3Avalue"),
        ("100%", "100%25"),
        ("[branch]", "%5Bbranch%5D"),
        ("a\\b", "a%5Cb"),
        ("line\nbreak\t", "line%0Abreak%09"),
        ("café/東京", "caf%C3%A9/%E6%9D%B1%E4%BA%AC"),
        ("🙂", "%F0%9F%99%82"),
    ];

    for (value, expected) in cases {
        assert_eq!(encode_url_path(value), expected);
    }
}

#[test]
fn repository_root_urls_require_an_exact_repository_suffix() {
    let cases = [
        (
            repository("acme/widget", "https://github.com/acme/widget"),
            Some("https://github.com"),
        ),
        (
            repository("acme/widget", "https://github.com/acme/widget/"),
            Some("https://github.com"),
        ),
        (
            repository("acme/widget", "https://git.example.test/scm/acme/widget"),
            Some("https://git.example.test/scm"),
        ),
        (repository("acme/widget", "acme/widget"), Some("")),
        (
            repository("acme/widget", "https://github.com/acme/other"),
            None,
        ),
        (
            repository("Acme/Widget", "https://github.com/acme/widget"),
            None,
        ),
        (
            repository("acme/widget", "https://github.com/acme/widget/issues"),
            None,
        ),
        (repository("acme/widget", ""), None),
    ];

    for (repository, expected) in &cases {
        assert_eq!(repository_root_url(repository), *expected);
    }
}

#[test]
fn repository_branch_targets_validate_and_encode_components() {
    let cases = [
        (("", "main"), None),
        (("https://github.com/acme/widget", ""), None),
        (
            ("https://github.com/acme/widget", "main"),
            Some("https://github.com/acme/widget/tree/main"),
        ),
        (
            ("https://github.com/acme/widget/", "feature/topic"),
            Some("https://github.com/acme/widget/tree/feature/topic"),
        ),
        (
            ("https://github.com/acme/widget///", "release candidate"),
            Some("https://github.com/acme/widget/tree/release%20candidate"),
        ),
        (
            ("https://github.com/acme/widget", "fix?#%"),
            Some("https://github.com/acme/widget/tree/fix%3F%23%25"),
        ),
        (
            ("https://github.com/acme/widget", "café/東京"),
            Some("https://github.com/acme/widget/tree/caf%C3%A9/%E6%9D%B1%E4%BA%AC"),
        ),
    ];

    for ((repository, branch), expected) in cases {
        let target =
            repository_branch_open_target(repository, branch).map(|OpenTarget::Browser(url)| url);
        assert_eq!(target.as_deref(), expected);
    }
}

#[test]
fn change_list_messages_keep_first_seen_display_paths() {
    let changes = [
        change("src/main.rs", None, ChangeArea::Unstaged),
        change("src/main.rs", None, ChangeArea::Staged),
        change("docs/new name.md", Some("docs/old.md"), ChangeArea::Staged),
        change("README.md", None, ChangeArea::Unstaged),
        change(
            "docs/new name.md",
            Some("docs/older.md"),
            ChangeArea::Unstaged,
        ),
    ];

    assert_eq!(change_list_message("Changed", &[]), "Changed");
    assert_eq!(
        change_list_message("Changed", &changes),
        "Changed\n  src/main.rs\n  docs/new name.md\n  README.md"
    );
    assert_eq!(
        change_list_message("", &changes[2..3]),
        "\n  docs/new name.md"
    );
}
