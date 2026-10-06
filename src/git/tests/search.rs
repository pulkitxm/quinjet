use super::*;
use crate::search::{SearchFile, SearchMode, SearchRequest, SearchSource, SearchTarget};

#[test]
fn name_search_finds_a_change_path_without_reading_contents() {
    let test_repository = TestRepository::new();
    fs::write(test_repository.path.join("secret.txt"), "payload\n").unwrap();
    let repository = test_repository.repository();
    let hits = repository
        .search(&SearchRequest {
            query: "secret".to_owned(),
            mode: SearchMode::Name,
            target: SearchTarget::Changes {
                files: vec![SearchFile {
                    path: PathBuf::from("secret.txt"),
                    source: SearchSource::Worktree,
                }],
            },
        })
        .unwrap();
    assert_eq!(hits.paths, vec!["secret.txt".to_owned()]);
}

#[test]
fn contents_search_matches_worktree_bytes_and_ignores_names() {
    let test_repository = TestRepository::new();
    fs::write(test_repository.path.join("notes.txt"), "unique-token\n").unwrap();
    let repository = test_repository.repository();
    let files = vec![SearchFile {
        path: PathBuf::from("notes.txt"),
        source: SearchSource::Worktree,
    }];
    let by_name = repository
        .search(&SearchRequest {
            query: "unique-token".to_owned(),
            mode: SearchMode::Name,
            target: SearchTarget::Changes {
                files: files.clone(),
            },
        })
        .unwrap();
    assert!(by_name.paths.is_empty(), "{by_name:?}");
    let by_contents = repository
        .search(&SearchRequest {
            query: "unique-token".to_owned(),
            mode: SearchMode::Contents,
            target: SearchTarget::Changes { files },
        })
        .unwrap();
    assert_eq!(by_contents.paths, vec!["notes.txt".to_owned()]);
}

#[test]
fn staged_contents_search_reads_the_index_version() {
    let test_repository = TestRepository::new();
    fs::write(test_repository.path.join("notes.txt"), "staged-token\n").unwrap();
    run_test_git(&test_repository.path, ["add", "notes.txt"]);
    fs::write(test_repository.path.join("notes.txt"), "worktree-token\n").unwrap();
    let repository = test_repository.repository();
    let request = |query: &str| SearchRequest {
        query: query.to_owned(),
        mode: SearchMode::Contents,
        target: SearchTarget::Changes {
            files: vec![SearchFile {
                path: PathBuf::from("notes.txt"),
                source: SearchSource::Index,
            }],
        },
    };
    assert_eq!(
        repository.search(&request("staged-token")).unwrap().paths,
        vec!["notes.txt".to_owned()]
    );
    assert_eq!(
        repository.search(&request("worktree-token")).unwrap().paths,
        Vec::<String>::new()
    );
}

#[test]
fn contents_search_reuses_state_across_worktree_index_and_pull_request_documents() {
    let test_repository = TestRepository::new();
    let documents: &[(&str, &[u8])] = &[
        ("00-empty.txt", b""),
        ("01-hit.txt", b"NEEDLE\n"),
        ("02-miss.txt", b"no match\n"),
        ("03-utf16.txt", b"\xff\xfen\0e\0e\0d\0l\0e\0\n\0"),
        ("04-binary.bin", b"needle\0needle\n"),
        ("05-hit.txt", b"needle\n"),
        ("06-empty.txt", b""),
    ];
    for &(name, contents) in documents {
        fs::write(test_repository.path.join(name), contents).unwrap();
    }
    run_test_git(&test_repository.path, ["add", "."]);
    let repository = test_repository.repository();
    for source in [SearchSource::Worktree, SearchSource::Index] {
        for mode in [SearchMode::Contents, SearchMode::Both] {
            let hits = repository
                .search(&SearchRequest {
                    query: "^needle$".to_owned(),
                    mode,
                    target: SearchTarget::Changes {
                        files: documents
                            .iter()
                            .map(|(name, _)| SearchFile {
                                path: PathBuf::from(name),
                                source,
                            })
                            .collect(),
                    },
                })
                .unwrap();
            assert_eq!(
                hits.paths,
                vec!["01-hit.txt", "03-utf16.txt", "05-hit.txt"],
                "{source:?}: {mode:?}"
            );
        }
    }
    let base_oid = run_test_git(&test_repository.path, ["rev-parse", "HEAD"]);
    run_test_git(
        &test_repository.path,
        [
            "-c",
            "user.name=Quinjet Test",
            "-c",
            "user.email=quinjet@example.com",
            "commit",
            "--message=search fixtures",
        ],
    );
    let workspace = repository
        .prepare_pull_request_diff(
            &github::PullRequest {
                base_oid,
                head_oid: run_test_git(&test_repository.path, ["rev-parse", "HEAD"]),
                ..github::PullRequest::default()
            },
            |_| {},
        )
        .unwrap();
    let paths: Vec<PathBuf> = documents
        .iter()
        .map(|(name, _)| PathBuf::from(name))
        .collect();
    for mode in [SearchMode::Contents, SearchMode::Both] {
        assert_eq!(
            workspace.search_paths("^needle$", mode, &paths).paths,
            vec!["01-hit.txt", "03-utf16.txt", "05-hit.txt"],
            "{mode:?}"
        );
    }
}

#[test]
fn history_contents_search_stays_on_the_loaded_page() {
    let test_repository = TestRepository::new();
    run_test_git(
        &test_repository.path,
        [
            "-c",
            "user.name=Quinjet Test",
            "-c",
            "user.email=quinjet@example.com",
            "commit",
            "--allow-empty",
            "--message=unique-history-body",
        ],
    );
    let repository = test_repository.repository();
    let hits = repository
        .search(&SearchRequest {
            query: "unique-history-body".to_owned(),
            mode: SearchMode::Contents,
            target: SearchTarget::History {
                revision: "HEAD".to_owned(),
                skip: 0,
                limit: 10,
                selected: None,
            },
        })
        .unwrap();
    assert_eq!(hits.commits.len(), 1, "{hits:?}");
}

#[test]
fn history_contents_search_reuses_state_for_messages_and_selected_patch() {
    let test_repository = TestRepository::new();
    let mut expected = Vec::new();
    for message in ["needle body", "no match", "NEEDLE body", "selected patch"] {
        if message == "selected patch" {
            fs::write(test_repository.path.join("notes.txt"), "needle patch\n").unwrap();
            run_test_git(&test_repository.path, ["add", "notes.txt"]);
        }
        run_test_git(
            &test_repository.path,
            [
                "-c",
                "user.name=Quinjet Test",
                "-c",
                "user.email=quinjet@example.com",
                "commit",
                "--allow-empty",
                "--message",
                message,
            ],
        );
        if message != "no match" {
            expected.push(run_test_git(&test_repository.path, ["rev-parse", "HEAD"]));
        }
    }
    let selected = run_test_git(&test_repository.path, ["rev-parse", "HEAD"]);
    expected.sort();
    let repository = test_repository.repository();
    let hits = repository
        .search(&SearchRequest {
            query: "needle".to_owned(),
            mode: SearchMode::Contents,
            target: SearchTarget::History {
                revision: "HEAD".to_owned(),
                skip: 0,
                limit: 10,
                selected: Some(selected),
            },
        })
        .unwrap();
    assert_eq!(hits.commits, expected);
}
