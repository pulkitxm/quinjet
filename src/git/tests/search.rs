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
    assert!(
        repository
            .search(&request("worktree-token"))
            .unwrap()
            .paths
            .is_empty()
    );
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
