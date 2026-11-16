use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HEAD_LIMIT: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Target {
    host: String,
    port: u16,
    path: String,
}

fn target(url: &str) -> Result<Target> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| anyhow!("`{url}` is not an http:// address"))?;
    let (authority, path) = rest.find('/').map_or((rest, "/"), |index| {
        (
            rest.get(..index).unwrap_or(rest),
            rest.get(index..).unwrap_or("/"),
        )
    });
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !host.ends_with(']') || authority.starts_with('[') => (
            host,
            port.parse::<u16>()
                .with_context(|| format!("`{url}` has an invalid port"))?,
        ),
        _ => (authority, 80),
    };
    if host.is_empty() {
        bail!("`{url}` has no host");
    }
    Ok(Target {
        host: host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_owned(),
        port,
        path: path.to_owned(),
    })
}

pub(crate) fn request(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: &[u8],
    timeout: Duration,
    limit: usize,
) -> Result<Response> {
    let target = target(url)?;
    let started = Instant::now();
    let mut stream = connect(&target)?;
    let mut head = format!(
        "{method} {} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\nContent-Length: {}\r\n",
        target.path,
        target.host,
        target.port,
        body.len()
    );
    for (name, value) in headers {
        head.push_str(name);
        head.push_str(": ");
        head.push_str(value);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    stream.set_write_timeout(Some(timeout))?;
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    let mut received = Vec::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let remaining = timeout
            .checked_sub(started.elapsed())
            .filter(|left| !left.is_zero())
            .ok_or_else(|| anyhow!("no answer from {url} within {} seconds", timeout.as_secs()))?;
        stream.set_read_timeout(Some(remaining))?;
        let read = stream
            .read(&mut buffer)
            .with_context(|| format!("failed to read the answer from {url}"))?;
        if read == 0 {
            break;
        }
        received.extend_from_slice(buffer.get(..read).unwrap_or_default());
        if received.len() > limit.saturating_add(HEAD_LIMIT) {
            bail!("the answer from {url} is larger than {limit} bytes");
        }
        if complete(&received) {
            break;
        }
    }
    parse(&received)
}

fn connect(target: &Target) -> Result<TcpStream> {
    let addresses = (target.host.as_str(), target.port)
        .to_socket_addrs()
        .with_context(|| format!("could not resolve {}", target.host))?;
    let mut last = None;
    for address in addresses {
        match TcpStream::connect_timeout(&address, CONNECT_TIMEOUT) {
            Ok(stream) => return Ok(stream),
            Err(error) => last = Some(error),
        }
    }
    Err(last.map_or_else(
        || anyhow!("{} resolved to no address", target.host),
        |error| {
            anyhow!(
                "could not connect to {}:{}: {error}",
                target.host,
                target.port
            )
        },
    ))
}

fn split_head(bytes: &[u8]) -> Option<(&[u8], &[u8])> {
    let end = bytes.windows(4).position(|window| window == b"\r\n\r\n")?;
    Some((bytes.get(..end)?, bytes.get(end.saturating_add(4)..)?))
}

fn content_length(head: &[u8]) -> Option<usize> {
    String::from_utf8_lossy(head).lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse().ok())?
    })
}

fn complete(bytes: &[u8]) -> bool {
    split_head(bytes)
        .is_some_and(|(head, body)| content_length(head).is_some_and(|length| body.len() >= length))
}

fn parse(bytes: &[u8]) -> Result<Response> {
    let (head, body) = split_head(bytes).ok_or_else(|| anyhow!("the answer had no HTTP header"))?;
    let head = String::from_utf8_lossy(head);
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| anyhow!("the answer had no HTTP status"))?;
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_owned(), value.trim().to_owned()))
        .collect();
    let chunked = headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("transfer-encoding") && value.eq_ignore_ascii_case("chunked")
    });
    let body = if chunked {
        dechunk(body)?
    } else {
        let length = content_length(head.as_bytes()).unwrap_or(body.len());
        body.get(..length.min(body.len()))
            .unwrap_or_default()
            .to_vec()
    };
    Ok(Response {
        status,
        headers,
        body,
    })
}

fn dechunk(mut bytes: &[u8]) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let line_end = bytes
            .windows(2)
            .position(|window| window == b"\r\n")
            .ok_or_else(|| anyhow!("a chunk had no size line"))?;
        let size_line = String::from_utf8_lossy(bytes.get(..line_end).unwrap_or_default());
        let size = usize::from_str_radix(size_line.split(';').next().unwrap_or("").trim(), 16)
            .context("a chunk size was not hexadecimal")?;
        let start = line_end.saturating_add(2);
        if size == 0 {
            return Ok(body);
        }
        let end = start.saturating_add(size);
        body.extend_from_slice(
            bytes
                .get(start..end)
                .ok_or_else(|| anyhow!("a chunk ended early"))?,
        );
        bytes = bytes.get(end.saturating_add(2)..).unwrap_or_default();
    }
}

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests {
    use super::*;

    #[test]
    fn addresses_split_into_host_port_and_path() {
        assert_eq!(
            target("http://localhost:7381/mcp").unwrap(),
            Target {
                host: "localhost".to_owned(),
                port: 7381,
                path: "/mcp".to_owned(),
            }
        );
        assert_eq!(target("http://[::1]:9/x").unwrap().host, "::1");
        assert_eq!(target("http://gateway").unwrap().port, 80);
        assert_eq!(target("http://gateway").unwrap().path, "/");
        target("https://gateway").unwrap_err();
        target("http://:80/").unwrap_err();
    }

    #[test]
    fn fixed_length_and_chunked_bodies_are_read() {
        let fixed =
            parse(b"HTTP/1.1 200 OK\r\nMcp-Session-Id: s1\r\nContent-Length: 2\r\n\r\nokextra")
                .unwrap();
        assert_eq!(fixed.status, 200);
        assert_eq!(fixed.header("mcp-session-id"), Some("s1"));
        assert_eq!(fixed.body, b"ok");
        let chunked = parse(
            b"HTTP/1.1 202 Accepted\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2;x=y\r\nde\r\n0\r\n\r\n",
        )
        .unwrap();
        assert_eq!(chunked.status, 202);
        assert_eq!(chunked.body, b"abcde");
        parse(b"HTTP/1.1 200 OK\r\n").unwrap_err();
        dechunk(b"zz\r\n").unwrap_err();
    }

    #[test]
    fn a_full_content_length_ends_the_read() {
        assert!(complete(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok"));
        assert!(!complete(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nok"));
        assert!(!complete(b"HTTP/1.1 200 OK\r\n\r\nok"));
    }
}
