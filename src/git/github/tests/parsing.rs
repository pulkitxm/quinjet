use super::*;

#[test]
fn a_response_head_is_read_apart_from_its_body() {
    let response = b"HTTP/2.0 200 OK\r\nEtag: W/\"92ade\"\r\nContent-Type: application/json\r\n\r\n[{\"a\":1}]";
    let (head, body) = split_http_response(response);
    assert!(head.starts_with("HTTP/2.0 200 OK"));
    assert_eq!(body, b"[{\"a\":1}]");
    assert_eq!(header_value(&head, "etag").as_deref(), Some("W/\"92ade\""));
    assert_eq!(header_value(&head, "ETAG").as_deref(), Some("W/\"92ade\""));
    assert_eq!(header_value(&head, "link"), None);
}

#[test]
fn a_body_the_head_cannot_describe_still_arrives_whole() {
    let mut response = b"HTTP/2.0 200 OK\n\n".to_vec();
    response.extend_from_slice(&[0xff, 0xfe, b'o', b'k']);
    let (head, body) = split_http_response(&response);
    assert_eq!(head, "HTTP/2.0 200 OK");
    assert_eq!(body, [0xff, 0xfe, b'o', b'k']);
}

#[test]
fn only_a_single_page_answer_is_worth_a_validator() {
    let paged = "HTTP/2.0 200 OK\nLink: <https://api.github.com/x?page=2>; rel=\"next\"";
    let last = "HTTP/2.0 200 OK\nLink: <https://api.github.com/x?page=1>; rel=\"prev\"";
    assert!(has_next_page(paged));
    assert!(!has_next_page(last));
    assert!(!has_next_page("HTTP/2.0 200 OK"));
}

#[test]
fn api_file_counts_parse_and_skip_malformed_records() {
    let data = b"src/main.rs\t12\t3\tmodified\nREADME.md\t1\t0\tmodified\nbroken record\nassets/logo.png\tnot\tnumbers\tadded\nassets/icon.png\t0\t0\tadded\nsrc/old_name.rs\t0\t0\trenamed\n";
    let counts = parse_api_file_counts(data);

    assert_eq!(
        counts.len(),
        3,
        "malformed and countless records are skipped, pure renames are kept"
    );
    assert_eq!(
        counts.get(Path::new("src/old_name.rs")),
        Some(&DiffLineCounts {
            additions: 0,
            deletions: 0,
            binary: false,
        }),
        "a pure rename really has zero changed lines"
    );
    assert_eq!(
        counts.get(Path::new("src/main.rs")),
        Some(&DiffLineCounts {
            additions: 12,
            deletions: 3,
            binary: false,
        })
    );
}

#[test]
fn the_link_header_names_the_newest_timeline_page() {
    let head = "HTTP/2.0 200 OK\nLink: <https://api.github.com/x?per_page=100&page=2>; rel=\"next\", <https://api.github.com/x?per_page=100&page=12>; rel=\"last\"";
    assert_eq!(last_page(head), Some(12));
    assert_eq!(
        last_page("HTTP/2.0 200 OK"),
        None,
        "a single page advertises no last page"
    );
    let reversed =
        "HTTP/2.0 200 OK\nLink: <https://api.github.com/x?page=7&per_page=100>; rel=\"last\"";
    assert_eq!(
        last_page(reversed),
        Some(7),
        "per_page never shadows the page parameter"
    );
}

#[test]
fn a_cache_entry_keeps_its_validator_beside_the_body_it_validates() {
    let entry = b"W/\"92ade\"\nname\tvalue\n";
    let (validator, body) = split_validator(entry);
    assert_eq!(validator.as_deref(), Some("W/\"92ade\""));
    assert_eq!(body, b"name\tvalue\n");

    let (missing, whole) = split_validator(b"no newline here");
    assert_eq!(missing, None);
    assert_eq!(whole, b"no newline here");
}
