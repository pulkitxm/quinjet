use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use crossbeam_channel::{Receiver, Sender, bounded};

use crate::webhook_parser::{WebhookDelivery, content_length, parse_delivery};

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: u64 = 4 * 1024 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct WebhookListener {
    receiver: Receiver<WebhookDelivery>,
    address: SocketAddr,
    stopped: Arc<AtomicBool>,
}

impl WebhookListener {
    #[doc = " Bind a listener for `gh webhook forward --url http://<address>/`."]
    pub(crate) fn bind(target: &str) -> Result<Self> {
        let address = parse_listen_address(target)?;
        let listener = TcpListener::bind(address)
            .with_context(|| format!("failed to listen for webhooks on {address}"))?;
        let address = listener.local_addr().unwrap_or(address);
        let (sender, receiver) = bounded(64);
        let stopped = Arc::new(AtomicBool::new(false));
        let worker_stopped = Arc::clone(&stopped);
        drop(
            thread::Builder::new()
                .name("quinjet-webhook".to_owned())
                .spawn(move || serve(&listener, &sender, &worker_stopped))
                .context("failed to start the webhook listener")?,
        );
        Ok(Self {
            receiver,
            address,
            stopped,
        })
    }

    pub(crate) const fn deliveries(&self) -> &Receiver<WebhookDelivery> {
        &self.receiver
    }
}

impl Drop for WebhookListener {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        drop(TcpStream::connect(self.address));
    }
}

#[doc = " Accept only local connections. `gh webhook forward` runs beside Quinjet, so"]
#[doc = " a remote peer is never a legitimate source and is refused before its request"]
#[doc = " is read at all."]
fn serve(listener: &TcpListener, sender: &Sender<WebhookDelivery>, stopped: &AtomicBool) {
    for stream in listener.incoming() {
        if stopped.load(Ordering::Relaxed) {
            return;
        }
        let Ok(stream) = stream else {
            continue;
        };
        let local = stream.peer_addr().is_ok_and(|peer| peer.ip().is_loopback());
        if !local {
            continue;
        }
        if let Some(delivery) = read_delivery(stream) {
            drop(sender.try_send(delivery));
        }
    }
}

fn read_delivery(mut stream: TcpStream) -> Option<WebhookDelivery> {
    drop(stream.set_read_timeout(Some(READ_TIMEOUT)));
    drop(stream.set_write_timeout(Some(READ_TIMEOUT)));
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let head = read_head(&mut reader)?;
    let delivery = parse_delivery(&head);

    if let Some(length) = content_length(&head) {
        let mut body = Vec::new();
        drop(
            reader
                .take(length.min(MAX_BODY_BYTES))
                .read_to_end(&mut body),
        );
    }
    let response = if delivery.is_some() {
        "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    } else {
        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    };
    drop(stream.write_all(response.as_bytes()));
    drop(stream.flush());
    delivery
}

fn read_head(reader: &mut BufReader<TcpStream>) -> Option<String> {
    let mut head = String::new();
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line).ok()?;
        if read == 0 {
            return None;
        }
        if line == "\r\n" || line == "\n" {
            return Some(head);
        }
        head.push_str(&line);
        if head.len() > MAX_HEADER_BYTES {
            return None;
        }
    }
}

#[doc = " Accept either a full socket address or a bare port, which is what anyone"]
#[doc = " pairing this with `gh webhook forward` reaches for first."]
fn parse_listen_address(target: &str) -> Result<SocketAddr> {
    let target = target.trim();
    if let Ok(address) = target.parse::<SocketAddr>() {
        return Ok(address);
    }
    if let Ok(port) = target.parse::<u16>() {
        return Ok(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port));
    }
    bail!("`{target}` is not a port or a host:port address")
}

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn accepts_a_bare_port_or_a_full_address() {
        assert_eq!(
            parse_listen_address("8787").unwrap(),
            SocketAddr::from(([127, 0, 0, 1], 8787))
        );
        assert_eq!(
            parse_listen_address(" 0.0.0.0:9000 ").unwrap(),
            SocketAddr::from(([0, 0, 0, 0], 9000))
        );
        parse_listen_address("not-an-address").unwrap_err();
    }

    #[test]
    fn reads_the_event_name_and_ignores_anything_that_is_not_a_delivery() {
        let head = "POST / HTTP/1.1\r\nHost: localhost\r\nX-GitHub-Event: pull_request\r\nContent-Length: 12\r\n";
        assert_eq!(
            parse_delivery(head),
            Some(WebhookDelivery {
                event: "pull_request".to_owned()
            })
        );
        assert_eq!(content_length(head), Some(12));

        let lower = "POST /hook HTTP/1.1\r\nx-github-event: check_run\r\n";
        assert_eq!(
            parse_delivery(lower).unwrap().event,
            "check_run",
            "header names are case insensitive"
        );
        assert_eq!(
            parse_delivery("POST / HTTP/1.1\r\n").unwrap().event,
            "unknown"
        );
        assert!(parse_delivery("GET / HTTP/1.1\r\n").is_none());
        assert!(parse_delivery("").is_none());
    }

    #[test]
    fn a_forwarded_delivery_arrives_as_a_signal_and_is_answered() {
        let listener = WebhookListener::bind("127.0.0.1:0").unwrap();
        let mut stream = TcpStream::connect(listener.address).unwrap();
        let body = br#"{"action":"synchronize"}"#;
        write!(
            stream,
            "POST /webhook HTTP/1.1\r\nHost: localhost\r\nX-GitHub-Event: pull_request\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(body).unwrap();
        stream.flush().unwrap();

        let delivery = listener
            .deliveries()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert_eq!(delivery.event, "pull_request");

        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(
            response.starts_with("HTTP/1.1 204"),
            "the sender is answered so `gh webhook forward` does not report a failure: {response}"
        );
    }
}
