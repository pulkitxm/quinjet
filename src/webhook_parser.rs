/// A GitHub delivery forwarded to this session, reduced to the event name.
///
/// Quinjet treats a delivery purely as a signal that something changed and then
/// re-reads the pull request through `gh`. Nothing from the request body is
/// trusted or displayed, which is what makes an unauthenticated loopback
/// listener safe: the worst a forged request can do is trigger a refresh that
/// would have happened on the next poll anyway.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WebhookDelivery {
    pub event: String,
}

pub(crate) fn parse_delivery(head: &str) -> Option<WebhookDelivery> {
    let mut lines = head.lines();
    let request_line = lines.next()?;
    if !request_line.starts_with("POST ") {
        return None;
    }
    let event = lines
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
    head.lines().skip(1).find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse().ok())
            .flatten()
    })
}
