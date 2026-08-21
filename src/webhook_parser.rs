#[doc = " A GitHub delivery forwarded to this session, reduced to the event name."]
#[doc = ""]
#[doc = " Quinjet treats a delivery purely as a signal that something changed and then"]
#[doc = " re-reads the pull request through `gh`. Nothing from the request body is"]
#[doc = " trusted or displayed, which is what makes an unauthenticated loopback"]
#[doc = " listener safe: the worst a forged request can do is trigger a refresh that"]
#[doc = " would have happened on the next poll anyway."]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WebhookDelivery {
    pub event: String,
}

pub(crate) fn parse_delivery(head: &str) -> Option<WebhookDelivery> {
    let mut lines = head.lines();
    let request_line = lines.next()?;
    if !is_post_request(request_line) {
        return None;
    }
    let event = lines
        .take_while(|line| !line.is_empty())
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case("x-github-event")
                .then(|| value.trim().to_owned())
        })
        .unwrap_or_else(|| "unknown".to_owned());
    Some(WebhookDelivery { event })
}

pub(crate) fn content_length(head: &str) -> Option<u64> {
    let value = head
        .lines()
        .skip(1)
        .take_while(|line| !line.is_empty())
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case("content-length")
                .then_some(value.trim())
        })?;
    value.parse().ok()
}

fn is_post_request(request_line: &str) -> bool {
    let Some((method, target_and_version)) = request_line.split_once(' ') else {
        return false;
    };
    let Some((target, version)) = target_and_version.split_once(' ') else {
        return false;
    };
    method == "POST" && !target.is_empty() && matches!(version, "HTTP/1.0" | "HTTP/1.1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_line_contract_is_explicit() {
        for (name, request_line, accepted) in [
            ("root", "POST / HTTP/1.1", true),
            ("http 1.0", "POST /hook HTTP/1.0", true),
            ("query", "POST /hook?id=42 HTTP/1.1", true),
            ("unicode target", "POST /déploiement HTTP/1.1", true),
            ("get", "GET / HTTP/1.1", false),
            ("put", "PUT / HTTP/1.1", false),
            ("patch", "PATCH / HTTP/1.1", false),
            ("lowercase", "post / HTTP/1.1", false),
            ("empty", "", false),
            ("method only", "POST", false),
            ("trailing separator", "POST ", false),
            ("missing version", "POST /", false),
            ("missing target", "POST  HTTP/1.1", false),
            ("tab separators", "POST\t/\tHTTP/1.1", false),
            ("leading space", " POST / HTTP/1.1", false),
            ("http 2", "POST / HTTP/2", false),
            ("extra token", "POST / HTTP/1.1 extra", false),
            ("method prefix", "POSTER / HTTP/1.1", false),
        ] {
            let actual = parse_delivery(request_line).is_some();
            assert_eq!(actual, accepted, "{name}: {request_line:?}");
        }
    }

    #[test]
    fn event_header_contract_is_explicit() {
        for (name, head, expected) in [
            (
                "canonical crlf",
                "POST / HTTP/1.1\r\nX-GitHub-Event: pull_request\r\n",
                "pull_request",
            ),
            (
                "lowercase lf",
                "POST / HTTP/1.1\nx-github-event: check_run\n",
                "check_run",
            ),
            (
                "uppercase",
                "POST / HTTP/1.1\nX-GITHUB-EVENT: push\n",
                "push",
            ),
            (
                "name whitespace",
                "POST / HTTP/1.1\n  X-GitHub-Event \t: issue_comment\n",
                "issue_comment",
            ),
            (
                "value whitespace",
                "POST / HTTP/1.1\nX-GitHub-Event:\t workflow_run  \n",
                "workflow_run",
            ),
            ("missing", "POST / HTTP/1.1\nHost: localhost\n", "unknown"),
            ("empty", "POST / HTTP/1.1\nX-GitHub-Event:\n", ""),
            (
                "whitespace only",
                "POST / HTTP/1.1\nX-GitHub-Event:  \t\n",
                "",
            ),
            (
                "first duplicate",
                "POST / HTTP/1.1\nX-GitHub-Event: push\nX-GitHub-Event: pull_request\n",
                "push",
            ),
            (
                "empty first duplicate",
                "POST / HTTP/1.1\nX-GitHub-Event:\nX-GitHub-Event: push\n",
                "",
            ),
            (
                "malformed before valid",
                "POST / HTTP/1.1\nnot a header\nX-GitHub-Event: release\n",
                "release",
            ),
            (
                "missing colon",
                "POST / HTTP/1.1\nX-GitHub-Event push\n",
                "unknown",
            ),
            (
                "colon in value",
                "POST / HTTP/1.1\nX-GitHub-Event: pull:request\n",
                "pull:request",
            ),
            (
                "unicode value",
                "POST / HTTP/1.1\nX-GitHub-Event: déploiement 🚀\n",
                "déploiement 🚀",
            ),
            (
                "unicode header name",
                "POST / HTTP/1.1\nX-GitHub-Évent: push\n",
                "unknown",
            ),
            (
                "lf body separator",
                "POST / HTTP/1.1\nHost: local\n\nX-GitHub-Event: push\n",
                "unknown",
            ),
            (
                "crlf body separator",
                "POST / HTTP/1.1\r\nHost: local\r\n\r\nX-GitHub-Event: push\r\n",
                "unknown",
            ),
        ] {
            let event = parse_delivery(head).map(|delivery| delivery.event);
            assert_eq!(event.as_deref(), Some(expected), "{name}: {head:?}");
        }
    }

    #[test]
    fn content_length_contract_is_explicit() {
        for (name, head, expected) in [
            (
                "canonical",
                "POST / HTTP/1.1\nContent-Length: 12\n",
                Some(12),
            ),
            ("lowercase", "POST / HTTP/1.1\ncontent-length: 7\n", Some(7)),
            ("uppercase", "POST / HTTP/1.1\nCONTENT-LENGTH: 8\n", Some(8)),
            (
                "whitespace",
                "POST / HTTP/1.1\n Content-Length \t:  19 \t\n",
                Some(19),
            ),
            ("zero", "POST / HTTP/1.1\nContent-Length: 0\n", Some(0)),
            (
                "leading zeroes",
                "POST / HTTP/1.1\nContent-Length: 00012\n",
                Some(12),
            ),
            (
                "maximum",
                "POST / HTTP/1.1\nContent-Length: 18446744073709551615\n",
                Some(u64::MAX),
            ),
            (
                "overflow",
                "POST / HTTP/1.1\nContent-Length: 18446744073709551616\n",
                None,
            ),
            ("negative", "POST / HTTP/1.1\nContent-Length: -1\n", None),
            ("decimal", "POST / HTTP/1.1\nContent-Length: 1.5\n", None),
            ("units", "POST / HTTP/1.1\nContent-Length: 12 bytes\n", None),
            ("empty", "POST / HTTP/1.1\nContent-Length:\n", None),
            (
                "whitespace only",
                "POST / HTTP/1.1\nContent-Length: \t\n",
                None,
            ),
            ("missing", "POST / HTTP/1.1\nHost: localhost\n", None),
            (
                "first duplicate",
                "POST / HTTP/1.1\nContent-Length: 5\nContent-Length: 9\n",
                Some(5),
            ),
            (
                "invalid first duplicate",
                "POST / HTTP/1.1\nContent-Length: invalid\nContent-Length: 9\n",
                None,
            ),
            (
                "malformed before valid",
                "POST / HTTP/1.1\nnot a header\nContent-Length: 6\n",
                Some(6),
            ),
            (
                "lf body confusion",
                "POST / HTTP/1.1\nHost: local\n\nContent-Length: 99\n",
                None,
            ),
            (
                "crlf body confusion",
                "POST / HTTP/1.1\r\nHost: local\r\n\r\nContent-Length: 99\r\n",
                None,
            ),
            (
                "real header before body",
                "POST / HTTP/1.1\nContent-Length: 4\n\nContent-Length: 99\n",
                Some(4),
            ),
            (
                "unicode digits",
                "POST / HTTP/1.1\nContent-Length: １２\n",
                None,
            ),
            ("hex", "POST / HTTP/1.1\nContent-Length: 0xc\n", None),
            (
                "internal whitespace",
                "POST / HTTP/1.1\nContent-Length: 1 2\n",
                None,
            ),
        ] {
            assert_eq!(content_length(head), expected, "{name}: {head:?}");
        }
    }
}
