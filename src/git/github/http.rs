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
    for separator in [b"\r\n\r\n".as_slice(), b"\n\n".as_slice()] {
        if let Some(index) = output
            .windows(separator.len())
            .position(|window| window == separator)
        {
            let (head, rest) = output.split_at(index);
            return (
                String::from_utf8_lossy(head),
                rest.get(separator.len()..).unwrap_or_default(),
            );
        }
    }
    (String::from_utf8_lossy(output), &[])
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
    header_value(head, "link").is_some_and(|link| link.contains("rel=\"next\""))
}

/// The page number GitHub advertises as `rel="last"`, when the response is one
/// page of a longer listing.
pub(super) fn last_page(head: &str) -> Option<usize> {
    let link = header_value(head, "link")?;
    link.split(',').find_map(|segment| {
        if !segment.contains("rel=\"last\"") {
            return None;
        }
        let url = segment.trim().strip_prefix('<')?.split('>').next()?;
        url.split(['?', '&']).find_map(|parameter| {
            parameter
                .strip_prefix("page=")
                .and_then(|value| value.parse().ok())
        })
    })
}

pub(super) struct GhResponse {
    pub(super) data: Vec<u8>,
    pub(super) disposition: CacheDisposition,
}
