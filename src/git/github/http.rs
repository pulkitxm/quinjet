#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

/// A validated read: GitHub is asked whether the answer changed, and answers
/// `304 Not Modified` when it did not. That reply carries no body and costs
/// nothing against the rate limit, which is what lets an unchanged thread be
/// re-checked as often as it is worth checking.
///
/// The entry holds the validator on its first line and the body after it, so
/// the two can never be stored out of step with each other.
pub(crate) struct ValidatedRead {
    pub data: Vec<u8>,
    pub unchanged: bool,
    pub complete: bool,
    pub truncated: bool,
    pub last_page: Option<usize>,
}

impl Repository {
    pub(crate) fn validated_gh(&self, key: &str, args: Vec<OsString>) -> Result<ValidatedRead> {
        let cached = cache_read(key, CacheLife::Immutable);
        let validator = cached.as_ref().and_then(|entry| split_validator(entry).0);
        let mut request = vec![OsString::from("api"), OsString::from("-i")];
        if let Some(validator) = validator.as_ref() {
            request.push(OsString::from("-H"));
            request.push(OsString::from(format!("If-None-Match: {validator}")));
        }
        request.extend(args);

        let output = self.run_gh(request)?;
        if !output.status.success() && !output.stdout_truncated {
            bail!(
                "{}",
                bounded_command_error("unable to read from GitHub", &output)
            );
        }
        let (head, body) = split_http_response(&output.stdout);
        let head = head.as_ref();
        let status =
            String::from_utf8_lossy(head.lines().next().unwrap_or_default().as_bytes()).to_string();
        if status.contains(" 304")
            && let Some(entry) = cached
        {
            return Ok(ValidatedRead {
                data: split_validator(&entry).1.to_vec(),
                unchanged: true,
                complete: true,
                truncated: false,
                last_page: None,
            });
        }
        let complete = !output.stdout_truncated && !has_next_page(head);
        if let Some(etag) = header_value(head, "etag").filter(|_| complete) {
            let mut entry = etag.into_bytes();
            entry.push(b'\n');
            entry.extend_from_slice(body);
            cache_write(key, &entry);
        }
        Ok(ValidatedRead {
            data: body.to_vec(),
            unchanged: false,
            complete,
            truncated: output.stdout_truncated,
            last_page: last_page(head),
        })
    }
}

/// Split the stored entry into its validator and the body it validates.
pub(super) fn split_validator(entry: &[u8]) -> (Option<String>, &[u8]) {
    entry
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or((None, entry), |index| {
            let (validator, body) = entry.split_at(index);
            (
                Some(String::from_utf8_lossy(validator).into_owned()),
                body.get(1..).unwrap_or_default(),
            )
        })
}

/// `gh api -i` prints the response head, a blank line, then the body.
pub(super) fn split_http_response(output: &[u8]) -> (Cow<'_, str>, &[u8]) {
    let separator = [b"\r\n\r\n".as_slice(), b"\n\n".as_slice()]
        .into_iter()
        .filter_map(|separator| {
            output
                .windows(separator.len())
                .position(|window| window == separator)
                .map(|index| (index, separator))
        })
        .min_by_key(|(index, _)| *index);
    let Some((index, separator)) = separator else {
        return (String::from_utf8_lossy(output), &[]);
    };
    let (head, rest) = output.split_at(index);
    (
        String::from_utf8_lossy(head),
        rest.get(separator.len()..).unwrap_or_default(),
    )
}

pub(super) fn header_value(head: &str, name: &str) -> Option<String> {
    head.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().to_owned())
    })
}

pub(super) fn has_next_page(head: &str) -> bool {
    header_value(head, "link").is_some_and(|link| {
        link.split(',')
            .any(|segment| link_target_for_relation(segment, "next").is_some())
    })
}

/// The page number GitHub advertises as `rel="last"`, when the response is one
/// page of a longer listing.
pub(super) fn last_page(head: &str) -> Option<usize> {
    let link = header_value(head, "link")?;
    link.split(',').find_map(|segment| {
        let url = link_target_for_relation(segment, "last")?;
        let (_, query) = url.split_once('?')?;
        query
            .split('#')
            .next()
            .unwrap_or_default()
            .split('&')
            .find_map(|parameter| {
                let (key, value) = parameter.split_once('=')?;
                if key != "page" {
                    return None;
                }
                value.parse::<usize>().ok().filter(|page| *page > 0)
            })
    })
}

fn link_target_for_relation<'a>(segment: &'a str, relation: &str) -> Option<&'a str> {
    let (target, parameters) = segment.trim().split_once('>')?;
    let target = target.strip_prefix('<')?;
    let parameters = parameters.trim_start();
    if !parameters.starts_with(';') {
        return None;
    }
    parameters
        .split(';')
        .filter_map(|parameter| parameter.trim().split_once('='))
        .find_map(|(key, value)| {
            if !key.trim().eq_ignore_ascii_case("rel") {
                return None;
            }
            value
                .trim()
                .strip_prefix('"')?
                .strip_suffix('"')?
                .split_ascii_whitespace()
                .any(|value| value.eq_ignore_ascii_case(relation))
                .then_some(target)
        })
}

pub(super) struct GhResponse {
    pub(super) data: Vec<u8>,
    pub(super) disposition: CacheDisposition,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validators_split_at_the_first_line_feed() {
        let (validator, body) = split_validator(b"W/\"tag\"\nbody\nmore");
        assert_eq!(validator.as_deref(), Some("W/\"tag\""));
        assert_eq!(body, b"body\nmore");

        let (validator, body) = split_validator(b"tag\n\0\xff\n");
        assert_eq!(validator.as_deref(), Some("tag"));
        assert_eq!(body, b"\0\xff\n");

        assert_eq!(split_validator(b"tag\n").1, b"");
        let (validator, body) = split_validator(b"body without validator");
        assert_eq!(validator, None);
        assert_eq!(body, b"body without validator");
    }

    #[test]
    fn responses_preserve_crlf_lf_and_binary_bodies() {
        let (head, body) = split_http_response(b"HTTP/2 200\r\nEtag: one\r\n\r\nbody");
        assert_eq!(head, "HTTP/2 200\r\nEtag: one");
        assert_eq!(body, b"body");

        let mut response = b"HTTP/2 200\nEtag: two\n\n".to_vec();
        response.extend_from_slice(b"\xff\0body\r\n\r\ntail");
        let (head, body) = split_http_response(&response);
        assert_eq!(head, "HTTP/2 200\nEtag: two");
        assert_eq!(body, b"\xff\0body\r\n\r\ntail");

        let (head, body) = split_http_response(b"HTTP/2 204");
        assert_eq!(head, "HTTP/2 204");
        assert_eq!(body, b"");
    }

    #[test]
    fn headers_are_case_insensitive_and_trimmed() {
        let head = "HTTP/2 200\r\neTaG :  W/\"tag\"  \r\nLINK: <https://api.test/x>; rel=\"next\"\r\nBroken";
        assert_eq!(header_value(head, "ETAG").as_deref(), Some("W/\"tag\""));
        assert_eq!(
            header_value(head, "link").as_deref(),
            Some("<https://api.test/x>; rel=\"next\"")
        );
        assert_eq!(header_value(head, "broken"), None);
        assert_eq!(header_value(head, "etag-extra"), None);
    }

    #[test]
    fn next_relations_accept_parameter_and_token_variants() {
        for link in [
            "<https://api.test/x?page=2>; rel=\"next\"",
            "<https://api.test/x?page=2>; type=\"json\"; ReL=\"NEXT\"",
            "<https://api.test/x?page=1>; rel=\"prev\", <https://api.test/x?page=2>; rel=\"prev next\"",
        ] {
            assert!(
                has_next_page(&format!("HTTP/2 200\nLiNk: {link}")),
                "{link}"
            );
        }
    }

    #[test]
    fn malformed_next_relations_are_rejected() {
        for link in [
            "<https://api.test/x?rel=\"next\">; rel=\"prev\"",
            "<https://api.test/x?page=2; rel=\"next\"",
            "<https://api.test/x?page=2>; rel=next",
            "<https://api.test/x?page=2>; rel=\"next-page\"",
            "https://api.test/x?page=2>; rel=\"next\"",
        ] {
            assert!(
                !has_next_page(&format!("HTTP/2 200\nLink: {link}")),
                "{link}"
            );
        }
    }

    #[test]
    fn last_page_accepts_ordered_and_combined_relations() {
        let ordered = "HTTP/2 200\nLink: <https://api.test/x?per_page=100&page=12>; title=\"end\"; rel=\"last\"";
        assert_eq!(last_page(ordered), Some(12));

        let combined =
            "HTTP/2 200\nLink: <https://api.test/x?page=7&per_page=100>; rel=\"NEXT LAST\"";
        assert_eq!(last_page(combined), Some(7));

        let later_valid = "HTTP/2 200\nLink: <https://api.test/x?page=bad>; rel=\"last\", <https://api.test/x?page=9>; rel=\"last\"";
        assert_eq!(last_page(later_valid), Some(9));
    }

    #[test]
    fn malformed_last_page_links_are_rejected() {
        for link in [
            "<https://api.test/x?per_page=100>; rel=\"last\"",
            "<https://api.test/x?page=zero>; rel=\"last\"",
            "<https://api.test/x?page=0>; rel=\"last\"",
            "<https://api.test/x?page=5; rel=\"last\"",
            "<https://api.test/x?page=5>; rel=last",
            "<https://api.test/x?page=5>; rel=\"last-page\"",
        ] {
            assert_eq!(
                last_page(&format!("HTTP/2 200\nLink: {link}")),
                None,
                "{link}"
            );
        }
    }
}
