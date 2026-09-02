use super::*;

fn pull_request() -> PullRequest {
    PullRequest {
        number: 42,
        base_oid: BASE.to_owned(),
        head_oid: HEAD.to_owned(),
        ..PullRequest::default()
    }
}

#[test]
fn resolving_an_explicit_commit_accepts_a_unique_abbreviation() {
    let listing = commits(&[(OLDER, "older"), (HEAD, "head")]);
    let abbreviated: String = OLDER.chars().take(8).collect();

    assert_eq!(
        resolve_commit(&abbreviated, &listing, &pull_request()).unwrap(),
        OLDER
    );
    assert_eq!(
        resolve_commit(HEAD, &listing, &pull_request()).unwrap(),
        HEAD
    );
}

#[test]
fn the_merge_base_is_accepted_even_though_it_is_not_in_the_commit_list() {
    let listing = commits(&[(OLDER, "older"), (HEAD, "head")]);

    assert_eq!(
        resolve_commit(BASE, &listing, &pull_request()).unwrap(),
        BASE
    );
}

#[test]
fn resolving_is_case_insensitive_and_ignores_surrounding_space() {
    let listing = commits(&[(OLDER, "older"), (HEAD, "head")]);

    assert_eq!(
        resolve_commit(
            &format!("  {} ", OLDER.to_uppercase()),
            &listing,
            &pull_request()
        )
        .unwrap(),
        OLDER
    );
}

#[test]
fn resolving_rejects_an_empty_unknown_or_ambiguous_commit() {
    let listing = commits(&[("abc1", "one"), ("abc2", "two")]);

    drop(resolve_commit("", &listing, &pull_request()).unwrap_err());
    drop(resolve_commit("   ", &listing, &pull_request()).unwrap_err());
    drop(resolve_commit("deadbeef", &listing, &pull_request()).unwrap_err());
    drop(resolve_commit("abc", &listing, &pull_request()).unwrap_err());
    assert_eq!(
        resolve_commit("abc1", &listing, &pull_request()).unwrap(),
        "abc1"
    );
}

#[test]
fn an_unknown_commit_names_the_pull_request_it_searched() {
    let listing = commits(&[(HEAD, "head")]);

    let error = resolve_commit("deadbeef", &listing, &pull_request())
        .unwrap_err()
        .to_string();

    assert!(error.contains("pull request #42"), "{error}");
}
